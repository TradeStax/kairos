use prost::Message;

use super::config::{RithmicConnectionConfig, RithmicEnv};
use super::rti::{
    RequestAuxilliaryReferenceData, RequestDepthByOrderSnapshot, RequestDepthByOrderUpdates,
    RequestFrontMonthContract, RequestGetInstrumentByUnderlying, RequestGetVolumeAtPrice,
    RequestGiveTickSizeTypeTable, RequestHeartbeat, RequestListExchanges, RequestLogin,
    RequestLogout, RequestMarketDataUpdate, RequestMarketDataUpdateByUnderlying,
    RequestProductCodes, RequestReferenceData, RequestResumeBars, RequestRithmicSystemGatewayInfo,
    RequestRithmicSystemInfo, RequestSearchSymbols, RequestTickBarReplay, RequestTickBarUpdate,
    RequestTimeBarReplay, RequestTimeBarUpdate, RequestVolumeProfileMinuteBars,
    request_depth_by_order_updates,
    request_login::SysInfraType,
    request_market_data_update::{Request, UpdateBits},
    request_market_data_update_by_underlying, request_search_symbols,
    request_tick_bar_replay::{BarSubType, BarType, Direction, TimeOrder},
    request_tick_bar_update, request_time_bar_replay, request_time_bar_update,
};

#[derive(Debug, Clone)]
pub(crate) struct RithmicSenderApi {
    account_id: String,
    env: RithmicEnv,
    fcm_id: String,
    ib_id: String,
    message_id_counter: u64,
}

impl RithmicSenderApi {
    pub(crate) fn new(config: &RithmicConnectionConfig) -> Self {
        RithmicSenderApi {
            account_id: config.account_id.clone(),
            env: config.env,
            fcm_id: config.fcm_id.clone(),
            ib_id: config.ib_id.clone(),
            message_id_counter: 0,
        }
    }

    fn get_next_message_id(&mut self) -> String {
        self.message_id_counter += 1;
        self.message_id_counter.to_string()
    }

    fn request_to_buf(&self, req: impl Message, id: String) -> (Vec<u8>, String) {
        let mut buf = Vec::new();
        let len = req.encoded_len() as u32;
        let header = len.to_be_bytes();

        buf.reserve((len + 4) as usize);
        req.encode(&mut buf).unwrap();
        buf.splice(0..0, header.iter().cloned());

        (buf, id)
    }

    pub fn request_rithmic_system_info(&mut self) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestRithmicSystemInfo {
            template_id: 16,
            user_msg: vec![id.clone()],
        };

