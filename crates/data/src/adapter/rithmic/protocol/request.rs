//! Request-response correlation for Rithmic protocol messages.
//!
//! [`RithmicRequestHandler`] tracks in-flight requests by ID and routes
//! incoming responses to the correct oneshot sender. Supports both
//! single-response and multi-response (paginated) request patterns.

use std::collections::HashMap;

use log::error;
use tokio::sync::oneshot;

use super::{messages::RithmicMessage, response::RithmicResponse};

/// An in-flight request awaiting a response from a Rithmic plant.
#[derive(Debug)]
pub struct RithmicRequest {
    /// Unique request identifier for correlation
    pub request_id: String,
    /// Oneshot sender to deliver the response(s) back to the caller
    pub responder: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
}

/// Correlates Rithmic responses with their originating requests.
///
/// Handles both single-shot responses (forwarded immediately) and
/// multi-response sequences (buffered until `has_more == false`).
#[derive(Debug)]
pub struct RithmicRequestHandler {
    /// Pending request senders keyed by request ID
    handle_map: HashMap<String, oneshot::Sender<Result<Vec<RithmicResponse>, String>>>,
    /// Accumulated responses for multi-response requests
    response_vec_map: HashMap<String, Vec<RithmicResponse>>,
}

impl RithmicRequestHandler {
    /// Creates a new empty request handler
    pub fn new() -> Self {
        Self {
            handle_map: HashMap::new(),
            response_vec_map: HashMap::new(),
        }
    }

    /// Registers a request so its response can be correlated later
    pub fn register_request(&mut self, request: RithmicRequest) {
        self.handle_map
            .insert(request.request_id, request.responder);
    }

    /// Sends completed responses to the waiting caller
    fn send_to_responder(
        &self,
        responder: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
        responses: Vec<RithmicResponse>,
        request_id: &str,
    ) {
        if let Err(e) = responder.send(Ok(responses)) {
            error!(
                "Failed to send response: receiver dropped for request_id {}: {:#?}",
                request_id, e
            );
        }
    }

    /// Routes a response to the correct pending request.
    ///
    /// Single-response messages are forwarded immediately.
    /// Multi-response messages are buffered until the final
    /// message (`has_more == false`), then sent as a batch.
    pub fn handle_response(&mut self, response: RithmicResponse) {
        match response.message {
            RithmicMessage::ResponseHeartbeat(_) => {
                // Handle heartbeat response if a callback is registered
                if let Some(responder) = self.handle_map.remove(&response.request_id) {
                    let request_id = response.request_id.clone();
                    self.send_to_responder(responder, vec![response], &request_id);
                }
            }
            _ => {
                if !response.multi_response {
                    if let Some(responder) = self.handle_map.remove(&response.request_id) {
                        let request_id = response.request_id.clone();
                        self.send_to_responder(responder, vec![response], &request_id);
                    } else {
                        error!("No responder found for response: {:#?}", response);
                    }
                } else {
                    // If response has more, we store it in a vector and wait for more messages
                    if response.has_more {
                        self.response_vec_map
                            .entry(response.request_id.clone())
                            .or_default()
                            .push(response);
                    } else if let Some(responder) = self.handle_map.remove(&response.request_id) {
                        let request_id = response.request_id.clone();
                        let response_vec = match self.response_vec_map.remove(&request_id) {
                            Some(mut vec) => {
                                vec.push(response);
                                vec
                            }
                            None => {
                                vec![response]
                            }
                        };
                        self.send_to_responder(responder, response_vec, &request_id);
                    } else {
                        error!("No responder found for response: {:#?}", response);
                    }
                }
            }
        }
    }
}

impl Default for RithmicRequestHandler {
    fn default() -> Self {
        Self::new()
    }
}
