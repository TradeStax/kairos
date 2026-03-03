//! Rithmic response decoding and the [`RithmicResponse`] wrapper type.
//!
//! [`RithmicReceiverApi`] decodes raw protobuf bytes from the WebSocket
//! into typed [`RithmicResponse`] structs, handling template ID dispatch,
//! error extraction, and multi-response detection.

use log::error;
use prost::{Message, bytes::Bytes};

use super::messages::RithmicMessage;
use super::rti::{
    AccountListUpdates, AccountPnLPositionUpdate, AccountRmsUpdates, BestBidOffer, BracketUpdates,
    DepthByOrder, DepthByOrderEndEvent, EndOfDayPrices, ExchangeOrderNotification, ForcedLogout,
    FrontMonthContractUpdate, IndicatorPrices, InstrumentPnLPositionUpdate, LastTrade, MarketMode,
    MessageType, OpenInterest, OrderBook, OrderPriceLimits, QuoteStatistics, Reject,
    ResponseAcceptAgreement, ResponseAccountList, ResponseAccountRmsInfo,
    ResponseAccountRmsUpdates, ResponseAuxilliaryReferenceData, ResponseBracketOrder,
    ResponseCancelAllOrders, ResponseCancelOrder, ResponseDepthByOrderSnapshot,
    ResponseDepthByOrderUpdates, ResponseEasyToBorrowList, ResponseExitPosition,
    ResponseFrontMonthContract, ResponseGetInstrumentByUnderlying,
    ResponseGetInstrumentByUnderlyingKeys, ResponseGetVolumeAtPrice, ResponseGiveTickSizeTypeTable,
    ResponseHeartbeat, ResponseLinkOrders, ResponseListAcceptedAgreements,
    ResponseListExchangePermissions, ResponseListUnacceptedAgreements, ResponseLogin,
    ResponseLoginInfo, ResponseLogout, ResponseMarketDataUpdate,
    ResponseMarketDataUpdateByUnderlying, ResponseModifyOrder, ResponseModifyOrderReferenceData,
    ResponseNewOrder, ResponseOcoOrder, ResponseOrderSessionConfig, ResponsePnLPositionSnapshot,
    ResponsePnLPositionUpdates, ResponseProductCodes, ResponseProductRmsInfo,
    ResponseReferenceData, ResponseReplayExecutions, ResponseResumeBars,
    ResponseRithmicSystemGatewayInfo, ResponseRithmicSystemInfo, ResponseSearchSymbols,
    ResponseSetRithmicMrktDataSelfCertStatus, ResponseShowAgreement, ResponseShowBracketStops,
    ResponseShowBrackets, ResponseShowOrderHistory, ResponseShowOrderHistoryDates,
    ResponseShowOrderHistoryDetail, ResponseShowOrderHistorySummary, ResponseShowOrders,
    ResponseSubscribeForOrderUpdates, ResponseSubscribeToBracketUpdates, ResponseTickBarReplay,
    ResponseTickBarUpdate, ResponseTimeBarReplay, ResponseTimeBarUpdate, ResponseTradeRoutes,
    ResponseUpdateStopBracketLevel, ResponseUpdateTargetBracketLevel,
    ResponseVolumeProfileMinuteBars, RithmicOrderNotification, SymbolMarginRate, TickBar, TimeBar,
    TradeRoute, TradeStatistics, UpdateEasyToBorrowList, UserAccountUpdate,
};

