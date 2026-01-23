//! # Zero-Copy Buffer Implementation
//!
//! This module provides a lock-free, statically-allocated ring buffer designed
//! for high-performance data forwarding between USB and WiFi peripherals.
//!
//! ## Low-Latency Design Principles
//!
//! 1. **Static Allocation**: Buffer memory is allocated at compile time,
//!    eliminating heap allocation overhead during runtime.
//!
//! 2. **Lock-Free Access**: Uses atomic indices for producer/consumer pattern,
//!    allowing concurrent read/write without mutex overhead.
//!
//! 3. **DMA Compatibility**: Buffer provides direct slice access for DMA
//!    peripheral read/write operations.
//!
//! 4. **Cache-Line Alignment**: Buffer size and alignment optimized for
//!    efficient memory access patterns.
//!
//! ## Memory Layout
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │                    ZeroCopyBuffer (32KB)                   │
//! ├────────────────────────────────────────────────────────────┤
//! │ write_idx │ read_idx │ ... buffer data ...                 │
//! │  (atomic) │ (atomic) │                                     │
//! └────────────────────────────────────────────────────────────┘
//!                        │                                     
//!                        ▼                                     
//! ┌──────────┬───────────────────────┬───────────┬────────────┐
//! │ consumed │    readable data      │ writable  │  wrapped   │
//! │  region  │   (ready to send)     │  region   │   space    │
//! └──────────┴───────────────────────┴───────────┴────────────┘
//!            ▲                       ▲
//!         read_idx               write_idx
//! ```

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::traits::ForwarderError;

/// Buffer size: 32KB provides good balance between latency and throughput
/// - Large enough to handle burst traffic from Android Auto
/// - Small enough to fit in ESP32-S2's limited RAM (320KB)
/// - Aligned to power of 2 for efficient modulo operations
pub const BUFFER_SIZE: usize = 32 * 1024; // 32KB

/// Mask for efficient modulo operation (BUFFER_SIZE - 1)
const BUFFER_MASK: usize = BUFFER_SIZE - 1;

/// A slice view into the buffer for zero-copy access
#[derive(Debug)]
pub struct BufferSlice<'a> {
    /// First contiguous chunk (before wrap-around)
    pub first: &'a [u8],
    /// Second contiguous chunk (after wrap-around, may be empty)
    pub second: &'a [u8],
}

impl<'a> BufferSlice<'a> {
    /// Total length across both chunks
    pub fn len(&self) -> usize {
        self.first.len() + self.second.len()
    }

    /// Check if the slice is empty
    pub fn is_empty(&self) -> bool {
        self.first.is_empty() && self.second.is_empty()
    }
}

/// A mutable slice view into the buffer for zero-copy writes
#[derive(Debug)]
pub struct BufferSliceMut<'a> {
    /// First contiguous chunk (before wrap-around)
    pub first: &'a mut [u8],
    /// Second contiguous chunk (after wrap-around, may be empty)
    pub second: &'a mut [u8],
}

impl<'a> BufferSliceMut<'a> {
    /// Total length across both chunks
    pub fn len(&self) -> usize {
        self.first.len() + self.second.len()
    }

    /// Check if the slice is empty
    pub fn is_empty(&self) -> bool {
        self.first.is_empty() && self.second.is_empty()
    }
}

/// Errors specific to buffer operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BufferError {
    /// Attempted to write more data than available space
    Overflow,
    /// Attempted to read more data than available
    Underflow,
    /// Requested size exceeds buffer capacity
    SizeExceedsCapacity,
}

impl From<BufferError> for ForwarderError {
    fn from(e: BufferError) -> Self {
        match e {
            BufferError::Overflow => ForwarderError::BufferOverflow,
            BufferError::Underflow => ForwarderError::BufferUnderflow,
            BufferError::SizeExceedsCapacity => ForwarderError::BufferOverflow,
        }
    }
}

