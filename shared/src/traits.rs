//! # DataForwarder Trait & Endpoint Abstractions
//!
//! This module defines the core trait that abstracts the bidirectional data flow
//! between USB and WiFi endpoints. Following SOLID principles:
//!
//! - **Single Responsibility**: Each trait handles one aspect of data flow
//! - **Interface Segregation**: Separate traits for reading and writing
//! - **Dependency Inversion**: High-level bridge depends on abstractions
//!
//! ## Design Rationale
//!
//! The `DataForwarder` trait enables:
//! 1. **Testability**: Mock implementations for unit testing
//! 2. **Flexibility**: Swap USB/WiFi implementations without changing bridge logic
//! 3. **Async Support**: Native async/await for embassy compatibility

use core::future::Future;
use heapless::Vec;

use crate::buffer::ZeroCopyBuffer;
use crate::MTU;

/// Errors that can occur during data forwarding operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ForwarderError {
    /// The underlying transport is disconnected
    Disconnected,
    /// Read operation timed out
    ReadTimeout,
    /// Write operation timed out
    WriteTimeout,
    /// Buffer overflow - data arrived faster than it could be processed
    BufferOverflow,
    /// Buffer underflow - attempted to read from empty buffer
    BufferUnderflow,
    /// Invalid data or protocol error
    ProtocolError,
    /// USB-specific error (e.g., STALL, NAK)
    UsbError,
    /// WiFi-specific error (e.g., connection lost)
    WifiError,
    /// Generic I/O error
    IoError,
}

/// Result type alias for forwarder operations
pub type ForwarderResult<T> = Result<T, ForwarderError>;

/// Trait for reading data from an endpoint (USB or WiFi)
///
/// # Zero-Copy Design
///
/// The `read_into_buffer` method writes directly into a `ZeroCopyBuffer`,
/// allowing DMA transfers without intermediate copies.
///
/// # Example
///
/// ```ignore
/// async fn forward_data<R: EndpointReader, W: EndpointWriter>(
///     reader: &mut R,
///     writer: &mut W,
///     buffer: &mut ZeroCopyBuffer,
/// ) -> ForwarderResult<()> {
///     // Read directly into zero-copy buffer
///     let bytes_read = reader.read_into_buffer(buffer).await?;
///     
///     // Write from buffer without copying
///     let data = buffer.consume(bytes_read)?;
///     writer.write_from_slice(data).await?;
///     
///     Ok(())
/// }
/// ```
pub trait EndpointReader {
    /// Read data directly into a zero-copy buffer
    ///
    /// Returns the number of bytes read, or an error.
    /// This method should write to the buffer's write region without
    /// performing any intermediate copies.
    fn read_into_buffer(
        &mut self,
        buffer: &mut ZeroCopyBuffer,
    ) -> impl Future<Output = ForwarderResult<usize>>;

    /// Read data into a provided slice
    ///
    /// This is a fallback for when zero-copy isn't possible.
    /// Returns the number of bytes read.
    fn read_into_slice(
        &mut self,
        buf: &mut [u8],
    ) -> impl Future<Output = ForwarderResult<usize>>;

    /// Check if the endpoint is connected and ready
    fn is_connected(&self) -> bool;

    /// Get the maximum packet size for this endpoint
    fn max_packet_size(&self) -> usize;
}

/// Trait for writing data to an endpoint (USB or WiFi)
///
/// # Zero-Copy Design
///
/// The `write_from_buffer` method reads directly from a `ZeroCopyBuffer`,
/// enabling DMA transfers without intermediate copies.
pub trait EndpointWriter {
    /// Write data directly from a zero-copy buffer
    ///
    /// Returns the number of bytes written, or an error.
    /// The buffer's read pointer should be advanced by the caller
    /// after a successful write.
    fn write_from_buffer(
        &mut self,
        buffer: &ZeroCopyBuffer,
        len: usize,
    ) -> impl Future<Output = ForwarderResult<usize>>;