/// Response from a Rithmic plant, either from a request or a subscription update.
///
/// This structure wraps all messages received from Rithmic plants, including both
/// request-response messages and subscription updates (like market data, order updates, etc.).
///
/// ## Fields
///
/// - `request_id`: Unique identifier for matching responses to requests. Empty for updates.
/// - `message`: The actual Rithmic message data (see [`RithmicMessage`])
/// - `is_update`: `true` if this is a subscription update, `false` if it's a request response
/// - `has_more`: `true` if more responses are coming for this request
/// - `multi_response`: `true` if this request type can return multiple responses
/// - `error`: Error message if the operation failed or a connection error occurred
/// - `source`: Name of the plant that sent this response (e.g., "ticker_plant", "order_plant")
///
/// ## Error Handling
///
/// The `error` field is populated in two scenarios:
///
/// ### 1. Rithmic Protocol Errors
/// When Rithmic rejects a request or encounters an error, the response will have:
/// - `error: Some("error description from Rithmic")`
/// - `message`: Usually [`RithmicMessage::Reject`]
///
/// ### 2. Connection Errors
/// When a plant's WebSocket connection fails, you'll receive:
/// - `message: RithmicMessage::ConnectionError`
/// - `error: Some("WebSocket error description")`
/// - `is_update: true` (routed to subscription channel)
/// - The plant has stopped and the channel will close
///
/// See [`RithmicMessage::ConnectionError`] for detailed error handling guidance.
///
/// ## Example: Handling Errors
///
/// ```ignore
/// # fn handle_response(response: RithmicResponse) {
/// match response.message {
///     RithmicMessage::ConnectionError => {
///         // WebSocket connection failed
///         eprintln!(
///             "Connection error from {}: {}",
///             response.source,
///             response.error.as_ref().unwrap()
///         );
///         // Implement reconnection logic
///     }
///     RithmicMessage::Reject(reject) => {
///         // Rithmic rejected a request
///         eprintln!(
///             "Request rejected: {}",
///             response.error.as_ref().unwrap_or(&"Unknown".to_string())
///         );
///     }
///     _ => {
///         // Check error field even for successful-looking messages
///         if let Some(err) = response.error {
///             eprintln!("Error in {}: {}", response.source, err);
///         }
///     }
/// }
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct RithmicResponse {
    pub request_id: String,
    /// Boxed to reduce enum size — `RithmicMessage` has many large variants.
    pub message: Box<RithmicMessage>,
    pub is_update: bool,
    pub has_more: bool,
    pub multi_response: bool,
    pub error: Option<String>,
    pub source: String,
}