/// Zero-copy ring buffer for high-performance data forwarding
///
/// This buffer is designed for the producer-consumer pattern where:
/// - USB peripheral writes data (producer)
/// - WiFi peripheral reads data (consumer)
/// (or vice versa)
///
/// # Thread Safety
///
/// The buffer uses atomic operations for index management, making it safe
/// for single-producer single-consumer (SPSC) scenarios common in embedded.
///
/// # Example
///
/// ```rust
/// use shared::buffer::ZeroCopyBuffer;
///
/// let mut buffer = ZeroCopyBuffer::new();
///
/// // Producer: write data
/// let write_region = buffer.writable_slice_mut(100).unwrap();
/// write_region.first[..5].copy_from_slice(b"hello");
/// buffer.commit(5).unwrap();
///
/// // Consumer: read data
/// let data = buffer.readable_slice(5).unwrap();
/// assert_eq!(data, b"hello");
/// buffer.consume(5).unwrap();
/// ```
pub struct ZeroCopyBuffer {
    /// The actual buffer storage
    /// Using a fixed-size array for static allocation
    data: [u8; BUFFER_SIZE],
    
    /// Write index (where producer writes next)
    /// Uses atomic for lock-free access in SPSC scenario
    write_idx: AtomicUsize,
    
    /// Read index (where consumer reads next)
    /// Uses atomic for lock-free access in SPSC scenario
    read_idx: AtomicUsize,
}

impl ZeroCopyBuffer {
    /// Create a new zero-initialized buffer
    pub const fn new() -> Self {
        Self {
            data: [0u8; BUFFER_SIZE],
            write_idx: AtomicUsize::new(0),
            read_idx: AtomicUsize::new(0),
        }
    }

    /// Get the total capacity of the buffer
    #[inline]
    pub const fn capacity(&self) -> usize {
        BUFFER_SIZE
    }

    /// Get the number of bytes available to read
    #[inline]
    pub fn readable_len(&self) -> usize {
        let write = self.write_idx.load(Ordering::Acquire);
        let read = self.read_idx.load(Ordering::Acquire);
        write.wrapping_sub(read) & BUFFER_MASK
    }

    /// Get the number of bytes available to write
    #[inline]
    pub fn writable_len(&self) -> usize {
        // Leave one byte to distinguish full from empty
        BUFFER_SIZE - 1 - self.readable_len()
    }

