//! Rithmic client facade.
//!
//! [`RithmicClient`] manages connections to Rithmic's Ticker Plant (live
//! market data) and a pool of History Plant connections (historical data).
//! Provides subscribe/unsubscribe, front-month lookup, and paginated tick
//! loading behind a single API surface.

use super::RithmicConfig;
use super::RithmicError;
use super::plants::{RithmicHistoryPlantHandle, RithmicTickerPlant, RithmicTickerPlantHandle};
use super::protocol::response::RithmicReceiverApi;
use super::protocol::sender::RithmicSenderApi;
use super::protocol::ws::{ConnectStrategy, connect_with_strategy};
use super::protocol::{RithmicConnectionConfig, RithmicEnv, RithmicMessage, RithmicResponse};
use crate::connection::types::ConnectionStatus as FeedStatus;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite;

/// Top-level Rithmic client managing ticker and history plant connections.
///
/// Owns the underlying plant actors and exposes high-level methods for
/// subscribing to real-time data, querying front-month contracts, and
/// loading historical ticks. Send feed status updates through
/// `status_tx` so the UI can track connection state.
pub struct RithmicClient {
    /// Handle for real-time market data subscriptions
    ticker_handle: Option<RithmicTickerPlantHandle>,
    /// Pool of history plant connections for parallel fetching
    history_pool: Option<super::pool::HistoryPlantPool>,
    /// Kept alive to maintain the ticker plant WebSocket connection
    _ticker_plant: Option<RithmicTickerPlant>,
    /// Channel for emitting connection status updates
    status_tx: mpsc::UnboundedSender<FeedStatus>,
    /// Adapter configuration
    config: RithmicConfig,
}

impl RithmicClient {
    /// Creates a new client that is not yet connected.
    ///
    /// Call [`connect`](Self::connect) to establish WebSocket sessions.
    pub fn new(config: RithmicConfig, status_tx: mpsc::UnboundedSender<FeedStatus>) -> Self {
        Self {
            ticker_handle: None,
            history_pool: None,
            _ticker_plant: None,
            status_tx,
            config,
        }
    }

    /// Connects to the Rithmic ticker plant and history plant pool.
    ///
    /// Authenticates both plants. On history pool failure, the ticker
    /// plant is cleaned up before returning the error.
    pub async fn connect(
        &mut self,
        rithmic_config: &RithmicConnectionConfig,
    ) -> Result<(), RithmicError> {
        let _ = self.status_tx.send(FeedStatus::Connecting);

        let strategy = self.config.connect_strategy;

        // Connect ticker plant
        let ticker_plant = RithmicTickerPlant::connect(rithmic_config, strategy)
            .await
            .map_err(|e| {
                let msg = format!("Failed to connect ticker plant: {}", e);
                let _ = self.status_tx.send(FeedStatus::Error(msg.clone()));
                RithmicError::Connection(msg)
            })?;

        let ticker_handle = ticker_plant.get_handle();

        // Login to ticker plant
        ticker_handle.login().await.map_err(|e| {
            let msg = format!("Ticker plant login failed: {}", e);
            let _ = self.status_tx.send(FeedStatus::Error(msg.clone()));
            RithmicError::Auth(msg)
        })?;

        log::info!("Rithmic ticker plant connected and authenticated");

        // Connect history plant pool (clean up ticker on failure)
        let pool = match super::pool::HistoryPlantPool::connect(
            rithmic_config,
            strategy,
            self.config.history_pool_size,
        )
        .await
        {
            Ok(pool) => pool,
            Err(e) => {
                let msg = format!("Failed to connect history pool: {}", e);
                let _ = self.status_tx.send(FeedStatus::Error(msg.clone()));
                log::warn!(
                    "Cleaning up ticker plant after history pool \
                     connection failure"
                );
                if let Err(dc_err) = ticker_handle.disconnect().await {
                    log::warn!("Ticker plant cleanup error: {}", dc_err);
                }
                return Err(RithmicError::Connection(msg));
            }
        };

        log::info!("Rithmic history pool connected ({} plants)", pool.size(),);

        self._ticker_plant = Some(ticker_plant);
        self.ticker_handle = Some(ticker_handle);
        self.history_pool = Some(pool);

        let _ = self.status_tx.send(FeedStatus::Connected);

        Ok(())
    }

    /// Subscribes to real-time market data for a symbol on an exchange
    pub async fn subscribe(&mut self, symbol: &str, exchange: &str) -> Result<(), RithmicError> {
        let handle = self
            .ticker_handle
            .as_mut()
            .ok_or_else(|| RithmicError::Connection("Not connected".to_string()))?;

        handle.subscribe(symbol, exchange).await.map_err(|e| {
            RithmicError::Subscription(format!("Failed to subscribe {}: {}", symbol, e))
        })?;

        log::info!(
            "Subscribed to Rithmic market data: {} on {}",
            symbol,
            exchange
        );
        Ok(())
    }

