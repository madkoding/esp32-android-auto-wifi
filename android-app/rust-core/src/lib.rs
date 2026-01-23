//! # Android JNI Rust Core Library
//!
//! This library provides the native Rust backend for the Android Auto WiFi app.
//! It handles:
//!
//! - Protocol message encoding/decoding
//! - Network communication with ESP32
//! - Android Auto projection stream management
//!
//! ## JNI Bridge Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Android App (Kotlin)                       │
//! │                                                                 │
//! │  ┌─────────────────┐     ┌──────────────────────────────────┐  │
//! │  │  MainActivity   │────►│     WifiAutoService (Service)    │  │
//! │  └─────────────────┘     └──────────────────────────────────┘  │
//! │                                       │                         │
//! │                                       │ JNI Calls               │
//! │                                       ▼                         │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │                    rust_core (this lib)                   │  │
//! │  │                                                           │  │
//! │  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐   │  │
//! │  │  │  Protocol   │  │   Network   │  │  Stream Manager │   │  │
//! │  │  │  (shared)   │  │  (tokio)    │  │   (AA control)  │   │  │
//! │  │  └─────────────┘  └─────────────┘  └─────────────────┘   │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## JNI Functions Exported
//!
//! - `Java_com_androidauto_wifi_RustBridge_init`: Initialize the Rust runtime
//! - `Java_com_androidauto_wifi_RustBridge_connect`: Connect to ESP32
//! - `Java_com_androidauto_wifi_RustBridge_disconnect`: Disconnect
//! - `Java_com_androidauto_wifi_RustBridge_sendData`: Send data to ESP32
//! - `Java_com_androidauto_wifi_RustBridge_getStats`: Get connection statistics

use jni::objects::{JClass, JObject, JString, JByteArray};
use jni::sys::{jboolean, jbyteArray, jint, jlong, jstring, JNI_TRUE, JNI_FALSE};
use jni::JNIEnv;
use log::{debug, error, info, warn, LevelFilter};
use std::sync::{Arc, Mutex, Once};
use thiserror::Error;

use shared::protocol::{ControlMessage, Message, FrameBuilder};
use shared::buffer::ZeroCopyBuffer;

// Initialize logging once
static INIT_LOGGER: Once = Once::new();

/// Errors that can occur in the JNI bridge
#[derive(Error, Debug)]
pub enum BridgeError {
    #[error("Not connected to ESP32")]
    NotConnected,
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    #[error("JNI error: {0}")]
    JniError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Connection state shared between JNI calls
struct ConnectionState {
    /// Whether currently connected
    connected: bool,
    /// ESP32 IP address
    esp32_ip: Option<String>,
    /// TCP port
    port: u16,
    /// Frame builder for protocol encoding
    frame_builder: FrameBuilder,
    /// Session ID (from handshake)
    session_id: u32,
    /// Statistics
    bytes_sent: u64,
    bytes_received: u64,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self {
            connected: false,
            esp32_ip: None,
            port: shared::protocol::MAX_PAYLOAD_SIZE as u16, // Use as placeholder
            frame_builder: FrameBuilder::new(),
            session_id: 0,
            bytes_sent: 0,
            bytes_received: 0,
        }
    }
}

// Global state (wrapped in Arc<Mutex> for thread safety)
lazy_static::lazy_static! {
    static ref STATE: Arc<Mutex<ConnectionState>> = Arc::new(Mutex::new(ConnectionState::default()));
}

// Required for lazy_static
#[macro_use]
extern crate lazy_static;

/// Initialize the Rust native library
///
/// Called from Kotlin:
/// ```kotlin
/// external fun init(): Boolean
/// ```
#[no_mangle]
pub extern "system" fn Java_com_androidauto_wifi_RustBridge_init(
    mut env: JNIEnv,
    _class: JClass,
) -> jboolean {
    // Initialize Android logger (only once)
    INIT_LOGGER.call_once(|| {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(LevelFilter::Debug)
                .with_tag("RustCore"),
        );
    });

    info!("Rust core library initialized");
    info!("Protocol version: {}", shared::VERSION);

    JNI_TRUE
}

