//! `.zap` schema parser. Produces a `Schema` IR consumed by codegen.

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    Bool,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Int8,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
    Text,
    Data,
}

impl FieldType {
    /// Fixed primitive size in bytes. Variable-length types (Text/Data)
    /// occupy 8 bytes at the struct slot — i32 relative offset + u32
    /// length.
    pub fn slot_size(&self) -> usize {
        match self {
            FieldType::Bool | FieldType::UInt8 | FieldType::Int8 => 1,
            FieldType::UInt16 | FieldType::Int16 => 2,
            FieldType::UInt32 | FieldType::Int32 | FieldType::Float32 => 4,
            FieldType::UInt64 | FieldType::Int64 | FieldType::Float64 => 8,
            FieldType::Text | FieldType::Data => 8,
        }
    }

    pub fn rust_read_type(&self) -> &'static str {
        match self {
            FieldType::Bool => "bool",
            FieldType::UInt8 => "u8",
            FieldType::UInt16 => "u16",
            FieldType::UInt32 => "u32",
            FieldType::UInt64 => "u64",
            FieldType::Int8 => "i8",
            FieldType::Int16 => "i16",
            FieldType::Int32 => "i32",
            FieldType::Int64 => "i64",
            FieldType::Float32 => "f32",
            FieldType::Float64 => "f64",
            FieldType::Text => "&'a str",
            FieldType::Data => "&'a [u8]",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub index: u16,
    pub ty: FieldType,
    /// Byte offset within the struct's fixed slot region.
    pub offset: usize,
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<Field>,
    /// Total size of the fixed slot region, 8-byte aligned.
    pub slot_size: usize,
}

#[derive(Debug, Clone)]
pub struct Schema {
    pub file_id: u64,
    pub structs: Vec<Struct>,
}

/// Parse a `.zap` schema source string.
pub fn parse(src: &str) -> Result<Schema> {
    let mut p = Parser::new(src);
    p.skip_whitespace_and_comments();

    let file_id = p.parse_file_id()?;
    let mut structs = Vec::new();

    p.skip_whitespace_and_comments();
    while !p.eof() {
        if p.peek_keyword("struct") {
            structs.push(p.parse_struct()?);
        } else {
            return Err(Error::Parse(
                p.pos_string(),
                format!("expected `struct`, got {:?}", p.rest_preview()),
            ));
        }
        p.skip_whitespace_and_comments();
    }

    Ok(Schema { file_id, structs })
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn rest(&self) -> &str {
        &self.src[self.pos..]
    }

    fn rest_preview(&self) -> String {
        let r = self.rest();
        let n = r.chars().take(20).count();
        let prefix: String = r.chars().take(n).collect();
        prefix
    }

    fn pos_string(&self) -> String {
        let line = self.src[..self.pos].matches('\n').count() + 1;
        let col = self.src[..self.pos]
            .rfind('\n')
            .map(|i| self.pos - i - 1)
            .unwrap_or(self.pos)
            + 1;
        format!("line {} col {}", line, col)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            let r = self.rest();
            let mut consumed = 0;
            for c in r.chars() {
                if c.is_whitespace() {
                    consumed += c.len_utf8();
                } else {
                    break;
                }
            }
            self.pos += consumed;
            // Comments: `#` to EOL, or `//` to EOL.
            let r = self.rest();
            if r.starts_with('#') || r.starts_with("//") {
                if let Some(nl) = r.find('\n') {
                    self.pos += nl + 1;
                    continue;
                } else {
                    self.pos = self.src.len();
                    break;
                }
            }
            break;
        }
    }

    fn peek_keyword(&self, kw: &str) -> bool {
        let r = self.rest();
        r.starts_with(kw)
            && r[kw.len()..]
                .chars()
                .next()
                .map(|c| !c.is_alphanumeric() && c != '_')
                .unwrap_or(true)
    }

    fn expect(&mut self, lit: &str) -> Result<()> {
        if self.rest().starts_with(lit) {
            self.pos += lit.len();
            Ok(())
        } else {
            Err(Error::Parse(
                self.pos_string(),
                format!("expected {:?}, got {:?}", lit, self.rest_preview()),
            ))
        }
    }

    fn consume_ident(&mut self) -> Result<String> {
        let r = self.rest();
        let mut end = 0;
        for (i, c) in r.char_indices() {
            if i == 0 && !(c.is_ascii_alphabetic() || c == '_') {
                break;
            }
            if !(c.is_ascii_alphanumeric() || c == '_') {
                break;
            }
            end = i + c.len_utf8();
        }
        if end == 0 {
            return Err(Error::Parse(
                self.pos_string(),
                "expected identifier".into(),
            ));
        }
        let s = r[..end].to_string();
        self.pos += end;
        Ok(s)
    }

