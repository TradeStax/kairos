use super::{TickerStats, Timeframe};
use crate::types::{Depth, Kline, OpenInterest, Trade};
use crate::{FuturesTicker, FuturesTickerInfo, FuturesVenue};
use crate::PushFrequency;

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

pub mod databento;
pub mod massive;

#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedStream {
    /// Streams that are persisted but needs to be resolved for use
    Waiting(Vec<PersistStreamKind>),
    /// Streams that are active and ready to use, but can't persist
    Ready(Vec<StreamKind>),
}

impl ResolvedStream {
    pub fn matches_stream(&self, stream: &StreamKind) -> bool {
        match self {
            ResolvedStream::Ready(existing) => existing.iter().any(|s| s == stream),
            _ => false,
        }
    }

    pub fn ready_iter_mut(&mut self) -> Option<impl Iterator<Item = &mut StreamKind>> {
        match self {
            ResolvedStream::Ready(streams) => Some(streams.iter_mut()),
            _ => None,
        }
    }

    pub fn ready_iter(&self) -> Option<impl Iterator<Item = &StreamKind>> {
        match self {
            ResolvedStream::Ready(streams) => Some(streams.iter()),
            _ => None,
        }
    }

    pub fn find_ready_map<F, T>(&self, f: F) -> Option<T>
    where
        F: FnMut(&StreamKind) -> Option<T>,
    {
        match self {
            ResolvedStream::Ready(streams) => streams.iter().find_map(f),
            _ => None,
        }
    }

    pub fn into_waiting(self) -> Vec<PersistStreamKind> {
        match self {
            ResolvedStream::Waiting(streams) => streams,
            ResolvedStream::Ready(streams) => streams
                .into_iter()
                .map(|s| match s {
                    StreamKind::DepthAndTrades {
                        ticker_info,
                        depth_aggr,
                        push_freq,
                    } => {
                        let persist_depth = PersistDepth {
                            ticker: ticker_info.ticker,
                            depth_aggr,
                            push_freq,
                        };
                        PersistStreamKind::DepthAndTrades(persist_depth)
                    }
                    StreamKind::Kline {
                        ticker_info,
                        timeframe,
                    } => {
                        let persist_kline = PersistKline {
                            ticker: ticker_info.ticker,
                            timeframe,
                        };
                        PersistStreamKind::Kline(persist_kline)
                    }
                })
                .collect(),
        }
    }

    pub fn waiting_to_resolve(&self) -> Option<&[PersistStreamKind]> {
        match self {
            ResolvedStream::Waiting(streams) => Some(streams),
            _ => None,
        }
    }

    pub fn ready_tickers(&self) -> Option<Vec<FuturesTickerInfo>> {
        match self {
            ResolvedStream::Ready(streams) => {
                Some(streams.iter().map(|s| s.ticker_info()).collect())
            }
            ResolvedStream::Waiting(_) => None,
        }
    }
}

