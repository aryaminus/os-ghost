//! Native Messaging bridge for Chrome extension communication
//! Handles real-time browser events from the extension via TCP

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use tauri::{AppHandle, Emitter};

const BRIDGE_PORT: u16 = 9876;

/// Message received from Chrome extension (via native_bridge)
#[derive(Debug, Deserialize)]
pub struct BrowserMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub url: Option<String>,
    pub title: Option<String>,
    pub body_text: Option<String>,
    pub timestamp: Option<i64>,
}

/// Response sent back to native_bridge
#[derive(Debug, Serialize)]
pub struct NativeResponse {
    pub action: String,
    pub success: bool,
    pub data: serde_json::Value,
}

/// Handle a single client connection
fn handle_client(mut stream: TcpStream, app: &AppHandle) {
    tracing::info!("Native bridge connected from {:?}", stream.peer_addr());

    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(30)))
        .ok();

    loop {
        // Read length prefix (4 bytes, little-endian)
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf) {
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(_) => break, // Connection closed
        }

        let msg_len = u32::from_le_bytes(len_buf) as usize;
        if msg_len == 0 || msg_len > 1024 * 1024 {
            continue;
        }

        // Read message
        let mut msg_buf = vec![0u8; msg_len];
        if stream.read_exact(&mut msg_buf).is_err() {
            break;
        }

        // Parse and handle message
        if let Ok(message) = serde_json::from_slice::<BrowserMessage>(&msg_buf) {
            tracing::debug!("Received from Chrome: {:?}", message);

            match message.msg_type.as_str() {
                "page_load" | "tab_changed" => {
                    let _ = app.emit(
                        "browser_navigation",
                        serde_json::json!({
                            "url": message.url.unwrap_or_default(),
                            "title": message.title.unwrap_or_default(),
                            "timestamp": message.timestamp.unwrap_or(0)
                        }),
                    );
                }
                "page_content" => {
                    let _ = app.emit(
                        "page_content",
                        serde_json::json!({
                            "url": message.url.unwrap_or_default(),
                            "body_text": message.body_text.unwrap_or_default()
                        }),
                    );
                }
                _ => {
                    tracing::debug!("Unknown message type: {}", message.msg_type);
                }
            }

            // Send response
            let response = NativeResponse {
                action: "acknowledged".to_string(),
                success: true,
                data: serde_json::json!({}),
            };

            if let Ok(json) = serde_json::to_vec(&response) {
                let len = (json.len() as u32).to_le_bytes();
                let _ = stream.write_all(&len);
                let _ = stream.write_all(&json);
                let _ = stream.flush();
            }
        }
    }

    tracing::info!("Native bridge disconnected");
}

/// Start the TCP server for native messaging bridge (runs in background thread)
pub fn start_native_messaging_server(app: AppHandle) {
    std::thread::spawn(move || {
        tracing::info!(
            "Starting native messaging TCP server on port {}",
            BRIDGE_PORT
        );

        let listener = match TcpListener::bind(format!("127.0.0.1:{}", BRIDGE_PORT)) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind TCP server: {}", e);
                return;
            }
        };

        tracing::info!(
            "Native messaging server listening on 127.0.0.1:{}",
            BRIDGE_PORT
        );

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let app_clone = app.clone();
                    std::thread::spawn(move || {
                        handle_client(stream, &app_clone);
                    });
                }
                Err(e) => {
                    tracing::error!("Connection error: {}", e);
                }
            }
        }
    });
}
