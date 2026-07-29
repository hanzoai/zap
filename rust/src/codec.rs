//! The ZAP data segment — read and write.
//!
//! This is a line-for-line port of the canonical Go runtime
//! (`github.com/zap-proto/go` `zap.go` + `builder.go`). Every offset,
//! bound check and growth rule is deliberately identical, because the
//! two runtimes must emit byte-identical buffers and must AGREE on
//! accept/reject for every input — honest and adversarial. The proof
//! lives in `tests/crosswire.rs`, which pins Go's actual output.
//!
//! Layout: fixed-size fields sit at compile-time offsets inside a
//! struct's fixed section; variable-length fields (`bytes` / `text`)
//! occupy an `(offset: u32, length: u32)` pair at their slot, where
//! `offset` is RELATIVE to the position of the offset cell itself.

use crate::wire::{WireError, HEADER_SIZE, MAGIC, VERSION, VERSION2};

/// Data-segment alignment, in bytes.
pub const ALIGNMENT: usize = 8;

// ── read side ───────────────────────────────────────────────────────

/// A parsed ZAP message: a validated byte buffer read with zero copies.
#[derive(Debug, Clone, Copy)]
pub struct Message<'a> {
    data: &'a [u8],
}

impl<'a> Message<'a> {
    /// Parse a ZAP message. Accepts both wire versions (v1 and v2 share
    /// one data-segment encoding); callers that require a specific
    /// generation gate on [`Message::version`] afterwards.
    ///
    /// The declared size must be at least the header — a `size = 0`
    /// buffer would otherwise pass and then index an empty slice.
    pub fn parse(data: &'a [u8]) -> Result<Self, WireError> {
        if data.len() < HEADER_SIZE {
            return Err(WireError::ShortHeader {
                want: HEADER_SIZE,
                got: data.len(),
            });
        }
        if data[0..4] != MAGIC {
            return Err(WireError::InvalidMagic);
        }
        let version = u16::from_le_bytes([data[4], data[5]]);
        if version != VERSION && version != VERSION2 {
            return Err(WireError::InvalidVersion(version));
        }
        let size = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
        if size < HEADER_SIZE || size > data.len() {
            return Err(WireError::ShortBody {
                want: size,
                got: data.len(),
            });
        }
        Ok(Message {
            data: &data[..size],
        })
    }

    /// The wire version byte (1 or 2).
    pub fn version(&self) -> u16 {
        u16::from_le_bytes([self.data[4], self.data[5]])
    }

    /// The header flags word. The high byte carries the transport
    /// msgType (see [`crate::rpc::MSG_TYPE_ROUTER_BASE`]).
    pub fn flags(&self) -> u16 {
        u16::from_le_bytes([self.data[6], self.data[7]])
    }

    /// Total message size, header included.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// The whole validated buffer.
    pub fn bytes(&self) -> &'a [u8] {
        self.data
    }

    /// The root object.
    pub fn root(&self) -> Object<'a> {
        let offset = u32::from_le_bytes([self.data[8], self.data[9], self.data[10], self.data[11]]);
        Object {
            data: self.data,
            offset: offset as usize,
        }
    }
}