/// Connect to ESP32 WiFi bridge
///
/// Called from Kotlin:
/// ```kotlin
/// external fun connect(ip: String, port: Int): Boolean
/// ```
#[no_mangle]
pub extern "system" fn Java_com_androidauto_wifi_RustBridge_connect(
    mut env: JNIEnv,
    _class: JClass,
    ip: JString,
    port: jint,
) -> jboolean {
    // Get IP string from Java
    let ip_str: String = match env.get_string(&ip) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get IP string: {:?}", e);
            return JNI_FALSE;
        }
    };

    info!("Connecting to ESP32 at {}:{}", ip_str, port);

    // TODO: Implement actual TCP connection using tokio
    // For now, just update state
    let mut state = STATE.lock().unwrap();
    state.esp32_ip = Some(ip_str.clone());
    state.port = port as u16;
    state.connected = true;
    state.session_id = 0; // Will be set after handshake

    info!("Connection state updated (actual TCP not yet implemented)");

    JNI_TRUE
}

/// Disconnect from ESP32
///
/// Called from Kotlin:
/// ```kotlin
/// external fun disconnect()
/// ```
#[no_mangle]
pub extern "system" fn Java_com_androidauto_wifi_RustBridge_disconnect(
    _env: JNIEnv,
    _class: JClass,
) {
    info!("Disconnecting from ESP32");

    let mut state = STATE.lock().unwrap();
    state.connected = false;
    state.esp32_ip = None;
    state.session_id = 0;

    info!("Disconnected");
}

/// Check if connected to ESP32
///
/// Called from Kotlin:
/// ```kotlin
/// external fun isConnected(): Boolean
/// ```
#[no_mangle]
pub extern "system" fn Java_com_androidauto_wifi_RustBridge_isConnected(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    let state = STATE.lock().unwrap();
    if state.connected { JNI_TRUE } else { JNI_FALSE }
}

/// Send data to ESP32
///
/// Called from Kotlin:
/// ```kotlin
/// external fun sendData(channel: Int, data: ByteArray): Int
/// ```
///
/// Returns: Number of bytes sent, or -1 on error
#[no_mangle]
pub extern "system" fn Java_com_androidauto_wifi_RustBridge_sendData(
    mut env: JNIEnv,
    _class: JClass,
    channel: jint,
    data: JByteArray,
) -> jint {
    let state = STATE.lock().unwrap();
    if !state.connected {
        warn!("sendData called but not connected");
        return -1;
    }
    drop(state); // Release lock before heavy operations

    // Get byte array from Java
    let data_len = match env.get_array_length(&data) {
        Ok(len) => len as usize,
        Err(e) => {
            error!("Failed to get array length: {:?}", e);
            return -1;
        }
    };

    let mut rust_data = vec![0u8; data_len];
    if let Err(e) = env.get_byte_array_region(&data, 0, bytemuck::cast_slice_mut(&mut rust_data)) {
        error!("Failed to copy byte array: {:?}", e);
        return -1;
    }

    debug!("Sending {} bytes on channel {}", data_len, channel);

    // TODO: Actually send data over TCP
    // For now, just update statistics
    let mut state = STATE.lock().unwrap();
    state.bytes_sent += data_len as u64;

    data_len as jint
}

/// Get connection statistics as JSON
///
/// Called from Kotlin:
/// ```kotlin
/// external fun getStats(): String
/// ```
#[no_mangle]
pub extern "system" fn Java_com_androidauto_wifi_RustBridge_getStats(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let state = STATE.lock().unwrap();
    
    let stats_json = format!(
        r#"{{"connected":{},"bytes_sent":{},"bytes_received":{},"session_id":{}}}"#,
        state.connected,
        state.bytes_sent,
        state.bytes_received,
        state.session_id
    );

    match env.new_string(&stats_json) {
        Ok(s) => s.into_raw(),
        Err(e) => {
            error!("Failed to create stats string: {:?}", e);
            std::ptr::null_mut()
        }
    }
}