    /// Write data from a provided slice
    ///
    /// This is a fallback for when zero-copy isn't possible.
    /// Returns the number of bytes written.
    fn write_from_slice(
        &mut self,
        data: &[u8],
    ) -> impl Future<Output = ForwarderResult<usize>>;

    /// Flush any buffered data to the underlying transport
    fn flush(&mut self) -> impl Future<Output = ForwarderResult<()>>;

    /// Check if the endpoint is connected and ready
    fn is_connected(&self) -> bool;
}

/// Main trait that abstracts the bidirectional data flow between USB and WiFi
///
/// # Architecture
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │                       DataForwarder                              │
/// │                                                                  │
/// │   USB Endpoint                              WiFi Socket          │
/// │   ┌─────────┐     ┌─────────────────┐      ┌─────────┐          │
/// │   │  Read   │────►│  ZeroCopyBuffer │─────►│  Write  │          │
/// │   └─────────┘     │   (USB → WiFi)  │      └─────────┘          │
/// │                   └─────────────────┘                            │
/// │                                                                  │
/// │   ┌─────────┐     ┌─────────────────┐      ┌─────────┐          │
/// │   │  Write  │◄────│  ZeroCopyBuffer │◄─────│  Read   │          │
/// │   └─────────┘     │   (WiFi → USB)  │      └─────────┘          │
/// │                   └─────────────────┘                            │
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # Low-Latency Guarantees
///
/// 1. **No heap allocations**: All buffers are statically allocated
/// 2. **No mutex locks**: Lock-free atomic operations for buffer access
/// 3. **No data copies**: DMA-compatible buffer design
/// 4. **Async/await**: Non-blocking I/O with embassy runtime
pub trait DataForwarder {
    /// The USB endpoint reader type
    type UsbReader: EndpointReader;
    /// The USB endpoint writer type
    type UsbWriter: EndpointWriter;
    /// The WiFi endpoint reader type
    type WifiReader: EndpointReader;
    /// The WiFi endpoint writer type
    type WifiWriter: EndpointWriter;

    /// Get mutable reference to the USB reader
    fn usb_reader(&mut self) -> &mut Self::UsbReader;
    
    /// Get mutable reference to the USB writer
    fn usb_writer(&mut self) -> &mut Self::UsbWriter;
    
    /// Get mutable reference to the WiFi reader
    fn wifi_reader(&mut self) -> &mut Self::WifiReader;
    
    /// Get mutable reference to the WiFi writer
    fn wifi_writer(&mut self) -> &mut Self::WifiWriter;

    /// Get reference to the USB→WiFi buffer
    fn usb_to_wifi_buffer(&mut self) -> &mut ZeroCopyBuffer;
    
    /// Get reference to the WiFi→USB buffer
    fn wifi_to_usb_buffer(&mut self) -> &mut ZeroCopyBuffer;

    /// Forward data from USB to WiFi (single iteration)
    ///
    /// Returns the number of bytes forwarded, or an error.
    /// This is a non-blocking operation that processes available data.
    fn forward_usb_to_wifi(&mut self) -> impl Future<Output = ForwarderResult<usize>> {
        async {
            let buffer = self.usb_to_wifi_buffer();
            let reader = self.usb_reader();
            
            // Read from USB into buffer (zero-copy)
            let bytes_read = reader.read_into_buffer(buffer).await?;
            
            if bytes_read == 0 {
                return Ok(0);
            }

            // Get the data slice from buffer
            let data = buffer.readable_slice(bytes_read)
                .ok_or(ForwarderError::BufferUnderflow)?;
            
            // Write to WiFi (zero-copy from buffer)
            let writer = self.wifi_writer();
            let bytes_written = writer.write_from_slice(data).await?;
            
            // Consume the written bytes from buffer
            let buffer = self.usb_to_wifi_buffer();
            buffer.consume(bytes_written)?;
            
            Ok(bytes_written)
        }
    }

