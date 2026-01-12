//! DBN format decoding utilities
//!
//! Wraps databento's AsyncDbnDecoder for schema-specific decoding.
//! Most decoding is handled by the databento library itself.

use databento::dbn::{Mbp10Msg, OhlcvMsg, Schema, TradeMsg, decode::AsyncDbnDecoder};
use std::path::Path;

/// Decode OHLCV messages from a DBN file
pub async fn decode_ohlcv_from_file(
    path: impl AsRef<Path>,
) -> Result<Vec<OhlcvMsg>, databento::dbn::Error> {
    let mut decoder = AsyncDbnDecoder::from_zstd_file(path).await?;
    let mut messages = Vec::new();

    while let Some(msg) = decoder.decode_record::<OhlcvMsg>().await? {
        messages.push(msg.clone());
    }

    Ok(messages)
}

/// Decode Trade messages from a DBN file
pub async fn decode_trades_from_file(
    path: impl AsRef<Path>,
) -> Result<Vec<TradeMsg>, databento::dbn::Error> {
    let mut decoder = AsyncDbnDecoder::from_zstd_file(path).await?;
    let mut messages = Vec::new();

    while let Some(msg) = decoder.decode_record::<TradeMsg>().await? {
        messages.push(msg.clone());
    }

    Ok(messages)
}

/// Decode MBP-10 messages from a DBN file
pub async fn decode_mbp10_from_file(
    path: impl AsRef<Path>,
) -> Result<Vec<Mbp10Msg>, databento::dbn::Error> {
    let mut decoder = AsyncDbnDecoder::from_zstd_file(path).await?;
    let mut messages = Vec::new();

    while let Some(msg) = decoder.decode_record::<Mbp10Msg>().await? {
        messages.push(msg.clone());
    }

    Ok(messages)
}

/// Get schema from a DBN file
pub async fn get_schema_from_file(path: impl AsRef<Path>) -> Result<Schema, databento::dbn::Error> {
    let decoder = AsyncDbnDecoder::from_zstd_file(path).await?;
    decoder.metadata().schema.ok_or_else(|| {
        databento::dbn::Error::conversion::<String>("No schema found in DBN file metadata")
    })
}