impl RithmicResponse {
    /// Returns true if this response represents an error condition.
    ///
    /// This checks both:
    /// - The `error` field being set (Rithmic protocol errors)
    /// - Connection issues (WebSocket errors, heartbeat timeouts, forced logout)
    ///
    /// # Example
    /// ```ignore
    /// if response.is_error() {
    ///     eprintln!("Error: {:?}", response.error);
    /// }
    /// ```
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.error.is_some() || self.is_connection_issue()
    }

    /// Returns true if this response indicates a connection health issue.
    ///
    /// Connection issues include:
    /// - `ConnectionError`: WebSocket connection failed
    /// - `HeartbeatTimeout`: Connection appears dead
    /// - `ForcedLogout`: Server forcibly logged out the client
    ///
    /// These conditions typically require reconnection logic.
    ///
    /// # Example
    /// ```ignore
    /// if response.is_connection_issue() {
    ///     // Trigger reconnection
    /// }
    /// ```
    #[must_use]
    pub fn is_connection_issue(&self) -> bool {
        matches!(
            *self.message,
            RithmicMessage::ConnectionError
                | RithmicMessage::HeartbeatTimeout
                | RithmicMessage::ForcedLogout(_)
        )
    }

    /// Returns true if this response contains market data.
    ///
    /// Market data messages include:
    /// - `BestBidOffer`: Top-of-book quotes
    /// - `LastTrade`: Trade executions
    /// - `DepthByOrder`: Order book depth updates
    /// - `DepthByOrderEndEvent`: End of depth snapshot marker
    /// - `OrderBook`: Aggregated order book
    ///
    /// # Example
    /// ```ignore
    /// if response.is_market_data() {
    ///     // Process market data update
    /// }
    /// ```
    #[must_use]
    pub fn is_market_data(&self) -> bool {
        matches!(
            *self.message,
            RithmicMessage::BestBidOffer(_)
                | RithmicMessage::LastTrade(_)
                | RithmicMessage::DepthByOrder(_)
                | RithmicMessage::DepthByOrderEndEvent(_)
                | RithmicMessage::OrderBook(_)
        )
    }

    /// Returns true if this response is an order update notification.
    ///
    /// Order update messages include:
    /// - `RithmicOrderNotification`: Order status updates from Rithmic
    /// - `ExchangeOrderNotification`: Order status updates from exchange
    /// - `BracketUpdates`: Bracket order updates
    ///
    /// # Example
    /// ```ignore
    /// if response.is_order_update() {
    ///     // Process order status change
    /// }
    /// ```
    #[must_use]
    pub fn is_order_update(&self) -> bool {
        matches!(
            *self.message,
            RithmicMessage::RithmicOrderNotification(_)
                | RithmicMessage::ExchangeOrderNotification(_)
                | RithmicMessage::BracketUpdates(_)
        )
    }

    /// Returns true if this response is a P&L or position update.
    ///
    /// P&L update messages include:
    /// - `AccountPnLPositionUpdate`: Account-level P&L updates
    /// - `InstrumentPnLPositionUpdate`: Per-instrument P&L updates
    ///
    /// # Example
    /// ```ignore
    /// if response.is_pnl_update() {
    ///     // Update position tracking
    /// }
    /// ```
    #[must_use]
    pub fn is_pnl_update(&self) -> bool {
        matches!(
            *self.message,
            RithmicMessage::AccountPnLPositionUpdate(_)
                | RithmicMessage::InstrumentPnLPositionUpdate(_)
        )
    }
}

/// Decodes a protobuf message type, returning an error response on failure.
macro_rules! decode_or_err {
    ($ty:ty, $payload:expr, $source:expr) => {
        match <$ty>::decode($payload) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to decode {}: {}", stringify!($ty), e);
                return Err(RithmicResponse {
                    message: Box::new(RithmicMessage::Unknown),
                    error: Some(format!("Decode error for {}: {}", stringify!($ty), e)),
                    request_id: String::new(),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    source: $source.clone(),
                });
            }
        }
    };
}

/// Decodes raw protobuf bytes from the WebSocket into typed responses.
///
/// Dispatches on the protobuf `template_id` to determine which message
/// type to decode, extracts error codes, and wraps everything in a
/// [`RithmicResponse`].
#[derive(Debug)]
pub(crate) struct RithmicReceiverApi {
    /// Plant name used in response `source` field (e.g. "ticker_plant")
    pub(crate) source: String,
}