impl IntoIterator for &ResolvedStream {
    type Item = StreamKind;
    type IntoIter = std::vec::IntoIter<StreamKind>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            ResolvedStream::Ready(streams) => streams.clone().into_iter(),
            ResolvedStream::Waiting(_) => vec![].into_iter(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum AdapterError {
    #[error("{0}")]
    FetchError(#[from] reqwest::Error),
    #[error("Parsing: {0}")]
    ParseError(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
}

impl AdapterError {
    pub fn to_user_message(&self) -> &'static str {
        match self {
            AdapterError::InvalidRequest(err) => {
                log::error!("Adapter invalid request: {err}");
                "Invalid request made to the exchange. Check logs for details."
            }
            AdapterError::FetchError(err) => {
                log::error!("Adapter fetch error: {err}");
                "Network error while contacting the exchange."
            }
            AdapterError::ParseError(err) => {
                log::error!("Adapter parse error: {err}");
                "Unexpected response from the exchange. Check logs for details."
            }
            AdapterError::ConnectionError(err) => {
                log::error!("Adapter connection error: {err}");
                "Connection error while communicating with the exchange."
            }
        }
    }
}

// MarketKind removed - futures markets don't use this crypto-specific concept

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum StreamKind {
    Kline {
        ticker_info: FuturesTickerInfo,
        timeframe: Timeframe,
    },
    DepthAndTrades {
        ticker_info: FuturesTickerInfo,
        #[serde(default = "default_depth_aggr")]
        depth_aggr: StreamTicksize,
        push_freq: PushFrequency,
    },
}

impl StreamKind {
    pub fn ticker_info(&self) -> FuturesTickerInfo {
        match self {
            StreamKind::Kline { ticker_info, .. }
            | StreamKind::DepthAndTrades { ticker_info, .. } => *ticker_info,
        }
    }

    pub fn as_depth_stream(&self) -> Option<(FuturesTickerInfo, StreamTicksize, PushFrequency)> {
        match self {
            StreamKind::DepthAndTrades {
                ticker_info,
                depth_aggr,
                push_freq,
            } => Some((*ticker_info, *depth_aggr, *push_freq)),
            _ => None,
        }
    }

    pub fn as_kline_stream(&self) -> Option<(FuturesTickerInfo, Timeframe)> {
        match self {
            StreamKind::Kline {
                ticker_info,
                timeframe,
            } => Some((*ticker_info, *timeframe)),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct UniqueStreams {
    // Simplified for futures - only one venue (CME Globex)
    streams: FxHashMap<FuturesTickerInfo, FxHashSet<StreamKind>>,
    specs: Option<StreamSpecs>,
}

impl UniqueStreams {
    pub fn from<'a>(streams: impl Iterator<Item = &'a StreamKind>) -> Self {
        let mut unique_streams = UniqueStreams::default();
        for stream in streams {
            unique_streams.add(*stream);
        }
        unique_streams
    }

    pub fn add(&mut self, stream: StreamKind) {
        let ticker_info = stream.ticker_info();

        self.streams.entry(ticker_info).or_default().insert(stream);

        self.update_specs();
    }

    pub fn extend<'a>(&mut self, streams: impl IntoIterator<Item = &'a StreamKind>) {
        for stream in streams {
            self.add(*stream);
        }
    }

    fn update_specs(&mut self) {
        let depth_streams = self.depth_streams();
        let kline_streams = self.kline_streams();

        self.specs = Some(StreamSpecs {
            depth: depth_streams,
            kline: kline_streams,
        });
    }

    pub fn depth_streams(&self) -> Vec<(FuturesTickerInfo, StreamTicksize, PushFrequency)> {
        self.streams
            .values()
            .flatten()
            .filter_map(|stream| stream.as_depth_stream())
            .collect()
    }

    pub fn kline_streams(&self) -> Vec<(FuturesTickerInfo, Timeframe)> {
        self.streams
            .values()
            .flatten()
            .filter_map(|stream| stream.as_kline_stream())
            .collect()
    }

    pub fn combined_used(&self) -> Option<&StreamSpecs> {
        self.specs.as_ref()
    }

    pub fn combined(&self) -> Option<&StreamSpecs> {
        self.specs.as_ref()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum PersistStreamKind {
    Kline(PersistKline),
    DepthAndTrades(PersistDepth),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PersistDepth {
    pub ticker: FuturesTicker,
    #[serde(default = "default_depth_aggr")]
    pub depth_aggr: StreamTicksize,
    #[serde(default = "default_push_freq")]
    pub push_freq: PushFrequency,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PersistKline {
    pub ticker: FuturesTicker,
    pub timeframe: Timeframe,
}

impl From<StreamKind> for PersistStreamKind {
    fn from(s: StreamKind) -> Self {
        match s {
            StreamKind::Kline {
                ticker_info,
                timeframe,
            } => PersistStreamKind::Kline(PersistKline {
                ticker: ticker_info.ticker,
                timeframe,
            }),
            StreamKind::DepthAndTrades {
                ticker_info,
                depth_aggr,
                push_freq,
            } => PersistStreamKind::DepthAndTrades(PersistDepth {
                ticker: ticker_info.ticker,
                depth_aggr,
                push_freq,
            }),
        }
    }
}

impl PersistStreamKind {
    /// Try to convert into runtime StreamKind. `resolver` should return Some(FuturesTickerInfo) for a ticker string,
    /// otherwise the conversion fails (so caller can trigger a refresh / fetch).
    pub fn into_stream_kind<F>(self, mut resolver: F) -> Result<StreamKind, String>
    where
        F: FnMut(&FuturesTicker) -> Option<FuturesTickerInfo>,
    {
        match self {
            PersistStreamKind::Kline(k) => resolver(&k.ticker)
                .map(|ti| StreamKind::Kline {
                    ticker_info: ti,
                    timeframe: k.timeframe,
                })
                .ok_or_else(|| format!("FuturesTickerInfo not found for {}", k.ticker)),
            PersistStreamKind::DepthAndTrades(d) => resolver(&d.ticker)
                .map(|ti| StreamKind::DepthAndTrades {
                    ticker_info: ti,
                    depth_aggr: d.depth_aggr,
                    push_freq: d.push_freq,
                })
                .ok_or_else(|| format!("FuturesTickerInfo not found for {}", d.ticker)),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum StreamTicksize {
    // ServerSide removed - tick multiplier only for crypto
    #[default]
    Client,
}

fn default_depth_aggr() -> StreamTicksize {
    StreamTicksize::Client
}

fn default_push_freq() -> PushFrequency {
    PushFrequency::ServerDefault
}

#[derive(Debug, Clone, Default)]
pub struct StreamSpecs {
    pub depth: Vec<(FuturesTickerInfo, StreamTicksize, PushFrequency)>,
    pub kline: Vec<(FuturesTickerInfo, Timeframe)>,
}

// ExchangeInclusive and Exchange enums removed - crypto-specific, not needed for futures

// Futures venues support all heatmap timeframes
pub fn supports_heatmap_timeframe(_tf: Timeframe) -> bool {
    true
}

/// Events for both historical data replay and live streaming
#[derive(Debug, Clone)]
pub enum Event {
    // ===== Historical Events =====
    /// Historical depth snapshot and trades for a specific time
    HistoricalDepth(u64, Arc<Depth>, Box<[Trade]>),
    /// Historical kline/candle data
    HistoricalKline(Kline),

    // ===== Live Streaming Events =====
    /// WebSocket connection established
    Connected(FuturesVenue),
    /// WebSocket connection closed
    Disconnected(FuturesVenue, String), // reason
    /// WebSocket connection lost (will attempt reconnection)
    ConnectionLost,
    /// Live depth snapshot with trades
    DepthReceived(StreamKind, u64, Arc<Depth>, Box<[Trade]>), // stream, timestamp, depth, trades
    /// Live kline update
    KlineReceived(StreamKind, Kline),
    /// Individual trade update (for real-time feed)
    TradeReceived(StreamKind, Trade),
}

#[derive(Debug, Clone, Hash)]
pub struct StreamConfig<I> {
    pub id: I,
    pub venue: FuturesVenue,
    // tick_mltp removed - only for crypto
    pub push_freq: PushFrequency,
}

impl<I> StreamConfig<I> {
    pub fn new(
        id: I,
        venue: FuturesVenue,
        push_freq: PushFrequency,
    ) -> Self {
        Self {
            id,
            venue,
            push_freq,
        }
    }
}

/// Fetch ticker info for continuous futures contracts (fast - no API calls needed)
pub async fn fetch_ticker_info(
    venue: FuturesVenue,
) -> Result<HashMap<FuturesTicker, Option<FuturesTickerInfo>>, AdapterError> {
    log::info!("=== fetch_ticker_info called for {:?} ===", venue);

    // Get ticker info instantly (no API call)
    let ticker_info = databento::mapper::get_continuous_ticker_info();
    log::info!(
        "=== fetch_ticker_info SUCCESS: {} tickers ===",
        ticker_info.len()
    );
    Ok(ticker_info)
}

pub async fn fetch_ticker_prices(
    venue: FuturesVenue,
) -> Result<HashMap<FuturesTicker, TickerStats>, AdapterError> {
    log::info!("=== fetch_ticker_prices called for {:?} ===", venue);

    // Get databento config
    let config = databento::DatabentoConfig::from_env().map_err(|e| {
        log::error!("DATABENTO_API_KEY not set: {:?}", e);
        AdapterError::InvalidRequest(format!("DATABENTO_API_KEY required: {:?}", e))
    })?;

    log::info!("Fetching REAL historical prices from databento (yesterday's data)");
    log::info!("This makes ONE batch API call for all 12 contracts - fast!");

    // Fetch REAL yesterday's prices from databento
    // Uses ONE API call for all 12 symbols (fast: 2-5 seconds)
    match databento::mapper::fetch_historical_prices(config, None).await {
        Ok(stats) => {
            log::info!(
                "=== fetch_ticker_prices SUCCESS: {} REAL prices from databento ===",
                stats.len()
            );
            Ok(stats)
        }
        Err(e) => {
            log::error!("=== fetch_ticker_prices FAILED: {:?} ===", e);
            log::error!(
                "NOTE: Databento historical API may have delay. Data available up to midnight UTC."
            );
            Err(AdapterError::ParseError(format!(
                "Databento historical price fetch failed: {:?}",
                e
            )))
        }
    }
}

pub async fn fetch_klines(
    ticker_info: FuturesTickerInfo,
    timeframe: Timeframe,
    range: Option<(u64, u64)>,
) -> Result<Vec<Kline>, AdapterError> {
    log::info!(
        "=== fetch_klines called for {} ({:?}) ===",
        ticker_info.ticker,
        timeframe
    );
    log::info!("Range provided: {:?}", range);

    // Require API key - no fallbacks
    let config = databento::DatabentoConfig::from_env().map_err(|e| {
        AdapterError::InvalidRequest(format!("DATABENTO_API_KEY required: {:?}", e))
    })?;

    let mut manager = databento::HistoricalDataManager::new(config)
        .await
        .map_err(|e| AdapterError::ParseError(format!("Failed to create manager: {:?}", e)))?;

    // Convert time range
    let (start, end) = if let Some((start_ms, end_ms)) = range {
        let start = chrono::DateTime::from_timestamp((start_ms / 1000) as i64, 0)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid start timestamp".to_string()))?
            .with_timezone(&chrono::Utc);
        let end = chrono::DateTime::from_timestamp((end_ms / 1000) as i64, 0)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid end timestamp".to_string()))?
            .with_timezone(&chrono::Utc);
        (start, end)
    } else {
        // Default: Last week (8 days ending 2 days before today)
        // This ensures we're within databento's historical data availability
        let now = chrono::Utc::now();
        let end_date = now.date_naive() - chrono::Duration::days(2);
        let start_date = end_date - chrono::Duration::days(8);

        let start = start_date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid start time".to_string()))?
            .and_utc();
        let end = end_date
            .and_hms_opt(23, 59, 59)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid end time".to_string()))?
            .and_utc();

        log::info!(
            "No range specified, using default: {} to {} (8 days of historical data)",
            start_date,
            end_date
        );
        (start, end)
    };

    let symbol = ticker_info.ticker.to_string();

    log::info!("Calling HistoricalDataManager.fetch_ohlcv:");
    log::info!("  Symbol: {}", symbol);
    log::info!("  Timeframe: {:?}", timeframe);
    log::info!("  Start: {}", start);
    log::info!("  End: {}", end);

    match manager.fetch_ohlcv(&symbol, timeframe, (start, end)).await {
        Ok(klines) => {
            log::info!("=== fetch_klines SUCCESS: {} bars ===", klines.len());
            Ok(klines)
        }
        Err(e) => {
            log::error!("=== fetch_klines FAILED: {:?} ===", e);
            log::error!(
                "Symbol: {}, Timeframe: {:?}, Range: {} to {}",
                symbol,
                timeframe,
                start,
                end
            );
            Err(AdapterError::ParseError(format!(
                "OHLCV fetch failed: {:?}",
                e
            )))
        }
    }
}

pub async fn fetch_trades(
    ticker_info: FuturesTickerInfo,
    range: Option<(u64, u64)>,
) -> Result<Vec<Trade>, AdapterError> {
    log::info!("=== fetch_trades called for {} ===", ticker_info.ticker);

    let config = databento::DatabentoConfig::from_env()
        .map_err(|e| AdapterError::ParseError(format!("Databento config error: {:?}", e)))?;

    let mut manager = databento::HistoricalDataManager::new(config)
        .await
        .map_err(|e| AdapterError::ParseError(format!("Failed to create manager: {:?}", e)))?;

    // Convert time range
    let (start, end) = if let Some((start_ms, end_ms)) = range {
        let start = chrono::DateTime::from_timestamp((start_ms / 1000) as i64, 0)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid start timestamp".to_string()))?
            .with_timezone(&chrono::Utc);
        let end = chrono::DateTime::from_timestamp((end_ms / 1000) as i64, 0)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid end timestamp".to_string()))?
            .with_timezone(&chrono::Utc);
        (start, end)
    } else {
        // Default: Last week
        let now = chrono::Utc::now();
        let end_date = now.date_naive() - chrono::Duration::days(2);
        let start_date = end_date - chrono::Duration::days(8);

        let start = start_date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid start time".to_string()))?
            .and_utc();
        let end = end_date
            .and_hms_opt(23, 59, 59)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid end time".to_string()))?
            .and_utc();

        (start, end)
    };

    let symbol = ticker_info.ticker.to_string();

    log::info!("Fetching trades: {} from {} to {}", symbol, start, end);

    match manager.fetch_trades(&symbol, (start, end)).await {
        Ok(trades) => {
            log::info!("=== fetch_trades SUCCESS: {} trades ===", trades.len());
            Ok(trades)
        }
        Err(e) => {
            log::error!("=== fetch_trades FAILED: {:?} ===", e);
            Err(AdapterError::ParseError(format!(
                "Trades fetch failed: {:?}",
                e
            )))
        }
    }
}

pub async fn fetch_open_interest(
    ticker: FuturesTicker,
    _timeframe: Timeframe,
    range: Option<(u64, u64)>,
) -> Result<Vec<OpenInterest>, AdapterError> {
    log::info!("=== fetch_open_interest called for {} ===", ticker);
    log::info!("Range provided: {:?}", range);

    // Require API key
    let config = databento::DatabentoConfig::from_env().map_err(|e| {
        AdapterError::InvalidRequest(format!("DATABENTO_API_KEY required: {:?}", e))
    })?;

    let mut manager = databento::HistoricalDataManager::new(config)
        .await
        .map_err(|e| AdapterError::ParseError(format!("Failed to create manager: {:?}", e)))?;

    // Convert time range
    let (start, end) = if let Some((start_ms, end_ms)) = range {
        let start = chrono::DateTime::from_timestamp((start_ms / 1000) as i64, 0)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid start timestamp".to_string()))?
            .with_timezone(&chrono::Utc);
        let end = chrono::DateTime::from_timestamp((end_ms / 1000) as i64, 0)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid end timestamp".to_string()))?
            .with_timezone(&chrono::Utc);
        (start, end)
    } else {
        // Default: Last 30 days of historical data
        let now = chrono::Utc::now();
        let end_date = now.date_naive() - chrono::Duration::days(2);
        let start_date = end_date - chrono::Duration::days(30);

        let start = start_date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid start time".to_string()))?
            .and_utc();
        let end = end_date
            .and_hms_opt(23, 59, 59)
            .ok_or_else(|| AdapterError::InvalidRequest("Invalid end time".to_string()))?
            .and_utc();

        log::info!(
            "No range specified, using default: {} to {} (30 days of historical data)",
            start_date,
            end_date
        );
        (start, end)
    };

    let symbol = ticker.as_str();

    log::info!("Calling HistoricalDataManager.fetch_open_interest:");
    log::info!("  Symbol: {}", symbol);
    log::info!("  Start: {}", start);
    log::info!("  End: {}", end);

    match manager.fetch_open_interest(symbol, (start, end)).await {
        Ok(oi_data) => {
            log::info!(
                "=== fetch_open_interest SUCCESS: {} data points ===",
                oi_data.len()
            );
            Ok(oi_data)
        }
        Err(e) => {
            log::error!("=== fetch_open_interest FAILED: {:?} ===", e);
            Err(AdapterError::ParseError(format!(
                "Open interest fetch failed: {:?}",
                e
            )))
        }
    }
}