/// A zero-copy view into one ZAP struct.
#[derive(Debug, Clone, Copy, Default)]
pub struct Object<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Object<'a> {
    /// Absolute position of this object inside the message buffer.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// True for the null object (offset 0 / unset pointer).
    pub fn is_null(&self) -> bool {
        self.offset == 0 || self.data.is_empty()
    }

    pub fn bool(&self, field: usize) -> bool {
        self.u8(field) != 0
    }

    pub fn u8(&self, field: usize) -> u8 {
        let pos = self.offset + field;
        if pos >= self.data.len() {
            return 0;
        }
        self.data[pos]
    }

    pub fn u16(&self, field: usize) -> u16 {
        let pos = self.offset + field;
        if pos + 2 > self.data.len() {
            return 0;
        }
        u16::from_le_bytes(self.data[pos..pos + 2].try_into().unwrap())
    }

    pub fn u32(&self, field: usize) -> u32 {
        let pos = self.offset + field;
        if pos + 4 > self.data.len() {
            return 0;
        }
        u32::from_le_bytes(self.data[pos..pos + 4].try_into().unwrap())
    }

    pub fn u64(&self, field: usize) -> u64 {
        let pos = self.offset + field;
        if pos + 8 > self.data.len() {
            return 0;
        }
        u64::from_le_bytes(self.data[pos..pos + 8].try_into().unwrap())
    }

    pub fn i8(&self, field: usize) -> i8 {
        self.u8(field) as i8
    }
    pub fn i16(&self, field: usize) -> i16 {
        self.u16(field) as i16
    }
    pub fn i32(&self, field: usize) -> i32 {
        self.u32(field) as i32
    }
    pub fn i64(&self, field: usize) -> i64 {
        self.u64(field) as i64
    }
    pub fn f32(&self, field: usize) -> f32 {
        f32::from_bits(self.u32(field))
    }
    pub fn f64(&self, field: usize) -> f64 {
        f64::from_bits(self.u64(field))
    }

    /// UTF-8 text at `field`. Invalid UTF-8 reads as empty rather than
    /// panicking — a hostile peer must never crash a reader.
    pub fn text(&self, field: usize) -> &'a str {
        std::str::from_utf8(self.bytes(field)).unwrap_or("")
    }

    /// Bytes at `field`, aliasing the message buffer.
    ///
    /// The relative offset is an UNSIGNED FORWARD pointer. A crafted
    /// negative bit-pattern flows through as a huge positive value and
    /// is rejected by the bounds check; a target inside the wire header
    /// is rejected outright, since a payload can never legitimately
    /// live at offsets `0..HEADER_SIZE`. Without that rule a backward
    /// pointer could alias the header and leak the version/flags words.
    pub fn bytes(&self, field: usize) -> &'a [u8] {
        let pos = self.offset + field;
        if pos + 4 > self.data.len() {
            return &[];
        }
        let rel = u32::from_le_bytes(self.data[pos..pos + 4].try_into().unwrap()) as usize;
        if rel == 0 {
            return &[];
        }
        let len_pos = pos + 4;
        if len_pos + 4 > self.data.len() {
            return &[];
        }
        let len = u32::from_le_bytes(self.data[len_pos..len_pos + 4].try_into().unwrap()) as usize;
        let abs = pos.saturating_add(rel);
        if abs < HEADER_SIZE || abs.saturating_add(len) > self.data.len() {
            return &[];
        }
        &self.data[abs..abs + len]
    }

    /// A nested object at `field`.
    ///
    /// Here the relative offset is SIGNED: a builder may finalize a
    /// nested object before its parent, leaving the payload earlier in
    /// the segment. Any absolute target inside the wire header is still
    /// rejected.
    pub fn object(&self, field: usize) -> Object<'a> {
        let pos = self.offset + field;
        if pos + 4 > self.data.len() {
            return Object::default();
        }
        let rel = u32::from_le_bytes(self.data[pos..pos + 4].try_into().unwrap()) as i32;
        if rel == 0 {
            return Object::default();
        }
        let abs = pos as i64 + rel as i64;
        if abs < HEADER_SIZE as i64 || abs >= self.data.len() as i64 {
            return Object::default();
        }
        Object {
            data: self.data,
            offset: abs as usize,
        }
    }

    /// A list at `field`.
    ///
    /// The length is clamped to the message size: an attacker-set
    /// `length = 0xFFFFFFFF` would otherwise let a `for i in 0..len`
    /// consumer spin 4G times even though every element accessor
    /// returns zero.
    pub fn list(&self, field: usize) -> List<'a> {
        let pos = self.offset + field;
        if pos + 8 > self.data.len() {
            return List::default();
        }
        let rel = u32::from_le_bytes(self.data[pos..pos + 4].try_into().unwrap()) as i32;
        if rel == 0 {
            return List::default();
        }
        let len = u32::from_le_bytes(self.data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if len > self.data.len() {
            return List::default();
        }
        let abs = pos as i64 + rel as i64;
        if abs < HEADER_SIZE as i64 || abs >= self.data.len() as i64 {
            return List::default();
        }
        List {
            data: self.data,
            offset: abs as usize,
            length: len,
        }
    }
}

