use crate::types::{ChromeBridgePayload, ChromeCommand};
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone, Default)]
pub struct SharedChromeState {
    inner: Arc<Mutex<(bool, ChromeBridgePayload)>>,
}

impl SharedChromeState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, connected: bool, payload: ChromeBridgePayload) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = (connected, payload);
        }
    }

    pub fn set_disconnected(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.0 = false;
        }
    }

    pub fn get(&self) -> (bool, ChromeBridgePayload) {
        if let Ok(guard) = self.inner.lock() {
            guard.clone()
        } else {
            (false, ChromeBridgePayload::default())
        }
    }
}

pub async fn run_chrome_ws_server(
    shared_state: SharedChromeState,
    cmd_rx: async_channel::Receiver<ChromeCommand>,
) {
    let Ok(listener) = TcpListener::bind("127.0.0.1:17421").await else {
        eprintln!("Warning: Could not bind Chrome Bridge on 127.0.0.1:17421");
        return;
    };

    while let Ok((stream, _)) = listener.accept().await {
        let Ok(ws_stream) = accept_async(stream).await else {
            continue;
        };

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let state_ref = shared_state.clone();
        let cmd_rx_clone = cmd_rx.clone();

        loop {
            tokio::select! {
                msg_opt = ws_receiver.next() => {
                    match msg_opt {
                        Some(Ok(Message::Text(text))) => {
                            if let Ok(payload) = serde_json::from_str::<ChromeBridgePayload>(&text) {
                                state_ref.set(true, payload);
                            }
                        }
                        Some(Ok(Message::Close(_))) | Some(Err(_)) | None => {
                            state_ref.set_disconnected();
                            break;
                        }
                        _ => {}
                    }
                }
                cmd_res = cmd_rx_clone.recv() => {
                    if let Ok(cmd) = cmd_res {
                        if let Ok(json) = serde_json::to_string(&cmd) {
                            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                state_ref.set_disconnected();
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}