//! Rithmic Client
//!
//! Manages connections to Rithmic's Ticker Plant and History Plant.
//! Provides market data subscriptions and historical data retrieval.

use super::plants::{
    RithmicHistoryPlant, RithmicHistoryPlantHandle, RithmicTickerPlant,
    RithmicTickerPlantHandle,
};
use super::protocol::{
    RithmicConnectionConfig, RithmicEnv, RithmicMessage, RithmicResponse,
};
use super::protocol::response::RithmicReceiverApi;
use super::protocol::sender::RithmicSenderApi;
use super::protocol::ws::{ConnectStrategy, connect_with_strategy};
use super::RithmicConfig;
use super::RithmicError;
use futures_util::{SinkExt, StreamExt};
use kairos_data::feed::FeedStatus;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite;

/// Rithmic client wrapping ticker and history plants
pub struct RithmicClient {
    /// Handle for real-time market data
    ticker_handle: Option<RithmicTickerPlantHandle>,
    /// Handle for historical data
    history_handle: Option<RithmicHistoryPlantHandle>,
    /// Connection handles (kept alive to maintain connections)
    _ticker_plant: Option<RithmicTickerPlant>,
    _history_plant: Option<RithmicHistoryPlant>,
    /// Status update sender
    status_tx: mpsc::UnboundedSender<FeedStatus>,
    /// Configuration
    config: RithmicConfig,
}

impl RithmicClient {
    /// Create a new RithmicClient (not yet connected)
    pub fn new(
        config: RithmicConfig,
        status_tx: mpsc::UnboundedSender<FeedStatus>,
    ) -> Self {
        Self {
            ticker_handle: None,
            history_handle: None,
            _ticker_plant: None,
            _history_plant: None,
            status_tx,
            config,
        }
    }