    /// Unsubscribes from real-time market data for a symbol
    pub async fn unsubscribe(&mut self, symbol: &str, exchange: &str) -> Result<(), RithmicError> {
        let handle = self
            .ticker_handle
            .as_mut()
            .ok_or_else(|| RithmicError::Connection("Not connected".to_string()))?;

        handle.unsubscribe(symbol, exchange).await.map_err(|e| {
            RithmicError::Subscription(format!("Failed to unsubscribe {}: {}", symbol, e))
        })?;

        log::info!("Unsubscribed from Rithmic: {} on {}", symbol, exchange);
        Ok(())
    }

    /// Returns the front-month contract symbol for a product.
    ///
    /// Falls back to the input symbol if the response does not contain
    /// the expected `trading_symbol` or `symbol` field.
    pub async fn get_front_month(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<String, RithmicError> {
        let handle = self
            .ticker_handle
            .as_ref()
            .ok_or_else(|| RithmicError::Connection("Not connected".to_string()))?;

        let response = handle
            .get_front_month_contract(symbol, exchange, false)
            .await
            .map_err(|e| {
                RithmicError::Data(format!("Failed to get front month for {}: {}", symbol, e))
            })?;

        if let Some(err) = &response.error {
            return Err(RithmicError::Data(format!("Front month error: {}", err)));
        }

        // Extract the front month trading symbol from the response
        if let RithmicMessage::ResponseFrontMonthContract(fmc) = &response.message {
            if let Some(trading_symbol) = &fmc.trading_symbol {
                return Ok(trading_symbol.clone());
            }
            if let Some(symbol_name) = &fmc.symbol {
                return Ok(symbol_name.clone());
            }
        }

        // Fallback: return input symbol if response doesn't contain
        // expected data
        log::warn!(
            "Could not extract front month from response for {}, \
             using input symbol",
            symbol
        );
        Ok(symbol.to_string())
    }

    /// Load historical ticks with automatic pagination.
    ///
    /// Rithmic truncates responses at ~10000 bars. This method
    /// automatically sends `ResumeBars` requests to fetch all
    /// remaining data.
    pub async fn load_ticks(
        &self,
        symbol: &str,
        exchange: &str,
        start_secs: i32,
        end_secs: i32,
    ) -> Result<Vec<RithmicResponse>, RithmicError> {
        let pool = self
            .history_pool
            .as_ref()
            .ok_or_else(|| RithmicError::Connection("History pool not connected".to_string()))?;

        let handle = pool.acquire().await?;
        handle
            .load_ticks(symbol, exchange, start_secs, end_secs)
            .await
    }

    /// Returns a reference to the history plant pool for direct
    /// parallel access from the `DataEngine`
    pub fn history_pool(&self) -> Option<&super::pool::HistoryPlantPool> {
        self.history_pool.as_ref()
    }

    /// Fetches available product codes from an exchange
    pub async fn get_product_codes(
        &self,
        exchange: Option<&str>,
    ) -> Result<Vec<String>, RithmicError> {
        let handle = self
            .ticker_handle
            .as_ref()
            .ok_or_else(|| RithmicError::Connection("Not connected".to_string()))?;

        let responses = handle
            .get_product_codes(exchange, None)
            .await
            .map_err(|e| RithmicError::Data(format!("Failed to get product codes: {}", e)))?;

        let codes: Vec<String> = responses
            .iter()
            .filter_map(|r| {
                if let super::protocol::RithmicMessage::ResponseProductCodes(pc) = &r.message {
                    pc.product_code.clone()
                } else {
                    None
                }
            })
            .collect();

        Ok(codes)
    }

    /// Takes the ticker plant handle for streaming.
    ///
    /// This consumes the handle -- call only once to create a
    /// [`RithmicStream`](super::streaming::RithmicStream).
    pub fn take_ticker_handle(&mut self) -> Option<RithmicTickerPlantHandle> {
        self.ticker_handle.take()
    }

    /// Disconnects from all Rithmic plants and emits a status update
    pub async fn disconnect(&mut self) {
        if let Some(handle) = &self.ticker_handle
            && let Err(e) = handle.disconnect().await
        {
            log::warn!("Ticker plant disconnect error: {}", e);
        }

        // Drop the pool — this drops all plants and their
        // WebSocket connections
        self.history_pool = None;

        self.ticker_handle = None;
        self._ticker_plant = None;

        let _ = self.status_tx.send(FeedStatus::Disconnected);
        log::info!("Rithmic client disconnected");
    }

    /// Returns `true` if both the ticker handle and history pool are present
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.ticker_handle.is_some() && self.history_pool.is_some()
    }
}

