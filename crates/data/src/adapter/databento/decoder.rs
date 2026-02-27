//! DBN format decoding utilities.
//!
//! Reads `.dbn.zst` (zstandard-compressed DBN) files and extracts typed
//! records (`OhlcvMsg`, `TradeMsg`, `Mbp10Msg`). Each function validates
//! that the file's schema matches the expected record type.

use std::path::Path;

use databento::dbn::{Mbp10Msg, Metadata, OhlcvMsg, Schema, TradeMsg, decode::AsyncDbnDecoder};

/// Validates that the file metadata schema matches the expected schema
fn validate_schema(metadata: &Metadata, expected: Schema) -> Result<(), databento::dbn::Error> {
    if let Some(actual) = metadata.schema
        && actual != expected
    {
        return Err(databento::dbn::Error::conversion::<String>(format!(
            "Schema mismatch: file contains {:?} but expected {:?}",
            actual, expected
        )));
    }
    Ok(())
}

/// Decodes all OHLCV-1S records from a `.dbn.zst` file
pub async fn decode_ohlcv_from_file(
    path: impl AsRef<Path>,
) -> Result<Vec<OhlcvMsg>, databento::dbn::Error> {
    let mut decoder = AsyncDbnDecoder::from_zstd_file(path).await?;
    validate_schema(decoder.metadata(), Schema::Ohlcv1S)?;
    let mut messages = Vec::new();
    while let Some(msg) = decoder.decode_record::<OhlcvMsg>().await? {
        messages.push(msg.clone());
    }
    Ok(messages)
}

/// Decodes all trade records from a `.dbn.zst` file
pub async fn decode_trades_from_file(
    path: impl AsRef<Path>,
) -> Result<Vec<TradeMsg>, databento::dbn::Error> {
    let mut decoder = AsyncDbnDecoder::from_zstd_file(path).await?;
    validate_schema(decoder.metadata(), Schema::Trades)?;
    let mut messages = Vec::new();
    while let Some(msg) = decoder.decode_record::<TradeMsg>().await? {
        messages.push(msg.clone());
    }
    Ok(messages)
}

/// Decodes all MBP-10 depth records from a `.dbn.zst` file
pub async fn decode_mbp10_from_file(
    path: impl AsRef<Path>,
) -> Result<Vec<Mbp10Msg>, databento::dbn::Error> {
    let mut decoder = AsyncDbnDecoder::from_zstd_file(path).await?;
    validate_schema(decoder.metadata(), Schema::Mbp10)?;
    let mut messages = Vec::new();
    while let Some(msg) = decoder.decode_record::<Mbp10Msg>().await? {
        messages.push(msg.clone());
    }
    Ok(messages)
}

/// Reads the schema from a `.dbn.zst` file's metadata without decoding records
pub async fn get_schema_from_file(path: impl AsRef<Path>) -> Result<Schema, databento::dbn::Error> {
    let decoder = AsyncDbnDecoder::from_zstd_file(path).await?;
    decoder
        .metadata()
        .schema
        .ok_or_else(|| databento::dbn::Error::conversion::<String>("No schema in DBN file"))
}