/// A zero-copy view into one ZAP list.
#[derive(Debug, Clone, Copy, Default)]
pub struct List<'a> {
    data: &'a [u8],
    offset: usize,
    length: usize,
}

impl<'a> List<'a> {
    pub fn len(&self) -> usize {
        self.length
    }
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
    pub fn is_null(&self) -> bool {
        self.data.is_empty()
    }

    pub fn u8(&self, i: usize) -> u8 {
        if i >= self.length {
            return 0;
        }
        let pos = self.offset + i;
        if pos >= self.data.len() {
            return 0;
        }
        self.data[pos]
    }

    pub fn u32(&self, i: usize) -> u32 {
        if i >= self.length {
            return 0;
        }
        let pos = self.offset + i * 4;
        if pos + 4 > self.data.len() {
            return 0;
        }
        u32::from_le_bytes(self.data[pos..pos + 4].try_into().unwrap())
    }

    pub fn u64(&self, i: usize) -> u64 {
        if i >= self.length {
            return 0;
        }
        let pos = self.offset + i * 8;
        if pos + 8 > self.data.len() {
            return 0;
        }
        u64::from_le_bytes(self.data[pos..pos + 8].try_into().unwrap())
    }

    /// Element `i` of a struct list with the given stride.
    pub fn object(&self, i: usize, elem_size: usize) -> Object<'a> {
        if i >= self.length {
            return Object::default();
        }
        Object {
            data: self.data,
            offset: self.offset + i * elem_size,
        }
    }

    /// The list's raw bytes (byte lists).
    pub fn bytes(&self) -> &'a [u8] {
        if self.data.is_empty() || self.offset + self.length > self.data.len() {
            return &[];
        }
        &self.data[self.offset..self.offset + self.length]
    }
}

// ── write side ──────────────────────────────────────────────────────

/// Builds a ZAP message: a growable buffer whose `pos` is the
/// high-water mark. The 16-byte header is reserved up front and
/// patched by [`Builder::finish`].
#[derive(Debug)]
pub struct Builder {
    buf: Vec<u8>,
    pos: usize,
    root_offset: usize,
}

impl Builder {
    /// A builder emitting a v1 header — the baseline this runtime uses
    /// for plain messages.
    pub fn new(capacity: usize) -> Self {
        Self::with_version(capacity, VERSION)
    }

    /// A builder emitting a v2 header — what the RPC call envelope and
    /// the transport framing use, so the bytes match the other language
    /// runtimes exactly.
    pub fn v2(capacity: usize) -> Self {
        Self::with_version(capacity, VERSION2)
    }

    fn with_version(capacity: usize, version: u16) -> Self {
        let cap = if capacity < HEADER_SIZE {
            256
        } else {
            capacity
        };
        let mut buf = vec![0u8; cap];
        buf[0..4].copy_from_slice(&MAGIC);
        buf[4..6].copy_from_slice(&version.to_le_bytes());
        Builder {
            buf,
            pos: HEADER_SIZE,
            root_offset: 0,
        }
    }

    /// Reset for reuse; the header stays.
    pub fn reset(&mut self) {
        self.pos = HEADER_SIZE;
        self.root_offset = 0;
    }

    fn grow(&mut self, n: usize) {
        if self.pos + n <= self.buf.len() {
            return;
        }
        let mut new_cap = self.buf.len() * 2;
        if new_cap < self.pos + n {
            new_cap = self.pos + n;
        }
        self.buf.resize(new_cap, 0);
    }

    fn align(&mut self, alignment: usize) {
        let padding = (alignment - (self.pos % alignment)) % alignment;
        self.grow(padding);
        for _ in 0..padding {
            self.buf[self.pos] = 0;
            self.pos += 1;
        }
    }

