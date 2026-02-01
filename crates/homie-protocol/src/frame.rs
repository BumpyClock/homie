use crate::ProtocolError;

/// Stream type indicators for binary PTY frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StreamType {
    Stdout = 0,
    Stderr = 1,
    Stdin = 2,
}

impl StreamType {
    pub fn from_u8(v: u8) -> Result<Self, ProtocolError> {
        match v {
            0 => Ok(Self::Stdout),
            1 => Ok(Self::Stderr),
            2 => Ok(Self::Stdin),
            _ => Err(ProtocolError::InvalidStreamType(v)),
        }
    }
}

/// Binary frame header size: 16 bytes session_id (UUID) + 1 byte stream_type.
pub const BINARY_HEADER_SIZE: usize = 17;

/// Binary frame layout for PTY data sent over WebSocket binary frames.
///
/// ```text
/// ┌──────────────────────────┬────────────┬──────────────────┐
/// │  session_id (16 bytes)   │ stream (1) │  payload (N)     │
/// │     UUID big-endian      │  0/1/2     │  raw PTY bytes   │
/// └──────────────────────────┴────────────┴──────────────────┘
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryFrame {
    /// PTY session identifier.
    pub session_id: uuid::Uuid,
    /// Which stream this data belongs to.
    pub stream: StreamType,
    /// Raw PTY bytes.
    pub payload: Vec<u8>,
}

impl BinaryFrame {
    /// Encode into bytes suitable for a WebSocket binary frame.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(BINARY_HEADER_SIZE + self.payload.len());
        buf.extend_from_slice(self.session_id.as_bytes());
        buf.push(self.stream as u8);
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Decode from raw WebSocket binary frame bytes.
    pub fn decode(data: &[u8]) -> Result<Self, ProtocolError> {
        if data.len() < BINARY_HEADER_SIZE {
            return Err(ProtocolError::FrameTooShort {
                expected: BINARY_HEADER_SIZE,
                got: data.len(),
            });
        }

        let session_id =
            uuid::Uuid::from_bytes(data[..16].try_into().expect("slice is exactly 16 bytes"));
        let stream = StreamType::from_u8(data[16])?;
        let payload = data[BINARY_HEADER_SIZE..].to_vec();

        Ok(Self {
            session_id,
            stream,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn roundtrip_stdout() {
        let frame = BinaryFrame {
            session_id: Uuid::new_v4(),
            stream: StreamType::Stdout,
            payload: b"hello world".to_vec(),
        };
        let encoded = frame.encode();
        let decoded = BinaryFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn roundtrip_stderr() {
        let frame = BinaryFrame {
            session_id: Uuid::new_v4(),
            stream: StreamType::Stderr,
            payload: b"error output".to_vec(),
        };
        let encoded = frame.encode();
        let decoded = BinaryFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn roundtrip_stdin() {
        let frame = BinaryFrame {
            session_id: Uuid::new_v4(),
            stream: StreamType::Stdin,
            payload: b"user input".to_vec(),
        };
        let encoded = frame.encode();
        let decoded = BinaryFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn roundtrip_empty_payload() {
        let frame = BinaryFrame {
            session_id: Uuid::new_v4(),
            stream: StreamType::Stdout,
            payload: vec![],
        };
        let encoded = frame.encode();
        assert_eq!(encoded.len(), BINARY_HEADER_SIZE);
        let decoded = BinaryFrame::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn decode_too_short() {
        let result = BinaryFrame::decode(&[0u8; 10]);
        assert!(result.is_err());
    }

    #[test]
    fn decode_invalid_stream_type() {
        let mut data = vec![0u8; BINARY_HEADER_SIZE];
        data[16] = 255; // invalid stream type
        let result = BinaryFrame::decode(&data);
        assert!(result.is_err());
    }

    #[test]
    fn header_size_is_17() {
        assert_eq!(BINARY_HEADER_SIZE, 17);
    }
}