/// Loads historical ticks with automatic pagination via a history
/// plant handle.
///
/// Tracks batch boundaries so that [`extract_resume_key`] only
/// examines the latest batch -- preventing stale resume keys from
/// earlier pages from being reused after the final page.
pub async fn load_ticks_paginated(
    handle: &RithmicHistoryPlantHandle,
    symbol: &str,
    exchange: &str,
    start_secs: i32,
    end_secs: i32,
) -> Result<Vec<RithmicResponse>, RithmicError> {
    let start_time = std::time::Instant::now();

    let mut all_responses = handle
        .load_ticks(
            symbol.to_string(),
            exchange.to_string(),
            start_secs,
            end_secs,
        )
        .await
        .map_err(|e| RithmicError::Data(format!("Failed to load ticks for {}: {}", symbol, e)))?;

    // Paginate: track batch boundaries so we only inspect the
    // latest batch for a resume key — prevents stale keys from
    // earlier pages from causing duplicate fetches.
    const MAX_PAGES: usize = 50; // safety limit
    let mut batch_start = 0;

    for page in 0..MAX_PAGES {
        let latest_batch = &all_responses[batch_start..];
        let Some(key) = extract_resume_key(latest_batch) else {
            break;
        };

        log::info!(
            "load_ticks: page {} — {} bars in batch, {} total, \
             resuming with key {}",
            page + 1,
            latest_batch.len(),
            all_responses.len(),
            key,
        );

        batch_start = all_responses.len();
        let resumed = handle.resume_bars(key).await.map_err(|e| {
            RithmicError::Data(format!("Failed to resume bars for {}: {}", symbol, e))
        })?;

        if resumed.is_empty() {
            break;
        }

        all_responses.extend(resumed);
    }

    let elapsed = start_time.elapsed();
    log::info!(
        "load_ticks: {} — {} responses in {:.2}s ({:.0} responses/s)",
        symbol,
        all_responses.len(),
        elapsed.as_secs_f64(),
        all_responses.len() as f64 / elapsed.as_secs_f64().max(0.001),
    );

    Ok(all_responses)
}

/// Checks if a batch was truncated and extracts the pagination
/// `request_key`.
///
/// Only examines the provided slice (should be the latest batch,
/// not all accumulated responses).
fn extract_resume_key(batch: &[RithmicResponse]) -> Option<String> {
    let data_count = batch.iter().filter(|r| r.has_more).count();

    // Search from end — the terminator or last data row carries
    // the resume key.
    for resp in batch.iter().rev() {
        if let RithmicMessage::ResponseTickBarReplay(bar) = &resp.message
            && bar.request_key.is_some()
        {
            log::debug!(
                "load_ticks: batch has request_key after {} data \
                 rows, more data available",
                data_count
            );
            return bar.request_key.clone();
        }
    }

    if data_count >= 5000 {
        log::warn!(
            "load_ticks: received {} data rows but no request_key \
             — resume_bars may not be enabled in the request",
            data_count
        );
    }

    None
}

/// Probes a Rithmic server for available system names (pre-login).
///
/// Opens a WebSocket, sends `RequestRithmicSystemInfo`, reads the
/// response, and returns the list of `system_name` values.
/// Wrapped in a 10-second timeout.
pub async fn probe_system_names(server_url: &str) -> Result<Vec<String>, RithmicError> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        probe_system_names_inner(server_url),
    )
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(RithmicError::Connection(
            "System info probe timed out after 10s".to_string(),
        )),
    }
}

async fn probe_system_names_inner(server_url: &str) -> Result<Vec<String>, RithmicError> {
    // Open WS with simple (no retry) strategy
    let mut ws = connect_with_strategy(server_url, server_url, ConnectStrategy::Simple)
        .await
        .map_err(|e| {
            RithmicError::Connection(format!("Failed to connect to {}: {}", server_url, e))
        })?;

    // Build a minimal sender (only needs message counter)
    let dummy_config = RithmicConnectionConfig {
        env: RithmicEnv::Demo,
        user: String::new(),
        password: String::new(),
        system_name: String::new(),
        url: server_url.to_string(),
        beta_url: server_url.to_string(),
        account_id: String::new(),
        fcm_id: String::new(),
        ib_id: String::new(),
    };
    let mut sender = RithmicSenderApi::new(&dummy_config);
    let (buf, _id) = sender.request_rithmic_system_info();

    // Send the request
    ws.send(tungstenite::Message::Binary(buf.into()))
        .await
        .map_err(|e| {
            RithmicError::Connection(format!("Failed to send system info request: {}", e))
        })?;

    // Read the response frame
    let receiver = RithmicReceiverApi {
        source: "probe".to_string(),
    };

    while let Some(msg) = ws.next().await {
        let msg =
            msg.map_err(|e| RithmicError::Connection(format!("WebSocket read error: {}", e)))?;

        let data = match msg {
            tungstenite::Message::Binary(data) => data,
            tungstenite::Message::Close(_) => break,
            _ => continue,
        };

        let response = receiver
            .buf_to_message(data)
            .map_err(|e| RithmicError::Data(format!("Failed to decode response: {:?}", e.error)))?;

        if let RithmicMessage::ResponseRithmicSystemInfo(info) = response.message {
            // Send close frame (best-effort)
            let _ = ws.send(tungstenite::Message::Close(None)).await;
            return Ok(info.system_name);
        }
    }

    Err(RithmicError::Data(
        "No system info response received".to_string(),
    ))
}