    /// Start an object with the given fixed-section size.
    ///
    /// The fixed section is reserved and zero-filled up front, so
    /// fields may be set in any order and any variable tail written
    /// afterwards lands AFTER the whole fixed section — never
    /// interleaved with a field that has not been written yet.
    pub fn object(&mut self, data_size: usize) -> ObjectBuilder<'_> {
        self.align(ALIGNMENT);
        let start = self.pos;
        let mut ob = ObjectBuilder {
            b: self,
            start,
            data_size,
            deferred: Vec::new(),
        };
        ob.ensure(data_size);
        ob
    }

    /// Append raw bytes at an aligned position; returns their offset.
    pub fn write_bytes(&mut self, data: &[u8]) -> usize {
        if data.is_empty() {
            return 0;
        }
        self.align(ALIGNMENT);
        let offset = self.pos;
        self.grow(data.len());
        self.buf[self.pos..self.pos + data.len()].copy_from_slice(data);
        self.pos += data.len();
        offset
    }

    /// Append a string; returns its offset.
    pub fn write_text(&mut self, s: &str) -> usize {
        self.write_bytes(s.as_bytes())
    }

    /// Start a list with the given element stride.
    pub fn list(&mut self, elem_size: usize) -> ListBuilder<'_> {
        self.align(ALIGNMENT);
        let start = self.pos;
        ListBuilder {
            b: self,
            start,
            elem_size,
            count: 0,
        }
    }

    /// Finalize: patch root offset + size and return the message.
    pub fn finish(mut self) -> Vec<u8> {
        let root = self.root_offset as u32;
        let size = self.pos as u32;
        self.buf[8..12].copy_from_slice(&root.to_le_bytes());
        self.buf[12..16].copy_from_slice(&size.to_le_bytes());
        self.buf.truncate(self.pos);
        self.buf
    }

    /// Finalize with the header flags word set.
    pub fn finish_with_flags(mut self, flags: u16) -> Vec<u8> {
        self.buf[6..8].copy_from_slice(&flags.to_le_bytes());
        self.finish()
    }
}

/// Builds one struct inside a [`Builder`].
#[derive(Debug)]
pub struct ObjectBuilder<'b> {
    b: &'b mut Builder,
    start: usize,
    data_size: usize,
    deferred: Vec<(usize, Vec<u8>)>,
}

