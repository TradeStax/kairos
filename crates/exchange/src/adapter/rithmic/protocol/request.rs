use super::{response::RithmicResponse, messages::RithmicMessage};
use std::collections::HashMap;
use tokio::sync::oneshot;
use log::error;

#[derive(Debug)]
pub struct RithmicRequest {
    pub request_id: String,
    pub responder: oneshot::Sender<Result<Vec<RithmicResponse>, String>>,
}

#[derive(Debug)]
pub struct RithmicRequestHandler {
    handle_map: HashMap<String, oneshot::Sender<Result<Vec<RithmicResponse>, String>>>,
    response_vec_map: HashMap<String, Vec<RithmicResponse>>,
}

impl RithmicRequestHandler {
    pub fn new() -> Self {
        Self {
            handle_map: HashMap::new(),
            response_vec_map: HashMap::new(),
        }
    }

    pub fn register_request(&mut self, request: RithmicRequest) {
        self.handle_map
            .insert(request.request_id, request.responder);
    }

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