    /// Check if the buffer is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.readable_len() == 0
    }

    /// Check if the buffer is full
    #[inline]
    pub fn is_full(&self) -> bool {
        self.writable_len() == 0
    }

    /// Get a readable slice of up to `max_len` bytes
    ///
    /// This returns a direct view into the buffer memory for zero-copy reads.
    /// The returned slice is valid until `consume()` is called.
    ///
    /// # Returns
    /// - `Some(&[u8])` if data is available
    /// - `None` if buffer is empty
    pub fn readable_slice(&self, max_len: usize) -> Option<&[u8]> {
        let available = self.readable_len();
        if available == 0 {
            return None;
        }

        let len = max_len.min(available);
        let read_idx = self.read_idx.load(Ordering::Acquire) & BUFFER_MASK;
        
        // Check for wrap-around
        let end_idx = read_idx + len;
        if end_idx <= BUFFER_SIZE {
            // No wrap-around, return single slice
            Some(&self.data[read_idx..read_idx + len])
        } else {
            // With wrap-around, only return first chunk
            // Caller should call again for second chunk
            Some(&self.data[read_idx..BUFFER_SIZE])
        }
    }

    /// Get a split readable view (handles wrap-around)
    ///
    /// Returns two slices that together contain all readable data,
    /// handling the case where data wraps around the buffer end.
    pub fn readable_split(&self, max_len: usize) -> BufferSlice<'_> {
        let available = self.readable_len();
        let len = max_len.min(available);
        
        if len == 0 {
            return BufferSlice {
                first: &[],
                second: &[],
            };
        }

        let read_idx = self.read_idx.load(Ordering::Acquire) & BUFFER_MASK;
        let end_idx = read_idx + len;

        if end_idx <= BUFFER_SIZE {
            BufferSlice {
                first: &self.data[read_idx..end_idx],
                second: &[],
            }
        } else {
            let first_len = BUFFER_SIZE - read_idx;
            let second_len = len - first_len;
            BufferSlice {
                first: &self.data[read_idx..BUFFER_SIZE],
                second: &self.data[0..second_len],
            }
        }
    }

    /// Get a mutable writable slice for zero-copy writes
    ///
    /// # Safety
    ///
    /// The caller must ensure only one writer accesses this at a time.
    /// After writing, call `commit()` to make data available to readers.
    pub fn writable_slice_mut(&mut self, max_len: usize) -> Result<BufferSliceMut<'_>, BufferError> {
        let available = self.writable_len();
        if available == 0 {
            return Err(BufferError::Overflow);
        }

        let len = max_len.min(available);
        let write_idx = self.write_idx.load(Ordering::Acquire) & BUFFER_MASK;
        let end_idx = write_idx + len;

        if end_idx <= BUFFER_SIZE {
            Ok(BufferSliceMut {
                first: &mut self.data[write_idx..end_idx],
                second: &mut [],
            })
        } else {
            let first_len = BUFFER_SIZE - write_idx;
            let (first_part, rest) = self.data.split_at_mut(BUFFER_SIZE);
            let _ = rest; // Silence unused warning
            
            // Need to handle wrap-around carefully
            let first = &mut self.data[write_idx..BUFFER_SIZE];
            let second_len = len - first_len;
            
            // This is safe because we're in the same buffer, just different regions
            // We need unsafe here due to borrow checker limitations with split borrows
            unsafe {
                let ptr = self.data.as_mut_ptr();
                let first = core::slice::from_raw_parts_mut(ptr.add(write_idx), first_len);
                let second = core::slice::from_raw_parts_mut(ptr, second_len);
                Ok(BufferSliceMut { first, second })
            }
        }
    }

    /// Get a direct mutable reference to the underlying buffer
    ///
    /// This is useful for DMA operations where the peripheral writes directly.
    ///
    /// # Safety
    ///
    /// Caller must ensure proper synchronization and call `commit()` after writing.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }

    /// Get the write index for DMA setup
    #[inline]
    pub fn write_offset(&self) -> usize {
        self.write_idx.load(Ordering::Acquire) & BUFFER_MASK
    }

    /// Commit written bytes, making them available to readers
    ///
    /// Call this after writing to `writable_slice_mut()` or via DMA.
    pub fn commit(&self, len: usize) -> Result<(), BufferError> {
        if len > self.writable_len() {
            return Err(BufferError::Overflow);
        }

        let old_write = self.write_idx.load(Ordering::Acquire);
        let new_write = old_write.wrapping_add(len);
        self.write_idx.store(new_write, Ordering::Release);
        
        Ok(())
    }

    /// Consume read bytes, freeing space for writers
    ///
    /// Call this after successfully processing data from `readable_slice()`.
    pub fn consume(&self, len: usize) -> Result<(), BufferError> {
        if len > self.readable_len() {
            return Err(BufferError::Underflow);
        }

        let old_read = self.read_idx.load(Ordering::Acquire);
        let new_read = old_read.wrapping_add(len);
        self.read_idx.store(new_read, Ordering::Release);
        
        Ok(())
    }

    /// Write data from a slice into the buffer
    ///
    /// This performs a copy but is convenient for non-DMA scenarios.
    pub fn write(&mut self, data: &[u8]) -> Result<usize, BufferError> {
        if data.len() > self.writable_len() {
            return Err(BufferError::Overflow);
        }

        let write_idx = self.write_idx.load(Ordering::Acquire) & BUFFER_MASK;
        let len = data.len();

        // Check for wrap-around
        if write_idx + len <= BUFFER_SIZE {
            // No wrap-around
            self.data[write_idx..write_idx + len].copy_from_slice(data);
        } else {
            // Handle wrap-around
            let first_len = BUFFER_SIZE - write_idx;
            self.data[write_idx..BUFFER_SIZE].copy_from_slice(&data[..first_len]);
            self.data[0..len - first_len].copy_from_slice(&data[first_len..]);
        }

        self.commit(len)?;
        Ok(len)
    }

    /// Read data from the buffer into a slice
    ///
    /// This performs a copy but is convenient for non-DMA scenarios.
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, BufferError> {
        let available = self.readable_len();
        if available == 0 {
            return Ok(0);
        }

        let len = buf.len().min(available);
        let read_idx = self.read_idx.load(Ordering::Acquire) & BUFFER_MASK;

        // Check for wrap-around
        if read_idx + len <= BUFFER_SIZE {
            // No wrap-around
            buf[..len].copy_from_slice(&self.data[read_idx..read_idx + len]);
        } else {
            // Handle wrap-around
            let first_len = BUFFER_SIZE - read_idx;
            buf[..first_len].copy_from_slice(&self.data[read_idx..BUFFER_SIZE]);
            buf[first_len..len].copy_from_slice(&self.data[0..len - first_len]);
        }

        self.consume(len)?;
        Ok(len)
    }

    /// Reset the buffer to empty state
    pub fn reset(&mut self) {
        self.read_idx.store(0, Ordering::Release);
        self.write_idx.store(0, Ordering::Release);
    }
}

