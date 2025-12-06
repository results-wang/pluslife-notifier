use std::sync::{Arc, Mutex};

use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use base64::{Engine, prelude::BASE64_STANDARD};
use serde::Serialize;
use tracing::error;

use crate::{
    Error,
    messages::{DetectionResult, SubgroupResult},
    state::State,
};

// We don't track disconnects, we just keep trying to send on all websockets.
// We expect tests to be relatively quick to run, and updates to be relatively rare.
// Accordingly, rather than carefully trying to avoid a few failed sends (at the risk of dropping messages if e.g. a reconnect happens), we just keep trying to send, knowing we'll garbage collect the websockets soon enough.

#[derive(Clone, Default)]
pub struct SessionSockets {
    websockets: Arc<Mutex<Vec<SessionSocket>>>,
}

impl SessionSockets {
    pub fn new() -> SessionSockets {
        SessionSockets {
            websockets: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn notify(&self, state: &State) {
        let websockets = self.websockets.lock().unwrap().clone();
        if !websockets.is_empty() {
            let maybe_message_json = WebsocketMessage::try_from(state)
                .and_then(|message| serde_json::to_string(&message).map_err(Into::into));
            match maybe_message_json {
                Ok(message) => {
                    for websocket in websockets {
                        websocket.send(message.clone());
                    }
                }
                Err(err) => {
                    error!(?err, "Failed to serialize websocket message");
                }
            }
        }
    }

    pub fn push(&self, websocket: WebSocket) -> (SessionSocket, usize) {
        let mut websockets = self.websockets.lock().unwrap();
        let websocket = SessionSocket::new(websocket);
        websockets.push(websocket.clone());
        (websocket, websockets.len())
    }
}

#[derive(Clone)]
pub struct SessionSocket {
    socket: Arc<tokio::sync::Mutex<WebSocket>>,
}

impl SessionSocket {
    fn new(websocket: WebSocket) -> SessionSocket {
        SessionSocket {
            socket: Arc::new(tokio::sync::Mutex::new(websocket)),
        }
    }

    pub fn notify(&self, state: &State) {
        let maybe_message_json = WebsocketMessage::try_from(state)
            .and_then(|message| serde_json::to_string(&message).map_err(Into::into));
        match maybe_message_json {
            Ok(message) => {
                self.send(message);
            }
            Err(err) => {
                error!(?err, "Failed to serialize websocket message");
            }
        }
    }

    fn send(&self, message: String) {
        let socket = self.socket.clone();
        tokio::spawn(async move {
            let result = socket
                .lock()
                .await
                .send(Message::Text(Utf8Bytes::from(message)))
                .await;
            if let Err(err) = result {
                error!(?err, "Error writing to websocket");
            }
        });
    }
}

#[derive(Serialize)]
struct WebsocketMessage {
    graph_png_base64: Option<String>,
    results: Option<Results>,
}

#[derive(Serialize)]
struct Results {
    overall: DetectionResult,
    subgroup_results: Vec<SubgroupResult>,
}

impl TryFrom<&State> for WebsocketMessage {
    type Error = Error;

    fn try_from(state: &State) -> Result<Self, Self::Error> {
        match state {
            State::IncompleteTest(incomplete_test) => {
                if incomplete_test.data.samples.is_empty() {
                    Ok(WebsocketMessage {
                        graph_png_base64: None,
                        results: None,
                    })
                } else {
                    Ok(WebsocketMessage {
                        graph_png_base64: Some(
                            BASE64_STANDARD.encode(
                                &incomplete_test
                                    .data
                                    .to_graph()?
                                    .normalise_values_to_zero()
                                    .plot_to_buffer()?,
                            ),
                        ),
                        results: None,
                    })
                }
            }
            State::CompletedTest(completed_test) => Ok(WebsocketMessage {
                graph_png_base64: Some(BASE64_STANDARD.encode(&completed_test.graph_png)),
                results: Some(Results {
                    overall: completed_test.overall,
                    subgroup_results: completed_test.subgroup_results.clone(),
                }),
            }),
        }
    }
}
