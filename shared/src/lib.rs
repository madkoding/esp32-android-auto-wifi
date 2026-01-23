//! # Shared Protocol Library
//!
//! This crate provides the core abstractions for the ESP32 Android Auto WiFi bridge:
//!
//! - **Protocol Messages**: Serializable message types for USB-WiFi communication
//! - **DataForwarder Trait**: Abstraction for bidirectional data flow
//! - **Zero-Copy Buffers**: Lock-free ring buffers for low-latency data transfer
//!
//! ## Architecture
//!
//! ```text
//! USB Endpoint ─────► ZeroCopyBuffer ─────► WiFi Socket
//!                          │
//!                    (No memcpy!)
//!                          │
//! USB Endpoint ◄───── ZeroCopyBuffer ◄───── WiFi Socket
//! ```
//!
//! ## Low-Latency Strategy
//!
//! The zero-copy design minimizes latency by:
//! 1. Pre-allocating static buffers at compile time
//! 2. Using atomic indices for lock-free producer/consumer access
//! 3. Allowing DMA peripherals to read/write directly to buffer memory
//! 4. Avoiding any heap allocations during runtime operation

#![cfg_attr(not(feature = "std"), no_std)]

pub mod buffer;
pub mod protocol;
pub mod traits;

// Re-export main types for convenience
pub use buffer::{BufferError, BufferSlice, ZeroCopyBuffer, BUFFER_SIZE};
pub use protocol::{ControlMessage, DataPayload, Header, Message, MessageType};
pub use traits::{DataForwarder, EndpointReader, EndpointWriter, ForwarderError};

/// Library version for protocol compatibility checks
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum Transfer Unit for Android Auto projection data
/// Aligned to USB full-speed bulk endpoint size (512 bytes) for efficiency
pub const MTU: usize = 16384;

/// Android Auto protocol magic bytes for frame detection
pub const AA_MAGIC: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
