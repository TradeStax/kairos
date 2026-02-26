//! DBN format decoding utilities

use databento::dbn::{Mbp10Msg, Metadata, OhlcvMsg, Schema, TradeMsg, decode::AsyncDbnDecoder};
use std::path::Path;

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

pub async fn get_schema_from_file(path: impl AsRef<Path>) -> Result<Schema, databento::dbn::Error> {
    let decoder = AsyncDbnDecoder::from_zstd_file(path).await?;
    decoder
        .metadata()
        .schema
        .ok_or_else(|| databento::dbn::Error::conversion::<String>("No schema in DBN file"))
}
