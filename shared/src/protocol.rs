//! # Protocol Message Definitions
//!
//! This module defines the message format for USB-WiFi bridge communication.
//! Messages are serialized using `postcard` for efficient, no_std-compatible encoding.
//!
//! ## Frame Format
//!
//! ```text
//! ┌─────────┬──────────┬────────┬──────────────────┬──────────┐
//! │  Magic  │  Header  │  Type  │     Payload      │   CRC    │
//! │ 4 bytes │  4 bytes │ 1 byte │  Variable size   │  2 bytes │
//! └─────────┴──────────┴────────┴──────────────────┴──────────┘
//! ```
//!
//! ## Message Types
//!
//! - **Control**: Connection management, handshake, keep-alive
//! - **Data**: Android Auto projection data (video, audio, input)
//! - **Debug**: Logging and diagnostics (development only)

use heapless::Vec;
use serde::{Deserialize, Serialize};

use crate::MTU;

/// Magic bytes to identify start of frame
pub const FRAME_MAGIC: [u8; 4] = [0xAA, 0x57, 0x49, 0x46]; // "AAWIF" sort of

/// Maximum payload size (MTU minus header overhead)
pub const MAX_PAYLOAD_SIZE: usize = MTU - 16;

/// Message types for the bridge protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum MessageType {
    /// Control messages for connection management
    Control = 0x01,
    /// Data payload (Android Auto stream)
    Data = 0x02,
    /// Acknowledgment
    Ack = 0x03,
    /// Negative acknowledgment / error
    Nack = 0x04,
    /// Keep-alive ping
    Ping = 0x05,
    /// Keep-alive pong response
    Pong = 0x06,
    /// Debug/logging message (dev only)
    Debug = 0xFF,
}

impl TryFrom<u8> for MessageType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Control),
            0x02 => Ok(Self::Data),
            0x03 => Ok(Self::Ack),
            0x04 => Ok(Self::Nack),
            0x05 => Ok(Self::Ping),
            0x06 => Ok(Self::Pong),
            0xFF => Ok(Self::Debug),
            _ => Err(()),
        }
    }
}

/// Message header containing metadata
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Header {
    /// Sequence number for ordering and deduplication
    pub sequence: u16,
    /// Total payload length
    pub payload_len: u16,
    /// Channel ID (for multiplexing different streams)
    pub channel: u8,
    /// Flags (reserved for future use)
    pub flags: u8,
}

impl Header {
    /// Create a new header with the given parameters
    pub fn new(sequence: u16, payload_len: u16, channel: u8) -> Self {
        Self {
            sequence,
            payload_len,
            channel,
            flags: 0,
        }
    }

    /// Size of the serialized header
    pub const fn serialized_size() -> usize {
        6 // 2 + 2 + 1 + 1
    }
}

/// Control message subtypes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ControlMessage {
    /// Initial handshake request from phone
    HandshakeRequest {
        /// Protocol version
        version: u8,
        /// Supported features bitmask
        features: u32,
    },
    /// Handshake response from ESP32
    HandshakeResponse {
        /// Accepted protocol version
        version: u8,
        /// Supported features bitmask
        features: u32,
        /// Session ID for this connection
        session_id: u32,
    },
    /// Start streaming request
    StartStream {
        /// Video channel ID
        video_channel: u8,
        /// Audio channel ID  
        audio_channel: u8,
        /// Input channel ID
        input_channel: u8,
    },
    /// Stop streaming request
    StopStream,
    /// Disconnect cleanly
    Disconnect {
        /// Reason code
        reason: u8,
    },
    /// Request current statistics
    StatsRequest,
    /// Statistics response
    StatsResponse {
        /// Bytes received
        bytes_rx: u64,
        /// Bytes transmitted
        bytes_tx: u64,
        /// Packets dropped
        packets_dropped: u32,
    },
}

/// Data payload wrapper with channel information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DataPayload<const N: usize = MAX_PAYLOAD_SIZE> {
    /// The actual data bytes
    #[serde(with = "heapless_serde")]
    pub data: Vec<u8, N>,
}

impl<const N: usize> DataPayload<N> {
    /// Create a new data payload from bytes
    pub fn new(data: &[u8]) -> Option<Self> {
        let mut vec = Vec::new();
        vec.extend_from_slice(data).ok()?;
        Some(Self { data: vec })
    }

    /// Get the payload length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if payload is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Helper module for serde with heapless::Vec
mod heapless_serde {
    use heapless::Vec;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S, const N: usize>(vec: &Vec<u8, N>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        vec.as_slice().serialize(serializer)
    }

    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<Vec<u8, N>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let slice: &[u8] = Deserialize::deserialize(deserializer)?;
        let mut vec = Vec::new();
        vec.extend_from_slice(slice)
            .map_err(serde::de::Error::custom)?;
        Ok(vec)
    }
}

/// Complete message enum for the bridge protocol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Message {
    /// Control message
    Control(ControlMessage),
    /// Data payload
    Data(DataPayload),
    /// Acknowledgment with sequence number
    Ack { sequence: u16 },
    /// Negative acknowledgment with error code
    Nack { sequence: u16, error: u8 },
    /// Keep-alive ping with timestamp
    Ping { timestamp: u32 },
    /// Keep-alive pong with original timestamp
    Pong { timestamp: u32 },
}

impl Message {
    /// Serialize the message to a buffer using postcard
    pub fn serialize<'a>(&self, buffer: &'a mut [u8]) -> Result<&'a [u8], postcard::Error> {
        postcard::to_slice(self, buffer)
    }

    /// Deserialize a message from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(data)
    }

    /// Get the message type
    pub fn message_type(&self) -> MessageType {
        match self {
            Message::Control(_) => MessageType::Control,
            Message::Data(_) => MessageType::Data,
            Message::Ack { .. } => MessageType::Ack,
            Message::Nack { .. } => MessageType::Nack,
            Message::Ping { .. } => MessageType::Ping,
            Message::Pong { .. } => MessageType::Pong,
        }
    }
}

