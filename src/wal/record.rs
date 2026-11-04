use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use crc32fast::Hasher;
use std::io::{self, Read, Write};
use crate::error::{Result, SuperbaseError};

pub const MAGIC_BYTES: [u8; 4] = [0x53, 0x55, 0x50, 0x42]; // "SUPB"
pub const HEADER_SIZE: usize = 4 + 8 + 8 + 1 + 4 + 4 + 4; // 33 bytes

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpType {
    Put = 1,
    Delete = 2,
}

impl TryFrom<u8> for OpType {
    type Error = SuperbaseError;
    fn try_from(v: u8) -> Result<Self> {
        match v {
            1 => Ok(OpType::Put),
            2 => Ok(OpType::Delete),
            _ => Err(SuperbaseError::Other(format!("Unknown OpType: {}", v))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WalRecord {
    pub seq: u64,
    pub timestamp: u64,
    pub op: OpType,
    pub key: String,
    pub value: Vec<u8>,
}

impl WalRecord {
    pub fn new_put(seq: u64, key: String, value: Vec<u8>) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            seq,
            timestamp,
            op: OpType::Put,
            key,
            value,
        }
    }

    pub fn new_delete(seq: u64, key: String) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            seq,
            timestamp,
            op: OpType::Delete,
            key,
            value: Vec::new(),
        }
    }

    /// Calculate CRC32 of payload (op + key + value)
    pub fn calculate_checksum(op: OpType, key: &[u8], value: &[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(&[op as u8]);
        hasher.update(key);
        hasher.update(value);
        hasher.finalize()
    }

    /// Encode record into bytes
    pub fn encode(&self) -> Vec<u8> {
        let key_bytes = self.key.as_bytes();
        let val_bytes = &self.value;
        let checksum = Self::calculate_checksum(self.op, key_bytes, val_bytes);

        let mut buf = Vec::with_capacity(HEADER_SIZE + key_bytes.len() + val_bytes.len());
        buf.write_all(&MAGIC_BYTES).expect("write magic");
        buf.write_u64::<BigEndian>(self.seq).expect("write seq");
        buf.write_u64::<BigEndian>(self.timestamp).expect("write timestamp");
        buf.write_u8(self.op as u8).expect("write op");
        buf.write_u32::<BigEndian>(key_bytes.len() as u32).expect("write key_len");
        buf.write_u32::<BigEndian>(val_bytes.len() as u32).expect("write val_len");
        buf.write_u32::<BigEndian>(checksum).expect("write checksum");

        buf.write_all(key_bytes).expect("write key");
        buf.write_all(val_bytes).expect("write val");
        buf
    }

    /// Decode record from stream or slice
    pub fn decode<R: Read>(reader: &mut R, current_offset: u64) -> Result<Option<Self>> {
        let mut magic = [0u8; 4];
        match reader.read_exact(&mut magic) {
            Ok(()) => {},
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(SuperbaseError::Io(e)),
        }

        if magic != MAGIC_BYTES {
            return Err(SuperbaseError::Other(format!(
                "Invalid magic header at offset {}: {:?}", current_offset, magic
            )));
        }

        let seq = reader.read_u64::<BigEndian>()?;
        let timestamp = reader.read_u64::<BigEndian>()?;
        let op_raw = reader.read_u8()?;
        let op = OpType::try_from(op_raw)?;
        let key_len = reader.read_u32::<BigEndian>()? as usize;
        let val_len = reader.read_u32::<BigEndian>()? as usize;
        let expected_checksum = reader.read_u32::<BigEndian>()?;

        let mut key_buf = vec![0u8; key_len];
        reader.read_exact(&mut key_buf)?;
        let key = String::from_utf8(key_buf)
            .map_err(|e| SuperbaseError::Other(format!("UTF-8 decode error: {}", e)))?;

        let mut val_buf = vec![0u8; val_len];
        reader.read_exact(&mut val_buf)?;

        let computed_checksum = Self::calculate_checksum(op, key.as_bytes(), &val_buf);
        if computed_checksum != expected_checksum {
            return Err(SuperbaseError::ChecksumMismatch(current_offset));
        }

        Ok(Some(Self {
            seq,
            timestamp,
            op,
            key,
            value: val_buf,
        }))
    }
}
