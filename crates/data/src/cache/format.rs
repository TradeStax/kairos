//! Per-day bincode+zstd serialization format.
//!
//! Each cache file stores a [`DayFileHeader`] followed by a `Vec<T>` payload,
//! both serialized with bincode and compressed with zstd (level 3).

use serde::{Deserialize, Serialize};

/// Magic bytes identifying a Kairos cache file ("KAIR" in ASCII).
const MAGIC: u32 = 0x4B414952;

/// Current format version for forward-compatibility checks.
const FORMAT_VERSION: u8 = 1;

/// File header written before the compressed payload.
///
/// Contains metadata for validation and debugging: magic number, format
/// version, schema name, symbol, date, and record count.
#[derive(Debug, Serialize, Deserialize)]
pub struct DayFileHeader {
    /// Magic number for file identification
    pub magic: u32,
    /// Format version for compatibility checks
    pub version: u8,
    /// Data schema (e.g. "trades", "depth", "ohlcv")
    pub schema: String,
    /// Ticker symbol (e.g. "ES.c.0")
    pub symbol: String,
    /// ISO date string (YYYY-MM-DD)
    pub date: String,
    /// Number of records in the payload
    pub record_count: u64,
}

impl DayFileHeader {
    /// Creates a new header with the current magic number and format version
    #[must_use]
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

    /// Validates the magic number and format version
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

/// Serializes a day's records to bincode+zstd compressed bytes.
///
/// Wire format: `[4-byte header_len LE][header bytes][record bytes]`, then
/// zstd-compressed as a single frame.
pub fn encode<T: Serialize>(header: &DayFileHeader, records: &[T]) -> Result<Vec<u8>, String> {
    let header_bytes =
        bincode::serialize(header).map_err(|e| format!("Header serialization failed: {e}"))?;

    let records_bytes =
        bincode::serialize(records).map_err(|e| format!("Records serialization failed: {e}"))?;

    let mut raw = Vec::with_capacity(4 + header_bytes.len() + records_bytes.len());
    let header_len = header_bytes.len() as u32;
    raw.extend_from_slice(&header_len.to_le_bytes());
    raw.extend_from_slice(&header_bytes);
    raw.extend_from_slice(&records_bytes);

    zstd::encode_all(raw.as_slice(), 3).map_err(|e| format!("zstd compression failed: {e}"))
}

/// Deserializes a day's records from bincode+zstd compressed bytes.
///
/// Returns the header and decoded record vector. Validates the header
/// magic number and format version before returning.
pub fn decode<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
) -> Result<(DayFileHeader, Vec<T>), String> {
    let raw = zstd::decode_all(bytes).map_err(|e| format!("zstd decompression failed: {e}"))?;

    if raw.len() < 4 {
        return Err("File too small to contain header length prefix".to_string());
    }

    let header_len = u32::from_le_bytes(
        raw[..4]
            .try_into()
            .map_err(|_| "Failed to read header length prefix".to_string())?,
    ) as usize;
    if raw.len() < 4 + header_len {
        return Err("File truncated before header end".to_string());
    }

    let header: DayFileHeader = bincode::deserialize(&raw[4..4 + header_len])
        .map_err(|e| format!("Header deserialization failed: {e}"))?;
    header.validate()?;

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