impl ObjectBuilder<'_> {
    fn ensure(&mut self, end: usize) {
        let needed = self.start + end;
        if needed > self.b.pos {
            self.b.grow(needed - self.b.pos);
            for i in self.b.pos..needed {
                self.b.buf[i] = 0;
            }
            self.b.pos = needed;
        }
    }

    pub fn set_bool(&mut self, field: usize, v: bool) {
        self.set_u8(field, u8::from(v));
    }

    pub fn set_u8(&mut self, field: usize, v: u8) {
        self.ensure(field + 1);
        self.b.buf[self.start + field] = v;
    }

    pub fn set_u16(&mut self, field: usize, v: u16) {
        self.ensure(field + 2);
        let p = self.start + field;
        self.b.buf[p..p + 2].copy_from_slice(&v.to_le_bytes());
    }

    pub fn set_u32(&mut self, field: usize, v: u32) {
        self.ensure(field + 4);
        let p = self.start + field;
        self.b.buf[p..p + 4].copy_from_slice(&v.to_le_bytes());
    }

    pub fn set_u64(&mut self, field: usize, v: u64) {
        self.ensure(field + 8);
        let p = self.start + field;
        self.b.buf[p..p + 8].copy_from_slice(&v.to_le_bytes());
    }

    pub fn set_i8(&mut self, field: usize, v: i8) {
        self.set_u8(field, v as u8);
    }
    pub fn set_i16(&mut self, field: usize, v: i16) {
        self.set_u16(field, v as u16);
    }
    pub fn set_i32(&mut self, field: usize, v: i32) {
        self.set_u32(field, v as u32);
    }
    pub fn set_i64(&mut self, field: usize, v: i64) {
        self.set_u64(field, v as u64);
    }
    pub fn set_f32(&mut self, field: usize, v: f32) {
        self.set_u32(field, v.to_bits());
    }
    pub fn set_f64(&mut self, field: usize, v: f64) {
        self.set_u64(field, v.to_bits());
    }

    pub fn set_text(&mut self, field: usize, v: &str) {
        self.set_bytes(field, v.as_bytes());
    }

    /// Set a bytes field. The payload is written after the fixed
    /// section by [`ObjectBuilder::finish`]; only the length is stored
    /// now, and the relative offset is patched then.
    pub fn set_bytes(&mut self, field: usize, v: &[u8]) {
        self.ensure(field + 8);
        let p = self.start + field;
        if v.is_empty() {
            self.b.buf[p..p + 8].fill(0);
            return;
        }
        self.deferred.push((field, v.to_vec()));
        let len = v.len() as u32;
        self.b.buf[p + 4..p + 8].copy_from_slice(&len.to_le_bytes());
    }

    /// Point `field` at an already-built nested object.
    pub fn set_object(&mut self, field: usize, obj_offset: usize) {
        self.ensure(field + 4);
        let p = self.start + field;
        if obj_offset == 0 {
            self.b.buf[p..p + 4].fill(0);
            return;
        }
        let rel = (obj_offset as i64 - p as i64) as i32;
        self.b.buf[p..p + 4].copy_from_slice(&rel.to_le_bytes());
    }

    /// Point `field` at an already-built list.
    pub fn set_list(&mut self, field: usize, list_offset: usize, length: usize) {
        self.ensure(field + 8);
        let p = self.start + field;
        if list_offset == 0 || length == 0 {
            self.b.buf[p..p + 8].fill(0);
            return;
        }
        let rel = (list_offset as i64 - p as i64) as i32;
        self.b.buf[p..p + 4].copy_from_slice(&rel.to_le_bytes());
        self.b.buf[p + 4..p + 8].copy_from_slice(&(length as u32).to_le_bytes());
    }

    fn flush(&mut self) -> usize {
        self.ensure(self.data_size);
        let deferred = std::mem::take(&mut self.deferred);
        for (field, data) in deferred {
            let data_pos = self.b.pos;
            self.b.grow(data.len());
            self.b.buf[data_pos..data_pos + data.len()].copy_from_slice(&data);
            self.b.pos += data.len();

            let field_abs = self.start + field;
            let rel = (data_pos as i64 - field_abs as i64) as i32;
            self.b.buf[field_abs..field_abs + 4].copy_from_slice(&rel.to_le_bytes());
        }
        self.start
    }

    /// Write the deferred payloads and patch their offsets; returns the
    /// object's position.
    pub fn finish(mut self) -> usize {
        self.flush()
    }

    /// [`ObjectBuilder::finish`], and make this object the message root.
    pub fn finish_as_root(mut self) -> usize {
        let offset = self.flush();
        self.b.root_offset = offset;
        offset
    }
}

/// Builds one list inside a [`Builder`].
#[derive(Debug)]
pub struct ListBuilder<'b> {
    b: &'b mut Builder,
    start: usize,
    elem_size: usize,
    count: usize,
}