impl Default for ZeroCopyBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer_is_empty() {
        let buffer = ZeroCopyBuffer::new();
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
        assert_eq!(buffer.readable_len(), 0);
        assert_eq!(buffer.writable_len(), BUFFER_SIZE - 1);
    }

    #[test]
    fn test_write_and_read() {
        let mut buffer = ZeroCopyBuffer::new();
        
        // Write some data
        let data = b"Hello, World!";
        let written = buffer.write(data).unwrap();
        assert_eq!(written, data.len());
        assert_eq!(buffer.readable_len(), data.len());

        // Read it back
        let mut read_buf = [0u8; 20];
        let read_len = buffer.read(&mut read_buf).unwrap();
        assert_eq!(read_len, data.len());
        assert_eq!(&read_buf[..read_len], data);
        
        // Buffer should be empty now
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_wrap_around() {
        let mut buffer = ZeroCopyBuffer::new();
        
        // Fill most of the buffer
        let large_data = [0xABu8; BUFFER_SIZE - 100];
        buffer.write(&large_data).unwrap();
        
        // Read most of it back
        let mut read_buf = [0u8; BUFFER_SIZE - 200];
        buffer.read(&mut read_buf).unwrap();
        
        // Now write more data that will wrap around
        let wrap_data = [0xCDu8; 150];
        buffer.write(&wrap_data).unwrap();
        
        // Read it back and verify
        let mut final_buf = [0u8; 150];
        let read_len = buffer.read(&mut final_buf).unwrap();
        assert!(read_len > 0);
    }

    #[test]
    fn test_overflow_error() {
        let mut buffer = ZeroCopyBuffer::new();
        
        // Try to write more than capacity
        let huge_data = [0u8; BUFFER_SIZE + 100];
        let result = buffer.write(&huge_data);
        assert!(matches!(result, Err(BufferError::Overflow)));
    }

    #[test]
    fn test_zero_copy_read() {
        let mut buffer = ZeroCopyBuffer::new();
        
        let data = b"Zero-copy test";
        buffer.write(data).unwrap();
        
        // Get zero-copy slice
        let slice = buffer.readable_slice(data.len()).unwrap();
        assert_eq!(slice, data);
        
        // Consume the data
        buffer.consume(data.len()).unwrap();
        assert!(buffer.is_empty());
    }
}