impl RithmicReceiverApi {
    /// Decodes a raw protobuf frame into a typed [`RithmicResponse`].
    ///
    /// Returns `Err(RithmicResponse)` when the frame cannot be decoded
    /// (the error response still carries useful routing information).
    #[allow(clippy::result_large_err)]
    pub(crate) fn buf_to_message(&self, data: Bytes) -> Result<RithmicResponse, RithmicResponse> {
        let Some(payload) = data.get(4..) else {
            return Err(RithmicResponse {
                message: Box::new(RithmicMessage::Unknown),
                error: Some(format!("Frame too short: {} bytes", data.len())),
                request_id: String::new(),
                is_update: false,
                has_more: false,
                multi_response: false,
                source: self.source.clone(),
            });
        };

        let parsed_message = match MessageType::decode(payload) {
            Ok(msg) => msg,
            Err(e) => {
                error!(
                    "Failed to decode MessageType: {} - data_size: {} bytes",
                    e,
                    data.len()
                );
                return Err(RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::Unknown),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error: Some(format!("Failed to decode message: {}", e)),
                    source: self.source.clone(),
                });
            }
        };

        let response = match parsed_message.template_id {
            11 => {
                let resp = decode_or_err!(ResponseLogin, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseLogin(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            13 => {
                let resp = decode_or_err!(ResponseLogout, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseLogout(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            15 => {
                let resp = decode_or_err!(ResponseReferenceData, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseReferenceData(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            17 => {
                let resp = decode_or_err!(ResponseRithmicSystemInfo, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseRithmicSystemInfo(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            19 => {
                let resp = decode_or_err!(ResponseHeartbeat, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseHeartbeat(resp)),
                    is_update: true, // Heartbeats are connection health events - route to subscription channel
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            21 => {
                let resp = decode_or_err!(ResponseRithmicSystemGatewayInfo, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseRithmicSystemGatewayInfo(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            75 => {
                let resp = decode_or_err!(Reject, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::Reject(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            76 => {
                let resp = decode_or_err!(UserAccountUpdate, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::UserAccountUpdate(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            77 => {
                let resp = decode_or_err!(ForcedLogout, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::ForcedLogout(resp)),
                    is_update: true, // Forced logout is a connection health event - route to subscription channel
                    has_more: false,
                    multi_response: false,
                    error: Some("forced logout from server".to_string()),
                    source: self.source.clone(),
                }
            }
            101 => {
                let resp = decode_or_err!(ResponseMarketDataUpdate, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseMarketDataUpdate(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            103 => {
                let resp = decode_or_err!(ResponseGetInstrumentByUnderlying, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseGetInstrumentByUnderlying(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            104 => {
                let resp =
                    decode_or_err!(ResponseGetInstrumentByUnderlyingKeys, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseGetInstrumentByUnderlyingKeys(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            106 => {
                let resp =
                    decode_or_err!(ResponseMarketDataUpdateByUnderlying, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseMarketDataUpdateByUnderlying(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            108 => {
                let resp = decode_or_err!(ResponseGiveTickSizeTypeTable, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseGiveTickSizeTypeTable(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            110 => {
                let resp = decode_or_err!(ResponseSearchSymbols, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseSearchSymbols(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            112 => {
                let resp = decode_or_err!(ResponseProductCodes, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseProductCodes(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            114 => {
                let resp = decode_or_err!(ResponseFrontMonthContract, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseFrontMonthContract(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            116 => {
                let resp = decode_or_err!(ResponseDepthByOrderSnapshot, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseDepthByOrderSnapshot(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            118 => {
                let resp = decode_or_err!(ResponseDepthByOrderUpdates, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseDepthByOrderUpdates(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            120 => {
                let resp = decode_or_err!(ResponseGetVolumeAtPrice, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseGetVolumeAtPrice(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            122 => {
                let resp = decode_or_err!(ResponseAuxilliaryReferenceData, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseAuxilliaryReferenceData(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            150 => {
                let resp = decode_or_err!(LastTrade, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::LastTrade(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            151 => {
                let resp = decode_or_err!(BestBidOffer, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::BestBidOffer(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            152 => {
                let resp = decode_or_err!(TradeStatistics, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::TradeStatistics(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            153 => {
                let resp = decode_or_err!(QuoteStatistics, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::QuoteStatistics(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            154 => {
                let resp = decode_or_err!(IndicatorPrices, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::IndicatorPrices(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            155 => {
                let resp = decode_or_err!(EndOfDayPrices, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::EndOfDayPrices(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            156 => {
                let resp = decode_or_err!(OrderBook, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::OrderBook(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            157 => {
                let resp = decode_or_err!(MarketMode, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::MarketMode(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            158 => {
                let resp = decode_or_err!(OpenInterest, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::OpenInterest(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            159 => {
                let resp = decode_or_err!(FrontMonthContractUpdate, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::FrontMonthContractUpdate(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            160 => {
                let resp = decode_or_err!(DepthByOrder, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::DepthByOrder(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            161 => {
                let resp = decode_or_err!(DepthByOrderEndEvent, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::DepthByOrderEndEvent(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            162 => {
                let resp = decode_or_err!(SymbolMarginRate, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::SymbolMarginRate(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            163 => {
                let resp = decode_or_err!(OrderPriceLimits, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::OrderPriceLimits(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            201 => {
                let resp = decode_or_err!(ResponseTimeBarUpdate, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseTimeBarUpdate(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            203 => {
                let resp = decode_or_err!(ResponseTimeBarReplay, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseTimeBarReplay(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            205 => {
                let resp = decode_or_err!(ResponseTickBarUpdate, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseTickBarUpdate(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            207 => {
                let resp = decode_or_err!(ResponseTickBarReplay, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseTickBarReplay(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            209 => {
                let resp = decode_or_err!(ResponseVolumeProfileMinuteBars, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseVolumeProfileMinuteBars(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            211 => {
                let resp = decode_or_err!(ResponseResumeBars, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseResumeBars(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            250 => {
                let resp = decode_or_err!(TimeBar, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::TimeBar(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            251 => {
                let resp = decode_or_err!(TickBar, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::TickBar(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            301 => {
                let resp = decode_or_err!(ResponseLoginInfo, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseLoginInfo(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            303 => {
                let resp = decode_or_err!(ResponseAccountList, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseAccountList(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            305 => {
                let resp = decode_or_err!(ResponseAccountRmsInfo, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseAccountRmsInfo(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            307 => {
                let resp = decode_or_err!(ResponseProductRmsInfo, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseProductRmsInfo(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            309 => {
                let resp = decode_or_err!(ResponseSubscribeForOrderUpdates, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseSubscribeForOrderUpdates(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            311 => {
                let resp = decode_or_err!(ResponseTradeRoutes, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseTradeRoutes(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            313 => {
                let resp = decode_or_err!(ResponseNewOrder, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseNewOrder(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            315 => {
                let resp = decode_or_err!(ResponseModifyOrder, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseModifyOrder(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            317 => {
                let resp = decode_or_err!(ResponseCancelOrder, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseCancelOrder(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            319 => {
                let resp = decode_or_err!(ResponseShowOrderHistoryDates, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseShowOrderHistoryDates(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            321 => {
                let resp = decode_or_err!(ResponseShowOrders, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseShowOrders(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            323 => {
                let resp = decode_or_err!(ResponseShowOrderHistory, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseShowOrderHistory(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            325 => {
                let resp = decode_or_err!(ResponseShowOrderHistorySummary, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseShowOrderHistorySummary(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            327 => {
                let resp = decode_or_err!(ResponseShowOrderHistoryDetail, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseShowOrderHistoryDetail(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            329 => {
                let resp = decode_or_err!(ResponseOcoOrder, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseOcoOrder(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            331 => {
                let resp = decode_or_err!(ResponseBracketOrder, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseBracketOrder(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            333 => {
                let resp = decode_or_err!(ResponseUpdateTargetBracketLevel, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseUpdateTargetBracketLevel(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            335 => {
                let resp = decode_or_err!(ResponseUpdateStopBracketLevel, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseUpdateStopBracketLevel(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            337 => {
                let resp = decode_or_err!(ResponseSubscribeToBracketUpdates, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseSubscribeToBracketUpdates(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            339 => {
                let resp = decode_or_err!(ResponseShowBrackets, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseShowBrackets(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            341 => {
                let resp = decode_or_err!(ResponseShowBracketStops, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseShowBracketStops(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            343 => {
                let resp = decode_or_err!(ResponseListExchangePermissions, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseListExchangePermissions(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            345 => {
                let resp = decode_or_err!(ResponseLinkOrders, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseLinkOrders(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            347 => {
                let resp = decode_or_err!(ResponseCancelAllOrders, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseCancelAllOrders(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            349 => {
                let resp = decode_or_err!(ResponseEasyToBorrowList, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseEasyToBorrowList(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            350 => {
                let resp = decode_or_err!(TradeRoute, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::TradeRoute(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            351 => {
                let resp = decode_or_err!(RithmicOrderNotification, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::RithmicOrderNotification(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            352 => {
                let resp = decode_or_err!(ExchangeOrderNotification, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::ExchangeOrderNotification(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            353 => {
                let resp = decode_or_err!(BracketUpdates, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::BracketUpdates(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            354 => {
                let resp = decode_or_err!(AccountListUpdates, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::AccountListUpdates(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            355 => {
                let resp = decode_or_err!(UpdateEasyToBorrowList, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::UpdateEasyToBorrowList(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            356 => {
                let resp = decode_or_err!(AccountRmsUpdates, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::AccountRmsUpdates(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            401 => {
                let resp = decode_or_err!(ResponsePnLPositionUpdates, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponsePnLPositionUpdates(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            403 => {
                let resp = decode_or_err!(ResponsePnLPositionSnapshot, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponsePnLPositionSnapshot(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            450 => {
                let resp = decode_or_err!(InstrumentPnLPositionUpdate, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::InstrumentPnLPositionUpdate(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            451 => {
                let resp = decode_or_err!(AccountPnLPositionUpdate, payload, self.source);

                RithmicResponse {
                    request_id: "".to_string(),
                    message: Box::new(RithmicMessage::AccountPnLPositionUpdate(resp)),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: None,
                    source: self.source.clone(),
                }
            }
            501 => {
                let resp = decode_or_err!(ResponseListUnacceptedAgreements, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseListUnacceptedAgreements(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            503 => {
                let resp = decode_or_err!(ResponseListAcceptedAgreements, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseListAcceptedAgreements(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            505 => {
                let resp = decode_or_err!(ResponseAcceptAgreement, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseAcceptAgreement(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            507 => {
                let resp = decode_or_err!(ResponseShowAgreement, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseShowAgreement(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            509 => {
                let resp = decode_or_err!(
                    ResponseSetRithmicMrktDataSelfCertStatus,
                    payload,
                    self.source
                );
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseSetRithmicMrktDataSelfCertStatus(
                        resp,
                    )),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            3501 => {
                let resp = decode_or_err!(ResponseModifyOrderReferenceData, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseModifyOrderReferenceData(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            3503 => {
                let resp = decode_or_err!(ResponseOrderSessionConfig, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseOrderSessionConfig(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            3505 => {
                let resp = decode_or_err!(ResponseExitPosition, payload, self.source);
                let has_more = has_multiple(&resp.rq_handler_rp_code);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseExitPosition(resp)),
                    is_update: false,
                    has_more,
                    multi_response: true,
                    error,
                    source: self.source.clone(),
                }
            }
            3507 => {
                let resp = decode_or_err!(ResponseReplayExecutions, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseReplayExecutions(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            3509 => {
                let resp = decode_or_err!(ResponseAccountRmsUpdates, payload, self.source);
                let error = get_error(&resp.rp_code);

                RithmicResponse {
                    request_id: resp.user_msg.first().cloned().unwrap_or_default(),
                    message: Box::new(RithmicMessage::ResponseAccountRmsUpdates(resp)),
                    is_update: false,
                    has_more: false,
                    multi_response: false,
                    error,
                    source: self.source.clone(),
                }
            }
            _ => {
                log::warn!(
                    "Unknown message type received - template_id: {}, data_size: {} bytes",
                    parsed_message.template_id,
                    data.len()
                );

                // Treat unknown template IDs as updates (broadcast to
                // subscribers) rather than request-response — unknown
                // messages are more likely to be new subscription types
                // than responses to a pending request.
                RithmicResponse {
                    request_id: String::new(),
                    message: Box::new(RithmicMessage::Unknown),
                    is_update: true,
                    has_more: false,
                    multi_response: false,
                    error: Some(format!(
                        "Unknown message type: template_id={}",
                        parsed_message.template_id
                    )),
                    source: self.source.clone(),
                }
            }
        };

        // Handle errors
        if let Some(error) = check_message_error(&response) {
            error!("receiver_api: error {:#?} {:?}", response, error);

            return Err(response);
        }

        Ok(response)
    }
}

fn has_multiple(rq_handler_rp_code: &[String]) -> bool {
    !rq_handler_rp_code.is_empty() && rq_handler_rp_code[0] == "0"
}

/// Extracts an error message from the Rithmic `rp_code` field.
///
/// Returns `None` for success (code "0" or empty), otherwise
/// returns the human-readable error text from `rp_code[1]`.
/// Does not log -- callers decide the appropriate log level.
fn get_error(rp_code: &[String]) -> Option<String> {
    match rp_code {
        [] => None,
        [s] if s == "0" => None,
        [code] => Some(format!("Error code: {}", code)),
        [_, msg, ..] => Some(msg.clone()),
    }
}

/// Returns the error string from a response, if present
fn check_message_error(message: &RithmicResponse) -> Option<String> {
    message.error.as_ref().map(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test response with a specific message type
    fn make_response(message: RithmicMessage) -> RithmicResponse {
        RithmicResponse {
            request_id: String::new(),
            message: Box::new(message),
            is_update: false,
            has_more: false,
            multi_response: false,
            error: None,
            source: "test".to_string(),
        }
    }

    fn make_response_with_error(message: RithmicMessage, error: &str) -> RithmicResponse {
        RithmicResponse {
            error: Some(error.to_string()),
            ..make_response(message)
        }
    }

    // =========================================================================
    // is_error() tests
    // =========================================================================

    #[test]
    fn is_error_true_when_error_field_set() {
        // Even with a normal message, if error field is set, is_error should be true
        let response = make_response_with_error(
            RithmicMessage::ResponseHeartbeat(ResponseHeartbeat::default()),
            "some error",
        );
        assert!(response.is_error());
    }

    #[test]
    fn is_error_true_for_connection_issues_without_error_field() {
        // Connection issues should be errors even without error field set
        let response = make_response(RithmicMessage::ConnectionError);
        assert!(response.is_error());
        assert!(response.error.is_none()); // Verify error field is not set
    }

    #[test]
    fn is_error_false_for_normal_response() {
        let response = make_response(RithmicMessage::ResponseHeartbeat(
            ResponseHeartbeat::default(),
        ));
        assert!(!response.is_error());
    }

    // =========================================================================
    // is_connection_issue() tests
    // =========================================================================

    #[test]
    fn is_connection_issue_detects_all_connection_error_types() {
        // Test all three connection issue types
        let connection_error = make_response(RithmicMessage::ConnectionError);
        let heartbeat_timeout = make_response(RithmicMessage::HeartbeatTimeout);
        let forced_logout = make_response(RithmicMessage::ForcedLogout(ForcedLogout::default()));

        assert!(connection_error.is_connection_issue());
        assert!(heartbeat_timeout.is_connection_issue());
        assert!(forced_logout.is_connection_issue());
    }

    #[test]
    fn is_connection_issue_false_for_reject() {
        // Reject is an error but NOT a connection issue
        let response = make_response(RithmicMessage::Reject(Reject::default()));
        assert!(!response.is_connection_issue());
    }

    // =========================================================================
    // is_market_data() tests
    // =========================================================================

    #[test]
    fn is_market_data_true_for_market_data_types() {
        let bbo = make_response(RithmicMessage::BestBidOffer(BestBidOffer::default()));
        let trade = make_response(RithmicMessage::LastTrade(LastTrade::default()));
        let depth = make_response(RithmicMessage::DepthByOrder(DepthByOrder::default()));
        let depth_end = make_response(RithmicMessage::DepthByOrderEndEvent(
            DepthByOrderEndEvent::default(),
        ));
        let orderbook = make_response(RithmicMessage::OrderBook(OrderBook::default()));

        assert!(bbo.is_market_data());
        assert!(trade.is_market_data());
        assert!(depth.is_market_data());
        assert!(depth_end.is_market_data());
        assert!(orderbook.is_market_data());
    }

    #[test]
    fn is_market_data_false_for_order_notifications() {
        // Order notifications are NOT market data
        let response = make_response(RithmicMessage::RithmicOrderNotification(
            RithmicOrderNotification::default(),
        ));
        assert!(!response.is_market_data());
    }

    // =========================================================================
    // is_order_update() tests
    // =========================================================================

    #[test]
    fn is_order_update_true_for_order_notification_types() {
        let rithmic_notif = make_response(RithmicMessage::RithmicOrderNotification(
            RithmicOrderNotification::default(),
        ));
        let exchange_notif = make_response(RithmicMessage::ExchangeOrderNotification(
            ExchangeOrderNotification::default(),
        ));
        let bracket = make_response(RithmicMessage::BracketUpdates(BracketUpdates::default()));

        assert!(rithmic_notif.is_order_update());
        assert!(exchange_notif.is_order_update());
        assert!(bracket.is_order_update());
    }

    #[test]
    fn is_order_update_false_for_market_data() {
        // Market data is NOT an order update
        let response = make_response(RithmicMessage::BestBidOffer(BestBidOffer::default()));
        assert!(!response.is_order_update());
    }

    // =========================================================================
    // is_pnl_update() tests
    // =========================================================================

    #[test]
    fn is_pnl_update_true_for_pnl_types() {
        let account_pnl = make_response(RithmicMessage::AccountPnLPositionUpdate(
            AccountPnLPositionUpdate::default(),
        ));
        let instrument_pnl = make_response(RithmicMessage::InstrumentPnLPositionUpdate(
            InstrumentPnLPositionUpdate::default(),
        ));

        assert!(account_pnl.is_pnl_update());
        assert!(instrument_pnl.is_pnl_update());
    }

    #[test]
    fn is_pnl_update_false_for_order_updates() {
        // Order updates are NOT P&L updates
        let response = make_response(RithmicMessage::RithmicOrderNotification(
            RithmicOrderNotification::default(),
        ));
        assert!(!response.is_pnl_update());
    }

    // =========================================================================
    // Mutual exclusivity tests - verify categories don't overlap unexpectedly
    // =========================================================================

    #[test]
    fn categories_are_mutually_exclusive() {
        // Market data should not be flagged as order update or pnl
        let market_data = make_response(RithmicMessage::BestBidOffer(BestBidOffer::default()));
        assert!(market_data.is_market_data());
        assert!(!market_data.is_order_update());
        assert!(!market_data.is_pnl_update());
        assert!(!market_data.is_connection_issue());

        // Order update should not be flagged as market data or pnl
        let order = make_response(RithmicMessage::RithmicOrderNotification(
            RithmicOrderNotification::default(),
        ));
        assert!(order.is_order_update());
        assert!(!order.is_market_data());
        assert!(!order.is_pnl_update());
        assert!(!order.is_connection_issue());

        // PnL should not be flagged as market data or order update
        let pnl = make_response(RithmicMessage::AccountPnLPositionUpdate(
            AccountPnLPositionUpdate::default(),
        ));
        assert!(pnl.is_pnl_update());
        assert!(!pnl.is_market_data());
        assert!(!pnl.is_order_update());
        assert!(!pnl.is_connection_issue());

        // Connection issue should not be in any other category
        let conn_err = make_response(RithmicMessage::ConnectionError);
        assert!(conn_err.is_connection_issue());
        assert!(!conn_err.is_market_data());
        assert!(!conn_err.is_order_update());
        assert!(!conn_err.is_pnl_update());
    }
}