impl ListBuilder<'_> {
    pub fn elem_size(&self) -> usize {
        self.elem_size
    }

    pub fn add_u8(&mut self, v: u8) {
        self.b.grow(1);
        self.b.buf[self.b.pos] = v;
        self.b.pos += 1;
        self.count += 1;
    }

    pub fn add_u32(&mut self, v: u32) {
        self.b.grow(4);
        let p = self.b.pos;
        self.b.buf[p..p + 4].copy_from_slice(&v.to_le_bytes());
        self.b.pos += 4;
        self.count += 1;
    }

    pub fn add_u64(&mut self, v: u64) {
        self.b.grow(8);
        let p = self.b.pos;
        self.b.buf[p..p + 8].copy_from_slice(&v.to_le_bytes());
        self.b.pos += 8;
        self.count += 1;
    }

    /// Append raw bytes (byte lists); each byte counts as an element.
    pub fn add_bytes(&mut self, data: &[u8]) {
        self.b.grow(data.len());
        let p = self.b.pos;
        self.b.buf[p..p + data.len()].copy_from_slice(data);
        self.b.pos += data.len();
        self.count += data.len();
    }

    /// Returns `(offset, length)` for [`ObjectBuilder::set_list`].
    pub fn finish(self) -> (usize, usize) {
        (self.start, self.count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_roundtrip() {
        let mut b = Builder::new(64);
        {
            let mut ob = b.object(24);
            ob.set_u32(0, 0xdead_beef);
            ob.set_u64(8, 42);
            ob.set_text(16, "hanzo");
            ob.finish_as_root();
        }
        let bytes = b.finish();

        let m = Message::parse(&bytes).unwrap();
        let r = m.root();
        assert_eq!(r.u32(0), 0xdead_beef);
        assert_eq!(r.u64(8), 42);
        assert_eq!(r.text(16), "hanzo");
        assert_eq!(m.version(), VERSION);
    }

    #[test]
    fn empty_bytes_is_a_null_pointer() {
        let mut b = Builder::new(64);
        {
            let mut ob = b.object(8);
            ob.set_bytes(0, b"");
            ob.finish_as_root();
        }
        let bytes = b.finish();
        assert_eq!(bytes.len(), HEADER_SIZE + 8);
        let m = Message::parse(&bytes).unwrap();
        assert!(m.root().bytes(0).is_empty());
    }

    #[test]
    fn list_roundtrip() {
        let mut b = Builder::new(64);
        let (off, len) = {
            let mut lb = b.list(4);
            lb.add_u32(1);
            lb.add_u32(2);
            lb.add_u32(3);
            lb.finish()
        };
        {
            let mut ob = b.object(8);
            ob.set_list(0, off, len);
            ob.finish_as_root();
        }
        let bytes = b.finish();
        let m = Message::parse(&bytes).unwrap();
        let l = m.root().list(0);
        assert_eq!(l.len(), 3);
        assert_eq!((l.u32(0), l.u32(1), l.u32(2)), (1, 2, 3));
    }

    /// A crafted backward pointer must not be able to alias the wire
    /// header and leak the version/flags words.
    #[test]
    fn rejects_backward_bytes_pointer() {
        let mut b = Builder::new(64);
        {
            let mut ob = b.object(8);
            ob.set_bytes(0, b"payload");
            ob.finish_as_root();
        }
        let mut bytes = b.finish();
        // Field cell sits at HEADER_SIZE; point it back at offset 0.
        let rel = -(HEADER_SIZE as i32);
        bytes[HEADER_SIZE..HEADER_SIZE + 4].copy_from_slice(&rel.to_le_bytes());
        let m = Message::parse(&bytes).unwrap();
        assert!(
            m.root().bytes(0).is_empty(),
            "header must not be readable as payload"
        );
    }

    /// length = 0xFFFFFFFF must not survive into a consumer loop.
    #[test]
    fn clamps_absurd_list_length() {
        let mut b = Builder::new(64);
        let (off, len) = {
            let mut lb = b.list(4);
            lb.add_u32(7);
            lb.finish()
        };
        {
            let mut ob = b.object(8);
            ob.set_list(0, off, len);
            ob.finish_as_root();
        }
        let mut bytes = b.finish();
        // The list elements were laid down first, so the root object does
        // NOT start at HEADER_SIZE — ask the message where it is rather
        // than assuming, or the patch lands on the wrong cell and the
        // test passes without exercising the clamp at all.
        let root = Message::parse(&bytes).unwrap().root().offset();
        bytes[root + 4..root + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        let m = Message::parse(&bytes).unwrap();
        assert_eq!(m.root().list(0).len(), 0);
    }

    #[test]
    fn rejects_size_smaller_than_header() {
        let mut bytes = vec![0u8; 32];
        bytes[0..4].copy_from_slice(&MAGIC);
        bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
        // size = 0
        assert!(Message::parse(&bytes).is_err());
    }
}
