//! Native Messaging Bridge CLI
//! A standalone binary that bridges Chrome ↔ Tauri via localhost TCP
//!
//! Chrome Extension → (stdio) → native-bridge → (TCP:9876) → Tauri App

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::TcpStream;

const TAURI_PORT: u16 = 9876;

/// Message from Chrome extension
#[derive(Debug, Deserialize, Serialize)]
struct BrowserMessage {
    #[serde(rename = "type")]
    msg_type: String,
    url: Option<String>,
    title: Option<String>,
    body_text: Option<String>,
    timestamp: Option<i64>,
    recent_history: Option<Vec<serde_json::Value>>,
    top_sites: Option<Vec<serde_json::Value>>,
}

/// Response to Chrome extension
#[derive(Debug, Serialize)]
struct NativeResponse {
    action: String,
    success: bool,
    error: Option<String>,
}

fn main() {
    // Try to connect to Tauri app
    let mut tauri_connection: Option<TcpStream> = None;

    loop {
        // Read message from Chrome (stdin)
        let message = match read_native_message() {
            Ok(Some(msg)) => msg,
            Ok(None) => break, // EOF
            Err(e) => {
                eprintln!("Error reading message: {}", e);
                continue;
            }
        };

        // Try to connect/reconnect to Tauri if needed
        if tauri_connection.is_none() {
            match TcpStream::connect(format!("127.0.0.1:{}", TAURI_PORT)) {
                Ok(stream) => {
                    stream.set_nonblocking(false).ok();
                    tauri_connection = Some(stream);
                }
                Err(_) => {
                    // Tauri not running, just acknowledge
                    send_native_response(&NativeResponse {
                        action: "error".to_string(),
                        success: false,
                        error: Some("Tauri app not connected".to_string()),
                    });
                    continue;
                }
            }
        }

        // Forward message to Tauri
        if let Some(ref mut stream) = tauri_connection {
            let json = serde_json::to_vec(&message).unwrap_or_default();
            let len = (json.len() as u32).to_le_bytes();

            if stream.write_all(&len).is_err() || stream.write_all(&json).is_err() {
                // Connection lost, clear it
                tauri_connection = None;
                send_native_response(&NativeResponse {
                    action: "error".to_string(),
                    success: false,
                    error: Some("Lost connection to Tauri".to_string()),
                });
                continue;
            }

            // Read response from Tauri
            let mut len_buf = [0u8; 4];
            if stream.read_exact(&mut len_buf).is_ok() {
                let response_len = u32::from_le_bytes(len_buf) as usize;
                let mut response_buf = vec![0u8; response_len];
                if stream.read_exact(&mut response_buf).is_ok() {
                    // Forward response to Chrome
                    write_raw_to_stdout(&response_buf);
                    continue;
                }
            }
        }

        // Default acknowledgment
        send_native_response(&NativeResponse {
            action: "acknowledged".to_string(),
            success: true,
            error: None,
        });
    }
}

/// Read a native messaging message from stdin (length-prefixed JSON)
fn read_native_message() -> io::Result<Option<BrowserMessage>> {
    let mut stdin = io::stdin();

    // Read 4-byte length prefix
    let mut length_bytes = [0u8; 4];
    match stdin.read_exact(&mut length_bytes) {
        Ok(_) => {}
        Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    // Chrome uses native endian, but we convert to little endian for TCP
    let message_length = u32::from_ne_bytes(length_bytes) as usize;

    if message_length == 0 || message_length > 1024 * 1024 {
        return Ok(None);
    }

    // Read message bytes
    let mut message_bytes = vec![0u8; message_length];
    stdin.read_exact(&mut message_bytes)?;

    // Parse JSON
    let message: BrowserMessage = serde_json::from_slice(&message_bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(Some(message))
}

/// Send a native messaging response to stdout (length-prefixed JSON)
fn send_native_response(response: &NativeResponse) {
    let json = serde_json::to_vec(response).unwrap_or_default();
    write_raw_to_stdout(&json);
}

/// Write raw bytes to stdout with length prefix
fn write_raw_to_stdout(data: &[u8]) {
    let length = (data.len() as u32).to_ne_bytes();
    let mut stdout = io::stdout();
    let _ = stdout.write_all(&length);
    let _ = stdout.write_all(data);
    let _ = stdout.flush();
}