    /// Connect to Rithmic ticker plant and history plant
    pub async fn connect(
        &mut self,
        rithmic_config: &RithmicConnectionConfig,
    ) -> Result<(), RithmicError> {
        let _ = self.status_tx.send(FeedStatus::Connecting);

        let strategy = self.config.connect_strategy;

        // Connect ticker plant
        let ticker_plant =
            RithmicTickerPlant::connect(rithmic_config, strategy)
                .await
                .map_err(|e| {
                    let msg =
                        format!("Failed to connect ticker plant: {}", e);
                    let _ =
                        self.status_tx.send(FeedStatus::Error(msg.clone()));
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

        // Connect history plant (clean up ticker plant on failure)
        let history_plant =
            match RithmicHistoryPlant::connect(rithmic_config, strategy)
                .await
            {
                Ok(plant) => plant,
                Err(e) => {
                    let msg = format!(
                        "Failed to connect history plant: {}",
                        e
                    );
                    let _ = self
                        .status_tx
                        .send(FeedStatus::Error(msg.clone()));
                    // Clean up already-connected ticker plant
                    log::warn!(
                        "Cleaning up ticker plant after history plant \
                         connection failure"
                    );
                    if let Err(dc_err) = ticker_handle.disconnect().await {
                        log::warn!(
                            "Ticker plant cleanup error: {}",
                            dc_err
                        );
                    }
                    return Err(RithmicError::Connection(msg));
                }
            };

        let history_handle = history_plant.get_handle();

        // Login to history plant (clean up on failure)
        if let Err(e) = history_handle.login().await {
            let msg = format!("History plant login failed: {}", e);
            let _ = self.status_tx.send(FeedStatus::Error(msg.clone()));
            if let Err(dc_err) = ticker_handle.disconnect().await {
                log::warn!("Ticker plant cleanup error: {}", dc_err);
            }
            return Err(RithmicError::Auth(msg));
        }

        log::info!("Rithmic history plant connected and authenticated");

        self._ticker_plant = Some(ticker_plant);
        self._history_plant = Some(history_plant);
        self.ticker_handle = Some(ticker_handle);
        self.history_handle = Some(history_handle);

        let _ = self.status_tx.send(FeedStatus::Connected);

        Ok(())
    }

    /// Subscribe to real-time market data for a symbol
    pub async fn subscribe(
        &mut self,
        symbol: &str,
        exchange: &str,
    ) -> Result<(), RithmicError> {
        let handle = self
            .ticker_handle
            .as_mut()
            .ok_or_else(|| {
                RithmicError::Connection("Not connected".to_string())
            })?;

        handle.subscribe(symbol, exchange).await.map_err(|e| {
            RithmicError::Subscription(format!(
                "Failed to subscribe {}: {}",
                symbol, e
            ))
        })?;

        log::info!(
            "Subscribed to Rithmic market data: {} on {}",
            symbol,
            exchange
        );
        Ok(())
    }

    /// Unsubscribe from real-time market data
    pub async fn unsubscribe(
        &mut self,
        symbol: &str,
        exchange: &str,
    ) -> Result<(), RithmicError> {
        let handle = self
            .ticker_handle
            .as_mut()
            .ok_or_else(|| {
                RithmicError::Connection("Not connected".to_string())
            })?;

        handle.unsubscribe(symbol, exchange).await.map_err(|e| {
            RithmicError::Subscription(format!(
                "Failed to unsubscribe {}: {}",
                symbol, e
            ))
        })?;

        log::info!(
            "Unsubscribed from Rithmic: {} on {}",
            symbol,
            exchange
        );
        Ok(())
    }

    /// Get the front-month contract for a product
    pub async fn get_front_month(
        &self,
        symbol: &str,
        exchange: &str,
    ) -> Result<String, RithmicError> {
        let handle = self.ticker_handle.as_ref().ok_or_else(|| {
            RithmicError::Connection("Not connected".to_string())
        })?;

        let response = handle
            .get_front_month_contract(symbol, exchange, false)
            .await
            .map_err(|e| {
                RithmicError::Data(format!(
                    "Failed to get front month for {}: {}",
                    symbol, e
                ))
            })?;

        if let Some(err) = &response.error {
            return Err(RithmicError::Data(format!(
                "Front month error: {}",
                err
            )));
        }

        // Extract the front month trading symbol from the response
        if let RithmicMessage::ResponseFrontMonthContract(fmc) =
            &response.message
        {
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

    /// Load historical ticks
    pub async fn load_ticks(
        &self,
        symbol: &str,
        exchange: &str,
        start_secs: i32,
        end_secs: i32,
    ) -> Result<Vec<RithmicResponse>, RithmicError> {
        let handle = self.history_handle.as_ref().ok_or_else(|| {
            RithmicError::Connection(
                "History plant not connected".to_string(),
            )
        })?;

        handle
            .load_ticks(
                symbol.to_string(),
                exchange.to_string(),
                start_secs,
                end_secs,
            )
            .await
            .map_err(|e| {
                RithmicError::Data(format!(
                    "Failed to load ticks for {}: {}",
                    symbol, e
                ))
            })
    }

    /// Fetch available product codes from an exchange
    pub async fn get_product_codes(
        &self,
        exchange: Option<&str>,
    ) -> Result<Vec<String>, RithmicError> {
        let handle = self.ticker_handle.as_ref().ok_or_else(|| {
            RithmicError::Connection("Not connected".to_string())
        })?;

        let responses = handle
            .get_product_codes(exchange, None)
            .await
            .map_err(|e| {
                RithmicError::Data(format!(
                    "Failed to get product codes: {}",
                    e
                ))
            })?;

        let codes: Vec<String> = responses
            .iter()
            .filter_map(|r| {
                if let super::protocol::RithmicMessage::ResponseProductCodes(
                    pc,
                ) = &r.message
                {
                    pc.product_code.clone()
                } else {
                    None
                }
            })
            .collect();

        Ok(codes)
    }

    /// Take the ticker handle's subscription receiver for streaming
    ///
    /// This consumes the receiver - only call once to create a streaming
    /// subscription.
    pub fn take_ticker_handle(
        &mut self,
    ) -> Option<RithmicTickerPlantHandle> {
        self.ticker_handle.take()
    }

    /// Disconnect from Rithmic
    pub async fn disconnect(&mut self) {
        if let Some(handle) = &self.ticker_handle
            && let Err(e) = handle.disconnect().await
        {
            log::warn!("Ticker plant disconnect error: {}", e);
        }
        if let Some(handle) = &self.history_handle
            && let Err(e) = handle.disconnect().await
        {
            log::warn!("History plant disconnect error: {}", e);
        }

        self.ticker_handle = None;
        self.history_handle = None;
        self._ticker_plant = None;
        self._history_plant = None;

        let _ = self.status_tx.send(FeedStatus::Disconnected);
        log::info!("Rithmic client disconnected");
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.ticker_handle.is_some() && self.history_handle.is_some()
    }
}

/// Probe a Rithmic server for available system names (pre-login).
///
/// Opens a WebSocket, sends `RequestRithmicSystemInfo`, reads the
/// response, and returns the list of `system_name` values.
/// Wrapped in a 10-second timeout.
pub async fn probe_system_names(
    server_url: &str,
) -> Result<Vec<String>, RithmicError> {
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

async fn probe_system_names_inner(
    server_url: &str,
) -> Result<Vec<String>, RithmicError> {
    // Open WS with simple (no retry) strategy
    let mut ws = connect_with_strategy(
        server_url,
        server_url,
        ConnectStrategy::Simple,
    )
    .await
    .map_err(|e| {
        RithmicError::Connection(format!(
            "Failed to connect to {}: {}",
            server_url, e
        ))
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
            RithmicError::Connection(format!(
                "Failed to send system info request: {}",
                e
            ))
        })?;

    // Read the response frame
    let receiver = RithmicReceiverApi {
        source: "probe".to_string(),
    };

    while let Some(msg) = ws.next().await {
        let msg = msg.map_err(|e| {
            RithmicError::Connection(format!(
                "WebSocket read error: {}",
                e
            ))
        })?;

        let data = match msg {
            tungstenite::Message::Binary(data) => data,
            tungstenite::Message::Close(_) => break,
            _ => continue,
        };

        let response = receiver
            .buf_to_message(data)
            .map_err(|e| {
                RithmicError::Data(format!(
                    "Failed to decode response: {:?}",
                    e.error
                ))
            })?;

        if let RithmicMessage::ResponseRithmicSystemInfo(info) =
            response.message
        {
            // Send close frame (best-effort)
            let _ = ws
                .send(tungstenite::Message::Close(None))
                .await;
            return Ok(info.system_name);
        }
    }

    Err(RithmicError::Data(
        "No system info response received".to_string(),
    ))
}