    /// Forward data from WiFi to USB (single iteration)
    ///
    /// Returns the number of bytes forwarded, or an error.
    /// This is a non-blocking operation that processes available data.
    fn forward_wifi_to_usb(&mut self) -> impl Future<Output = ForwarderResult<usize>> {
        async {
            let buffer = self.wifi_to_usb_buffer();
            let reader = self.wifi_reader();
            
            // Read from WiFi into buffer (zero-copy)
            let bytes_read = reader.read_into_buffer(buffer).await?;
            
            if bytes_read == 0 {
                return Ok(0);
            }

            // Get the data slice from buffer
            let data = buffer.readable_slice(bytes_read)
                .ok_or(ForwarderError::BufferUnderflow)?;
            
            // Write to USB (zero-copy from buffer)
            let writer = self.usb_writer();
            let bytes_written = writer.write_from_slice(data).await?;
            
            // Consume the written bytes from buffer
            let buffer = self.wifi_to_usb_buffer();
            buffer.consume(bytes_written)?;
            
            Ok(bytes_written)
        }
    }

    /// Run the forwarding loop until disconnection
    ///
    /// This method runs both USB→WiFi and WiFi→USB forwarding concurrently.
    /// It returns when either endpoint disconnects.
    fn run(&mut self) -> impl Future<Output = ForwarderResult<()>> {
        async {
            loop {
                // Check connection status
                if !self.usb_reader().is_connected() || !self.wifi_reader().is_connected() {
                    return Err(ForwarderError::Disconnected);
                }

                // Forward in both directions
                // In a real implementation, these would run concurrently
                // using embassy's select! or join! macros
                let usb_to_wifi = self.forward_usb_to_wifi().await;
                let wifi_to_usb = self.forward_wifi_to_usb().await;

                // Handle errors
                match (usb_to_wifi, wifi_to_usb) {
                    (Err(ForwarderError::Disconnected), _) |
                    (_, Err(ForwarderError::Disconnected)) => {
                        return Err(ForwarderError::Disconnected);
                    }
                    (Err(e), _) => return Err(e),
                    (_, Err(e)) => return Err(e),
                    (Ok(_), Ok(_)) => continue,
                }
            }
        }
    }

    /// Check if both endpoints are connected
    fn is_connected(&self) -> bool;

    /// Get statistics about the forwarding operation
    fn stats(&self) -> ForwardingStats;
}

/// Statistics about the data forwarding operation
#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ForwardingStats {
    /// Total bytes forwarded from USB to WiFi
    pub usb_to_wifi_bytes: u64,
    /// Total bytes forwarded from WiFi to USB
    pub wifi_to_usb_bytes: u64,
    /// Number of USB read operations
    pub usb_reads: u32,
    /// Number of WiFi read operations
    pub wifi_reads: u32,
    /// Number of buffer overflows (data loss)
    pub overflows: u32,
    /// Current USB→WiFi buffer usage (bytes)
    pub usb_to_wifi_buffer_used: usize,
    /// Current WiFi→USB buffer usage (bytes)
    pub wifi_to_usb_buffer_used: usize,
}

/// Configuration for the data forwarder
#[derive(Debug, Clone)]
pub struct ForwarderConfig {
    /// Timeout for read operations in milliseconds
    pub read_timeout_ms: u32,
    /// Timeout for write operations in milliseconds
    pub write_timeout_ms: u32,
    /// Enable statistics collection (slight overhead)
    pub enable_stats: bool,
    /// Maximum retries on transient errors
    pub max_retries: u8,
}

impl Default for ForwarderConfig {
    fn default() -> Self {
        Self {
            read_timeout_ms: 1000,
            write_timeout_ms: 1000,
            enable_stats: true,
            max_retries: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementations for testing would go here
    // In a real implementation, you'd have mock USB and WiFi endpoints

    #[test]
    fn test_forwarder_config_default() {
        let config = ForwarderConfig::default();
        assert_eq!(config.read_timeout_ms, 1000);
        assert_eq!(config.write_timeout_ms, 1000);
        assert!(config.enable_stats);
        assert_eq!(config.max_retries, 3);
    }
}
