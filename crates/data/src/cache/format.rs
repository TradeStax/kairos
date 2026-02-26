//! DayFile — per-day bincode+zstd serialization format
//!
//! Each file stores a header followed by a `Vec<T>` serialized with bincode
//! and compressed with zstd.

use serde::{Deserialize, Serialize};

const MAGIC: u32 = 0x4B414952; // "KAIR"
const FORMAT_VERSION: u8 = 1;

/// File header written before the payload
#[derive(Debug, Serialize, Deserialize)]
pub struct DayFileHeader {
    pub magic: u32,
    pub version: u8,
    pub schema: String,
    pub symbol: String,
    pub date: String, // YYYY-MM-DD
    pub record_count: u64,
}

impl DayFileHeader {
    pub fn new(
        schema: impl Into<String>,
        symbol: impl Into<String>,
        date: impl Into<String>,
        record_count: u64,
    ) -> Self {
        Self {
            magic: MAGIC,
            version: FORMAT_VERSION,
            schema: schema.into(),
            symbol: symbol.into(),
            date: date.into(),
            record_count,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.magic != MAGIC {
            return Err(format!(
                "Invalid magic number: expected {:#010x}, got {:#010x}",
                MAGIC, self.magic
            ));
        }
        if self.version != FORMAT_VERSION {
            return Err(format!("Unsupported format version: {}", self.version));
        }
        Ok(())
    }
}

/// Serialize a day's records to bincode+zstd bytes
pub fn encode<T: Serialize>(header: &DayFileHeader, records: &[T]) -> Result<Vec<u8>, String> {
    // Serialize header
    let header_bytes =
        bincode::serialize(header).map_err(|e| format!("Header serialization failed: {e}"))?;

    // Serialize records
    let records_bytes =
        bincode::serialize(records).map_err(|e| format!("Records serialization failed: {e}"))?;

    // Build raw payload: [4-byte header len][header][records]
    let mut raw = Vec::with_capacity(4 + header_bytes.len() + records_bytes.len());
    let header_len = header_bytes.len() as u32;
    raw.extend_from_slice(&header_len.to_le_bytes());
    raw.extend_from_slice(&header_bytes);
    raw.extend_from_slice(&records_bytes);

    // Compress with zstd
    zstd::encode_all(raw.as_slice(), 3).map_err(|e| format!("zstd compression failed: {e}"))
}

/// Deserialize a day's records from bincode+zstd bytes
pub fn decode<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
) -> Result<(DayFileHeader, Vec<T>), String> {
    // Decompress
    let raw = zstd::decode_all(bytes).map_err(|e| format!("zstd decompression failed: {e}"))?;

    if raw.len() < 4 {
        return Err("File too small to contain header length prefix".to_string());
    }

    // Extract header length
    let header_len = u32::from_le_bytes(
        raw[..4]
            .try_into()
            .map_err(|_| "Failed to read header length prefix".to_string())?,
    ) as usize;
    if raw.len() < 4 + header_len {
        return Err("File truncated before header end".to_string());
    }

    // Deserialize header
    let header: DayFileHeader = bincode::deserialize(&raw[4..4 + header_len])
        .map_err(|e| format!("Header deserialization failed: {e}"))?;
    header.validate()?;

    // Deserialize records
    let records: Vec<T> = bincode::deserialize(&raw[4 + header_len..])
        .map_err(|e| format!("Records deserialization failed: {e}"))?;

    Ok((header, records))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestRecord {
        time: u64,
        value: f32,
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let header = DayFileHeader::new("trades", "ES.c.0", "2025-01-15", 3);
        let records = vec![
            TestRecord {
                time: 1000,
                value: 100.0,
            },
            TestRecord {
                time: 2000,
                value: 101.5,
            },
            TestRecord {
                time: 3000,
                value: 99.25,
            },
        ];

        let encoded = encode(&header, &records).unwrap();
        let (decoded_header, decoded_records): (_, Vec<TestRecord>) = decode(&encoded).unwrap();

        assert_eq!(decoded_header.schema, "trades");
        assert_eq!(decoded_header.symbol, "ES.c.0");
        assert_eq!(decoded_header.record_count, 3);
        assert_eq!(decoded_records, records);
    }

    #[test]
    fn test_empty_records() {
        let header = DayFileHeader::new("trades", "NQ.c.0", "2025-01-01", 0);
        let records: Vec<TestRecord> = vec![];

        let encoded = encode(&header, &records).unwrap();
        let (_, decoded): (_, Vec<TestRecord>) = decode(&encoded).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_invalid_magic_rejected() {
        let header = DayFileHeader {
            magic: 0xDEADBEEF,
            version: 1,
            schema: "trades".into(),
            symbol: "ES.c.0".into(),
            date: "2025-01-01".into(),
            record_count: 0,
        };
        assert!(header.validate().is_err());
    }
}