/// Perform handshake with ESP32
///
/// Called from Kotlin:
/// ```kotlin
/// external fun performHandshake(): Boolean
/// ```
#[no_mangle]
pub extern "system" fn Java_com_androidauto_wifi_RustBridge_performHandshake(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    info!("Performing handshake with ESP32");

    let mut state = STATE.lock().unwrap();
    if !state.connected {
        warn!("Cannot handshake: not connected");
        return JNI_FALSE;
    }

    // Create handshake request message
    let handshake_msg = Message::Control(ControlMessage::HandshakeRequest {
        version: 1,
        features: 0xFF, // All features supported
    });

    // Serialize to frame
    let mut buffer = [0u8; 256];
    match state.frame_builder.build_frame(&handshake_msg, 0, &mut buffer) {
        Ok(len) => {
            debug!("Handshake frame built: {} bytes", len);
            // TODO: Send frame over TCP and wait for response
        }
        Err(e) => {
            error!("Failed to build handshake frame: {:?}", e);
            return JNI_FALSE;
        }
    }

    // TODO: Receive and parse HandshakeResponse
    // For now, simulate success
    state.session_id = 12345;

    info!("Handshake completed (simulated), session_id: {}", state.session_id);
    JNI_TRUE
}

/// Process incoming data from ESP32
///
/// This is called from a background thread in Kotlin when TCP data arrives.
///
/// Called from Kotlin:
/// ```kotlin
/// external fun processIncomingData(data: ByteArray): Int
/// ```
///
/// Returns: Number of bytes processed, or -1 on error
#[no_mangle]
pub extern "system" fn Java_com_androidauto_wifi_RustBridge_processIncomingData(
    mut env: JNIEnv,
    _class: JClass,
    data: JByteArray,
) -> jint {
    // Get byte array from Java
    let data_len = match env.get_array_length(&data) {
        Ok(len) => len as usize,
        Err(e) => {
            error!("Failed to get array length: {:?}", e);
            return -1;
        }
    };

    let mut rust_data = vec![0u8; data_len];
    if let Err(e) = env.get_byte_array_region(&data, 0, bytemuck::cast_slice_mut(&mut rust_data)) {
        error!("Failed to copy byte array: {:?}", e);
        return -1;
    }

    // Try to parse as protocol frame
    match FrameBuilder::parse_frame(&rust_data) {
        Ok((header, message)) => {
            debug!(
                "Received message: type={:?}, seq={}, channel={}",
                message.message_type(),
                header.sequence,
                header.channel
            );

            // Update statistics
            let mut state = STATE.lock().unwrap();
            state.bytes_received += data_len as u64;

            // Handle specific message types
            match message {
                Message::Ping { timestamp } => {
                    debug!("Received ping, timestamp: {}", timestamp);
                    // TODO: Send pong response
                }
                Message::Control(ctrl) => {
                    debug!("Received control message: {:?}", ctrl);
                }
                Message::Data(payload) => {
                    debug!("Received data: {} bytes", payload.len());
                    // TODO: Forward to Android Auto
                }
                _ => {}
            }

            data_len as jint
        }
        Err(e) => {
            warn!("Failed to parse frame: {:?}", e);
            -1
        }
    }
}

// Add bytemuck dependency for safe casting
mod bytemuck {
    pub fn cast_slice_mut(slice: &mut [u8]) -> &mut [i8] {
        unsafe { std::slice::from_raw_parts_mut(slice.as_mut_ptr() as *mut i8, slice.len()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_state_default() {
        let state = ConnectionState::default();
        assert!(!state.connected);
        assert!(state.esp32_ip.is_none());
        assert_eq!(state.bytes_sent, 0);
    }
}