        self.request_to_buf(req, id)
    }

    pub fn request_login(
        &mut self,
        system_name: &str,
        infra_type: SysInfraType,
        user: &str,
        password: &str,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestLogin {
            template_id: 10,
            template_version: Some("5.30".into()),
            user: Some(user.to_string()),
            password: Some(password.to_string()),
            app_name: Some("mamc:Kairos".to_string()),
            app_version: Some("1".into()),
            system_name: Some(system_name.to_string()),
            infra_type: Some(infra_type.into()),
            user_msg: vec![id.clone()],
            ..RequestLogin::default()
        };

        self.request_to_buf(req, id)
    }

    pub fn request_logout(&mut self) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestLogout {
            template_id: 12,
            user_msg: vec![id.clone()],
        };

        self.request_to_buf(req, id)
    }

    pub fn request_heartbeat(&mut self) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestHeartbeat {
            template_id: 18,
            user_msg: vec![id.clone()],
            ..RequestHeartbeat::default()
        };

        self.request_to_buf(req, id)
    }

    /// Request Rithmic system gateway information
    ///
    /// Returns gateway-specific information for a Rithmic system.
    ///
    /// # Arguments
    /// * `system_name` - Optional system name to get info for
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_rithmic_system_gateway_info(
        &mut self,
        system_name: Option<&str>,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestRithmicSystemGatewayInfo {
            template_id: 20,
            user_msg: vec![id.clone()],
            system_name: system_name.map(|s| s.to_string()),
        };

        self.request_to_buf(req, id)
    }

    pub fn request_market_data_update(
        &mut self,
        symbol: &str,
        exchange: &str,
        fields: Vec<UpdateBits>,
        request_type: Request,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let mut req = RequestMarketDataUpdate {
            template_id: 100,
            user_msg: vec![id.clone()],
            ..RequestMarketDataUpdate::default()
        };

        let mut bits = 0;

        for field in fields {
            bits |= field as u32;
        }

        req.symbol = Some(symbol.into());
        req.exchange = Some(exchange.into());
        req.request = Some(request_type.into());
        req.update_bits = Some(bits);

        self.request_to_buf(req, id)
    }

    /// Request instruments by underlying symbol
    ///
    /// Returns all instruments (options, futures) for a given underlying symbol.
    ///
    /// # Arguments
    /// * `underlying_symbol` - The underlying symbol (e.g., "ES" for E-mini S&P 500)
    /// * `exchange` - The exchange code (e.g., "CME")
    /// * `expiration_date` - Optional expiration date filter
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_get_instrument_by_underlying(
        &mut self,
        underlying_symbol: &str,
        exchange: &str,
        expiration_date: Option<&str>,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestGetInstrumentByUnderlying {
            template_id: 102,
            user_msg: vec![id.clone()],
            underlying_symbol: Some(underlying_symbol.to_string()),
            exchange: Some(exchange.to_string()),
            expiration_date: expiration_date.map(|d| d.to_string()),
        };

        self.request_to_buf(req, id)
    }

    /// Subscribe to or unsubscribe from market data updates by underlying
    ///
    /// Similar to request_market_data_update but subscribes to all instruments
    /// for a given underlying symbol.
    ///
    /// # Arguments
    /// * `underlying_symbol` - The underlying symbol (e.g., "ES")
    /// * `exchange` - The exchange code (e.g., "CME")
    /// * `expiration_date` - Optional expiration date filter
    /// * `fields` - The market data fields to subscribe to
    /// * `request_type` - Subscribe or Unsubscribe
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_market_data_update_by_underlying(
        &mut self,
        underlying_symbol: &str,
        exchange: &str,
        expiration_date: Option<&str>,
        fields: Vec<request_market_data_update_by_underlying::UpdateBits>,
        request_type: request_market_data_update_by_underlying::Request,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();
        let mut bits = 0;

        for field in fields {
            bits |= field as u32;
        }

        let req = RequestMarketDataUpdateByUnderlying {
            template_id: 105,
            user_msg: vec![id.clone()],
            underlying_symbol: Some(underlying_symbol.to_string()),
            exchange: Some(exchange.to_string()),
            expiration_date: expiration_date.map(|d| d.to_string()),
            request: Some(request_type.into()),
            update_bits: Some(bits),
        };

        self.request_to_buf(req, id)
    }

    /// Request tick size type table
    ///
    /// Returns the tick size table for a given tick size type.
    ///
    /// # Arguments
    /// * `tick_size_type` - The tick size type identifier
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_give_tick_size_type_table(&mut self, tick_size_type: &str) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestGiveTickSizeTypeTable {
            template_id: 107,
            user_msg: vec![id.clone()],
            tick_size_type: Some(tick_size_type.to_string()),
        };

        self.request_to_buf(req, id)
    }

    /// Request product codes
    ///
    /// Returns available product codes for an exchange.
    ///
    /// # Arguments
    /// * `exchange` - Optional exchange filter (e.g., "CME")
    /// * `give_toi_products_only` - If true, only return Time of Interest products
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_product_codes(
        &mut self,
        exchange: Option<&str>,
        give_toi_products_only: Option<bool>,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestProductCodes {
            template_id: 111,
            user_msg: vec![id.clone()],
            exchange: exchange.map(|e| e.to_string()),
            give_toi_products_only,
        };

        self.request_to_buf(req, id)
    }

    /// Request volume at price data
    ///
    /// Returns the volume profile (volume at each price level) for a symbol.
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_get_volume_at_price(
        &mut self,
        symbol: &str,
        exchange: &str,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestGetVolumeAtPrice {
            template_id: 119,
            user_msg: vec![id.clone()],
            symbol: Some(symbol.to_string()),
            exchange: Some(exchange.to_string()),
        };

        self.request_to_buf(req, id)
    }

    /// Request auxiliary reference data
    ///
    /// Returns additional reference data for a symbol.
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_auxilliary_reference_data(
        &mut self,
        symbol: &str,
        exchange: &str,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestAuxilliaryReferenceData {
            template_id: 121,
            user_msg: vec![id.clone()],
            symbol: Some(symbol.to_string()),
            exchange: Some(exchange.to_string()),
        };

        self.request_to_buf(req, id)
    }

    pub fn request_depth_by_order_snapshot(
        &mut self,
        symbol: &str,
        exchange: &str,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestDepthByOrderSnapshot {
            template_id: 115,
            user_msg: vec![id.clone()],
            symbol: Some(symbol.into()),
            exchange: Some(exchange.into()),
            depth_price: None,
        };

        self.request_to_buf(req, id)
    }

    pub fn request_depth_by_order_update(
        &mut self,
        symbol: &str,
        exchange: &str,
        request_type: request_depth_by_order_updates::Request,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestDepthByOrderUpdates {
            template_id: 117,
            user_msg: vec![id.clone()],
            request: Some(request_type.into()),
            symbol: Some(symbol.into()),
            exchange: Some(exchange.into()),
            depth_price: None,
        };

        self.request_to_buf(req, id)
    }

    /// Request to search for symbols matching a pattern
    ///
    /// # Arguments
    /// * `search_text` - Search query string
    /// * `exchange` - Optional exchange filter (e.g., "CME", "COMEX")
    /// * `product_code` - Optional product code filter (e.g., "ES", "SI")
    /// * `instrument_type` - Optional instrument type filter
    /// * `pattern` - Search pattern type (EQUALS or CONTAINS)
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_search_symbols(
        &mut self,
        search_text: &str,
        exchange: Option<&str>,
        product_code: Option<&str>,
        instrument_type: Option<request_search_symbols::InstrumentType>,
        pattern: Option<request_search_symbols::Pattern>,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestSearchSymbols {
            template_id: 109,
            user_msg: vec![id.clone()],
            search_text: Some(search_text.to_string()),
            exchange: exchange.map(|e| e.to_string()),
            product_code: product_code.map(|p| p.to_string()),
            instrument_type: instrument_type.map(|i| i.into()),
            pattern: pattern.map(|p| p.into()),
        };

        self.request_to_buf(req, id)
    }

    /// Request list of exchanges available to the user
    ///
    /// Returns the exchanges the user has permission to trade on.
    ///
    /// # Arguments
    /// * `user` - Username for authentication
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_list_exchanges(&mut self, _user: &str) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestListExchanges {
            template_id: 342,
            user_msg: vec![id.clone()],
        };

        self.request_to_buf(req, id)
    }

    /// Request front month contract information
    ///
    /// Returns the current front month contract for a given product.
    /// Optionally subscribe to updates when the front month rolls.
    ///
    /// # Arguments
    /// * `symbol` - The product symbol (e.g., "ES" for E-mini S&P 500)
    /// * `exchange` - The exchange code (e.g., "CME")
    /// * `need_updates` - Whether to receive updates when front month changes
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_front_month_contract(
        &mut self,
        symbol: &str,
        exchange: &str,
        need_updates: bool,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestFrontMonthContract {
            template_id: 113,
            user_msg: vec![id.clone()],
            symbol: Some(symbol.to_string()),
            exchange: Some(exchange.to_string()),
            need_updates: Some(need_updates),
        };

        self.request_to_buf(req, id)
    }

    /// Request reference data for a symbol
    ///
    /// Returns detailed information about a trading instrument including
    /// tick size, point value, trading hours, and other symbol specifications.
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_reference_data(&mut self, symbol: &str, exchange: &str) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestReferenceData {
            template_id: 14,
            user_msg: vec![id.clone()],
            symbol: Some(symbol.to_string()),
            exchange: Some(exchange.to_string()),
        };

        self.request_to_buf(req, id)
    }

    /// Request a replay of tick bar data
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol to request data for
    /// * `exchange` - The exchange of the symbol
    /// * `start_index_sec` - unix seconds
    /// * `finish_index_sec` - unix seconds
    ///
    /// # Returns
    ///
    /// A tuple containing the request buffer and the message id
    ///
    /// # Note
    ///
    /// Large data requests may be truncated by the server. If the response contains
    /// a round number of bars (e.g., 10000) or does not cover the entire requested
    /// time period, use [`request_resume_bars`](Self::request_resume_bars) with the
    /// `request_key` from the response to fetch the remaining data.
    pub fn request_tick_bar_replay(
        &mut self,
        symbol: &str,
        exchange: &str,
        start_index_sec: i32,
        finish_index_sec: i32,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestTickBarReplay {
            template_id: 206,
            exchange: Some(exchange.to_string()),
            symbol: Some(symbol.to_string()),
            bar_type: Some(BarType::TickBar.into()),
            bar_sub_type: Some(BarSubType::Regular.into()),
            bar_type_specifier: Some("1".to_string()),
            start_index: Some(start_index_sec),
            finish_index: Some(finish_index_sec),
            direction: Some(Direction::First.into()),
            time_order: Some(TimeOrder::Forwards.into()),
            user_msg: vec![id.clone()],
            ..Default::default()
        };

        self.request_to_buf(req, id)
    }

    /// Request a replay of time bar data
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol to request data for
    /// * `exchange` - The exchange of the symbol
    /// * `bar_type` - The type of time bar (SecondBar, MinuteBar, DailyBar, WeeklyBar)
    /// * `bar_type_period` - The period for the bar type (e.g., 1 for 1-minute bars,
    ///   5 for 5-minute bars)
    /// * `start_index_sec` - unix seconds
    /// * `finish_index_sec` - unix seconds
    ///
    /// # Returns
    ///
    /// A tuple containing the request buffer and the message id
    ///
    /// # Note
    ///
    /// Large data requests may be truncated by the server. If the response contains
    /// a round number of bars (e.g., 10000) or does not cover the entire requested
    /// time period, use [`request_resume_bars`](Self::request_resume_bars) with the
    /// `request_key` from the response to fetch the remaining data.
    pub fn request_time_bar_replay(
        &mut self,
        symbol: &str,
        exchange: &str,
        bar_type: request_time_bar_replay::BarType,
        bar_type_period: i32,
        start_index_sec: i32,
        finish_index_sec: i32,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestTimeBarReplay {
            template_id: 202,
            exchange: Some(exchange.to_string()),
            symbol: Some(symbol.to_string()),
            bar_type: Some(bar_type.into()),
            bar_type_period: Some(bar_type_period),
            start_index: Some(start_index_sec),
            finish_index: Some(finish_index_sec),
            direction: Some(request_time_bar_replay::Direction::First.into()),
            time_order: Some(request_time_bar_replay::TimeOrder::Forwards.into()),
            user_msg: vec![id.clone()],
            ..Default::default()
        };

        self.request_to_buf(req, id)
    }

    /// Request volume profile minute bars
    ///
    /// Returns minute bar data with volume profile information.
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    /// * `bar_type_period` - The period for the bars
    /// * `start_index_sec` - Start time in unix seconds
    /// * `finish_index_sec` - End time in unix seconds
    /// * `user_max_count` - Optional maximum number of bars to return
    /// * `resume_bars` - Whether to resume from a previous request
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    ///
    /// # Note
    ///
    /// Large data requests may be truncated by the server. If the response contains
    /// a round number of bars (e.g., 10000) or does not cover the entire requested
    /// time period, use [`request_resume_bars`](Self::request_resume_bars) with the
    /// `request_key` from the response to fetch the remaining data.
    #[allow(clippy::too_many_arguments)]
    pub fn request_volume_profile_minute_bars(
        &mut self,
        symbol: &str,
        exchange: &str,
        bar_type_period: i32,
        start_index_sec: i32,
        finish_index_sec: i32,
        user_max_count: Option<i32>,
        resume_bars: Option<bool>,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestVolumeProfileMinuteBars {
            template_id: 208,
            user_msg: vec![id.clone()],
            symbol: Some(symbol.to_string()),
            exchange: Some(exchange.to_string()),
            bar_type_period: Some(bar_type_period),
            start_index: Some(start_index_sec),
            finish_index: Some(finish_index_sec),
            user_max_count,
            resume_bars,
        };

        self.request_to_buf(req, id)
    }

    /// Request to resume a previously truncated bars request
    ///
    /// Use this when a bars request was truncated due to data limits.
    /// Pass the request_key from the previous response.
    ///
    /// # Arguments
    /// * `request_key` - The request key from the previous truncated response
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_resume_bars(&mut self, request_key: &str) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestResumeBars {
            template_id: 210,
            user_msg: vec![id.clone()],
            request_key: Some(request_key.to_string()),
        };

        self.request_to_buf(req, id)
    }

    /// Subscribe to or unsubscribe from live time bar updates
    ///
    /// Receive real-time time bar (OHLCV) updates for a symbol.
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    /// * `bar_type` - The type of time bar (SecondBar, MinuteBar, DailyBar, WeeklyBar)
    /// * `bar_type_period` - The period for the bar type (e.g., 1 for 1-minute bars)
    /// * `request` - Subscribe or Unsubscribe
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_time_bar_update(
        &mut self,
        symbol: &str,
        exchange: &str,
        bar_type: request_time_bar_update::BarType,
        bar_type_period: i32,
        request: request_time_bar_update::Request,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestTimeBarUpdate {
            template_id: 200,
            user_msg: vec![id.clone()],
            symbol: Some(symbol.to_string()),
            exchange: Some(exchange.to_string()),
            bar_type: Some(bar_type.into()),
            bar_type_period: Some(bar_type_period),
            request: Some(request.into()),
        };

        self.request_to_buf(req, id)
    }

    /// Subscribe to or unsubscribe from live tick bar updates
    ///
    /// Receive real-time tick bar updates for a symbol.
    ///
    /// # Arguments
    /// * `symbol` - The trading symbol (e.g., "ESH6")
    /// * `exchange` - The exchange code (e.g., "CME")
    /// * `bar_type` - The type of tick bar
    /// * `bar_sub_type` - Sub-type of the bar
    /// * `bar_type_specifier` - Specifier for the bar (e.g., "1" for 1-tick bars)
    /// * `request` - Subscribe or Unsubscribe
    ///
    /// # Returns
    /// A tuple of (serialized request buffer, request ID)
    pub fn request_tick_bar_update(
        &mut self,
        symbol: &str,
        exchange: &str,
        bar_type: request_tick_bar_update::BarType,
        bar_sub_type: request_tick_bar_update::BarSubType,
        bar_type_specifier: &str,
        request: request_tick_bar_update::Request,
    ) -> (Vec<u8>, String) {
        let id = self.get_next_message_id();

        let req = RequestTickBarUpdate {
            template_id: 204,
            user_msg: vec![id.clone()],
            symbol: Some(symbol.to_string()),
            exchange: Some(exchange.to_string()),
            bar_type: Some(bar_type.into()),
            bar_sub_type: Some(bar_sub_type.into()),
            bar_type_specifier: Some(bar_type_specifier.to_string()),
            request: Some(request.into()),
            ..Default::default()
        };

        self.request_to_buf(req, id)
    }
}
