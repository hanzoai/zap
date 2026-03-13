//! ZAP binary wire protocol — transport-agnostic, matches TypeScript @zap-proto/zap.
//!
//! Wire format:
//!   [0x5A 0x41 0x50 0x01]  4 bytes  magic ("ZAP\x01")
//!   [type]                 1 byte   message type
//!   [length]               4 bytes  payload length (big-endian)
//!   [payload]              N bytes  JSON-encoded payload

use serde_json::Value;

/// ZAP magic bytes: "ZAP\x01"
pub const ZAP_MAGIC: [u8; 4] = [0x5a, 0x41, 0x50, 0x01];

/// Header: 4 magic + 1 type + 4 length = 9 bytes
pub const HEADER_SIZE: usize = 9;

/// Max payload: 16 MB
pub const MAX_PAYLOAD_SIZE: usize = 16 * 1024 * 1024;

/// Protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// ZAP message types (matches TypeScript MessageType)
pub mod msg {
    // Connection lifecycle
    pub const INIT: u8 = 0x01;
    pub const INIT_ACK: u8 = 0x02;

    // RPC (object-capability protocol)
    pub const PUSH: u8 = 0x10;
    pub const PULL: u8 = 0x11;
    pub const RESOLVE: u8 = 0x12;
    pub const REJECT: u8 = 0x13;
    pub const RELEASE: u8 = 0x14;

    // Tool operations (MCP compat)
    pub const LIST_TOOLS: u8 = 0x20;
    pub const LIST_TOOLS_RESPONSE: u8 = 0x21;
    pub const CALL_TOOL: u8 = 0x22;
    pub const CALL_TOOL_RESPONSE: u8 = 0x23;

    // Resource operations
    pub const LIST_RESOURCES: u8 = 0x30;
    pub const LIST_RESOURCES_RESPONSE: u8 = 0x31;
    pub const READ_RESOURCE: u8 = 0x32;
    pub const READ_RESOURCE_RESPONSE: u8 = 0x33;

    // Prompt operations
    pub const LIST_PROMPTS: u8 = 0x40;
    pub const LIST_PROMPTS_RESPONSE: u8 = 0x41;
    pub const GET_PROMPT: u8 = 0x42;
    pub const GET_PROMPT_RESPONSE: u8 = 0x43;

    // Control
    pub const PING: u8 = 0xf0;
    pub const PONG: u8 = 0xf1;
    pub const ERROR: u8 = 0xff;
}

/// Decoded ZAP frame
#[derive(Debug, Clone)]
pub struct ZapFrame {
    pub msg_type: u8,
    pub payload: Value,
}

/// Encode a ZAP message into binary frame.
pub fn encode(msg_type: u8, payload: &Value) -> Vec<u8> {
    let json_bytes = serde_json::to_vec(payload).unwrap_or_default();
    assert!(json_bytes.len() <= MAX_PAYLOAD_SIZE, "ZAP payload exceeds max size");

    let mut frame = Vec::with_capacity(HEADER_SIZE + json_bytes.len());
    frame.extend_from_slice(&ZAP_MAGIC);
    frame.push(msg_type);
    frame.extend_from_slice(&(json_bytes.len() as u32).to_be_bytes());
    frame.extend_from_slice(&json_bytes);
    frame
}

/// Decode a ZAP binary frame. Returns None if invalid.
pub fn decode(data: &[u8]) -> Option<ZapFrame> {
    if data.len() < HEADER_SIZE {
        return None;
    }
    if data[0..4] != ZAP_MAGIC {
        return None;
    }
    let msg_type = data[4];
    let length = u32::from_be_bytes([data[5], data[6], data[7], data[8]]) as usize;
    if length > MAX_PAYLOAD_SIZE || data.len() < HEADER_SIZE + length {
        return None;
    }
    let payload = if length > 0 {
        serde_json::from_slice(&data[HEADER_SIZE..HEADER_SIZE + length]).ok()?
    } else {
        Value::Null
    };
    Some(ZapFrame { msg_type, payload })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip() {
        let payload = json!({"method": "tools/list", "id": "r1"});
        let frame = encode(msg::PUSH, &payload);
        let decoded = decode(&frame).unwrap();
        assert_eq!(decoded.msg_type, msg::PUSH);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn magic_bytes() {
        let frame = encode(msg::PING, &json!({}));
        assert_eq!(&frame[0..4], &ZAP_MAGIC);
        assert_eq!(frame[4], msg::PING);
    }

    #[test]
    fn reject_short() {
        assert!(decode(&[0x5a, 0x41]).is_none());
    }

    #[test]
    fn reject_bad_magic() {
        let mut frame = encode(msg::PING, &json!({}));
        frame[0] = 0x00;
        assert!(decode(&frame).is_none());
    }
}