    fn parse_file_id(&mut self) -> Result<u64> {
        self.expect("@")?;
        let r = self.rest();
        if !r.starts_with("0x") {
            return Err(Error::Parse(
                self.pos_string(),
                "file id needs 0x prefix".into(),
            ));
        }
        self.pos += 2;
        let r = self.rest();
        let mut end = 0;
        for (i, c) in r.char_indices() {
            if c.is_ascii_hexdigit() {
                end = i + 1;
            } else {
                break;
            }
        }
        if end == 0 {
            return Err(Error::Parse(
                self.pos_string(),
                "expected hex digits for file id".into(),
            ));
        }
        let id = u64::from_str_radix(&r[..end], 16)
            .map_err(|e| Error::Parse(self.pos_string(), e.to_string()))?;
        self.pos += end;
        self.skip_whitespace_and_comments();
        self.expect(";")?;
        Ok(id)
    }

    fn parse_struct(&mut self) -> Result<Struct> {
        self.expect("struct")?;
        self.skip_whitespace_and_comments();
        let name = self.consume_ident()?;
        self.skip_whitespace_and_comments();
        self.expect("{")?;
        self.skip_whitespace_and_comments();

        let mut fields = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.rest().starts_with('}') {
                self.pos += 1;
                break;
            }
            fields.push(self.parse_field()?);
        }

        // Pack: assign offsets in declaration order with primitive
        // alignment, then 8-byte align the whole struct slot region.
        let mut offset = 0usize;
        for f in fields.iter_mut() {
            let sz = f.ty.slot_size();
            let align = sz.min(8).max(1);
            offset = (offset + align - 1) & !(align - 1);
            f.offset = offset;
            offset += sz;
        }
        let slot_size = (offset + 7) & !7;

        Ok(Struct {
            name,
            fields,
            slot_size,
        })
    }

    fn parse_field(&mut self) -> Result<Field> {
        let name = self.consume_ident()?;
        self.skip_whitespace_and_comments();
        self.expect("@")?;
        let r = self.rest();
        let mut end = 0;
        for (i, c) in r.char_indices() {
            if c.is_ascii_digit() {
                end = i + 1;
            } else {
                break;
            }
        }
        if end == 0 {
            return Err(Error::Parse(
                self.pos_string(),
                "expected field index".into(),
            ));
        }
        let index: u16 = r[..end]
            .parse()
            .map_err(|e: std::num::ParseIntError| Error::Parse(self.pos_string(), e.to_string()))?;
        self.pos += end;
        self.skip_whitespace_and_comments();
        self.expect(":")?;
        self.skip_whitespace_and_comments();

        let ty_name = self.consume_ident()?;
        let ty = match ty_name.as_str() {
            "Bool" => FieldType::Bool,
            "UInt8" => FieldType::UInt8,
            "UInt16" => FieldType::UInt16,
            "UInt32" => FieldType::UInt32,
            "UInt64" => FieldType::UInt64,
            "Int8" => FieldType::Int8,
            "Int16" => FieldType::Int16,
            "Int32" => FieldType::Int32,
            "Int64" => FieldType::Int64,
            "Float32" => FieldType::Float32,
            "Float64" => FieldType::Float64,
            "Text" => FieldType::Text,
            "Data" => FieldType::Data,
            other => {
                return Err(Error::Parse(
                    self.pos_string(),
                    format!("unsupported field type: {}", other),
                ))
            }
        };
        self.skip_whitespace_and_comments();
        self.expect(";")?;

        Ok(Field {
            name,
            index,
            ty,
            offset: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_schema() {
        let src = r#"
            @0xc0a8b5e0e5f0a000;
            struct Ping { nonce @0 :UInt64; }
        "#;
        let s = parse(src).unwrap();
        assert_eq!(s.file_id, 0xc0a8b5e0e5f0a000);
        assert_eq!(s.structs.len(), 1);
        assert_eq!(s.structs[0].name, "Ping");
        assert_eq!(s.structs[0].fields[0].name, "nonce");
        assert_eq!(s.structs[0].fields[0].ty, FieldType::UInt64);
        assert_eq!(s.structs[0].fields[0].offset, 0);
        assert_eq!(s.structs[0].slot_size, 8);
    }

    #[test]
    fn aligns_fields_then_pads_struct_to_8() {
        let src = r#"
            @0x1;
            struct M {
                a @0 :UInt8;
                b @1 :UInt32;
                c @2 :Text;
            }
        "#;
        let s = parse(src).unwrap();
        let m = &s.structs[0];
        assert_eq!(m.fields[0].offset, 0); // u8 at 0
        assert_eq!(m.fields[1].offset, 4); // u32 aligned to 4
        assert_eq!(m.fields[2].offset, 8); // text slot at 8
        assert_eq!(m.slot_size, 16); // 8-byte aligned total
    }
}