/// Frame builder for constructing wire-format messages
pub struct FrameBuilder {
    sequence: u16,
}

impl FrameBuilder {
    /// Create a new frame builder
    pub const fn new() -> Self {
        Self { sequence: 0 }
    }

    /// Get the next sequence number and increment
    pub fn next_sequence(&mut self) -> u16 {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        seq
    }

    /// Build a complete frame with header and CRC
    ///
    /// Returns the number of bytes written to the buffer
    pub fn build_frame(
        &mut self,
        msg: &Message,
        channel: u8,
        buffer: &mut [u8],
    ) -> Result<usize, FrameError> {
        if buffer.len() < 16 {
            return Err(FrameError::BufferTooSmall);
        }

        // Write magic
        buffer[0..4].copy_from_slice(&FRAME_MAGIC);

        // Serialize message to temp buffer (skip header space)
        let payload_start = 11; // magic(4) + header(6) + type(1)
        let payload_buf = &mut buffer[payload_start..buffer.len() - 2];
        
        let payload = msg.serialize(payload_buf)
            .map_err(|_| FrameError::SerializationError)?;
        let payload_len = payload.len();

        // Build header
        let header = Header::new(
            self.next_sequence(),
            payload_len as u16,
            channel,
        );

        // Write header (manual serialization for fixed layout)
        buffer[4..6].copy_from_slice(&header.sequence.to_le_bytes());
        buffer[6..8].copy_from_slice(&header.payload_len.to_le_bytes());
        buffer[8] = header.channel;
        buffer[9] = header.flags;
        
        // Write message type
        buffer[10] = msg.message_type() as u8;

        // Calculate CRC over everything except CRC field itself
        let crc_data_len = payload_start + payload_len;
        let crc = crc16(&buffer[..crc_data_len]);
        
        // Write CRC
        buffer[crc_data_len..crc_data_len + 2].copy_from_slice(&crc.to_le_bytes());

        Ok(crc_data_len + 2)
    }

    /// Parse a frame from bytes
    pub fn parse_frame(data: &[u8]) -> Result<(Header, Message), FrameError> {
        if data.len() < 13 {
            return Err(FrameError::TooShort);
        }

        // Check magic
        if data[0..4] != FRAME_MAGIC {
            return Err(FrameError::InvalidMagic);
        }

        // Parse header
        let sequence = u16::from_le_bytes([data[4], data[5]]);
        let payload_len = u16::from_le_bytes([data[6], data[7]]) as usize;
        let channel = data[8];
        let flags = data[9];
        
        let header = Header {
            sequence,
            payload_len: payload_len as u16,
            channel,
            flags,
        };

        // Verify length
        let expected_len = 11 + payload_len + 2; // header + type + payload + crc
        if data.len() < expected_len {
            return Err(FrameError::TooShort);
        }

        // Verify CRC
        let crc_expected = u16::from_le_bytes([
            data[expected_len - 2],
            data[expected_len - 1],
        ]);
        let crc_actual = crc16(&data[..expected_len - 2]);
        if crc_expected != crc_actual {
            return Err(FrameError::CrcMismatch);
        }

        // Parse message
        let payload_data = &data[11..11 + payload_len];
        let message = Message::deserialize(payload_data)
            .map_err(|_| FrameError::DeserializationError)?;

        Ok((header, message))
    }
}

impl Default for FrameBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors during frame building/parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum FrameError {
    /// Buffer too small for frame
    BufferTooSmall,
    /// Frame data too short
    TooShort,
    /// Invalid magic bytes
    InvalidMagic,
    /// CRC check failed
    CrcMismatch,
    /// Serialization failed
    SerializationError,
    /// Deserialization failed
    DeserializationError,
}

/// Simple CRC-16-CCITT implementation
fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in data {
        crc ^= (*byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_roundtrip() {
        let msg = Message::Ping { timestamp: 12345 };
        let mut buffer = [0u8; 256];
        
        let serialized = msg.serialize(&mut buffer).unwrap();
        let deserialized = Message::deserialize(serialized).unwrap();
        
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_frame_roundtrip() {
        let mut builder = FrameBuilder::new();
        let msg = Message::Control(ControlMessage::HandshakeRequest {
            version: 1,
            features: 0xFF,
        });
        
        let mut buffer = [0u8; 512];
        let len = builder.build_frame(&msg, 0, &mut buffer).unwrap();
        
        let (header, parsed_msg) = FrameBuilder::parse_frame(&buffer[..len]).unwrap();
        
        assert_eq!(header.sequence, 0);
        assert_eq!(header.channel, 0);
        assert_eq!(msg, parsed_msg);
    }

    #[test]
    fn test_crc_verification() {
        let mut builder = FrameBuilder::new();
        let msg = Message::Ping { timestamp: 42 };
        
        let mut buffer = [0u8; 256];
        let len = builder.build_frame(&msg, 0, &mut buffer).unwrap();
        
        // Corrupt one byte
        buffer[5] ^= 0xFF;
        
        let result = FrameBuilder::parse_frame(&buffer[..len]);
        assert!(matches!(result, Err(FrameError::CrcMismatch)));
    }

    #[test]
    fn test_message_type_conversion() {
        assert_eq!(MessageType::try_from(0x01), Ok(MessageType::Control));
        assert_eq!(MessageType::try_from(0x02), Ok(MessageType::Data));
        assert!(MessageType::try_from(0x99).is_err());
    }
}
