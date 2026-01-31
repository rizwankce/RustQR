//! Memory pool and arena allocator for reducing allocation overhead
//!
//! Provides pre-allocated buffers for:
//! - Grayscale conversion (reusable buffer)
//! - Temporary vectors for detection pipeline
//! - Finder pattern candidate storage

/// A simple arena allocator that reuses a fixed-size buffer
pub struct BufferPool {
    // Pre-allocated grayscale buffer (max size: 1920x1080)
    grayscale_buffer: Vec<u8>,
    grayscale_capacity: usize,
}

impl BufferPool {
    /// Create a new buffer pool with default capacity (2MB for grayscale)
    pub fn new() -> Self {
        let default_capacity = 1920 * 1080; // Support up to 1080p
        Self {
            grayscale_buffer: Vec::with_capacity(default_capacity),
            grayscale_capacity: default_capacity,
        }
    }

    /// Create a pool with custom capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            grayscale_buffer: Vec::with_capacity(capacity),
            grayscale_capacity: capacity,
        }
    }

    /// Get a grayscale buffer of the required size (reuses if possible)
    /// Returns a mutable slice that can be used for grayscale conversion
    pub fn get_grayscale_buffer(&mut self, size: usize) -> &mut [u8] {
        if size > self.grayscale_capacity {
            // Need to grow the buffer
            self.grayscale_buffer
                .reserve(size - self.grayscale_capacity);
            self.grayscale_capacity = size;
        }

        // Safety: We're ensuring the buffer has enough capacity
        unsafe {
            self.grayscale_buffer.set_len(size);
        }

        &mut self.grayscale_buffer[..size]
    }

    /// Resize the internal buffer if needed
    pub fn ensure_grayscale_capacity(&mut self, capacity: usize) {
        if capacity > self.grayscale_capacity {
            self.grayscale_buffer
                .reserve(capacity - self.grayscale_capacity);
            self.grayscale_capacity = capacity;
        }
    }

    /// Get the current grayscale buffer capacity
    pub fn grayscale_capacity(&self) -> usize {
        self.grayscale_capacity
    }

    /// Clear all buffers (resets lengths but keeps capacity)
    pub fn clear(&mut self) {
        self.grayscale_buffer.clear();
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for monitoring allocation patterns
#[derive(Debug, Default)]
pub struct AllocationStats {
    pub grayscale_reuses: usize,
    pub grayscale_allocations: usize,
    pub total_bytes_reused: usize,
}

impl AllocationStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_grayscale_reuse(&mut self, bytes: usize) {
        self.grayscale_reuses += 1;
        self.total_bytes_reused += bytes;
    }

    pub fn record_grayscale_allocation(&mut self) {
        self.grayscale_allocations += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_basic() {
        let mut pool = BufferPool::new();

        // Get a small buffer
        let buf1 = pool.get_grayscale_buffer(100);
        assert_eq!(buf1.len(), 100);

        // Get a larger buffer (should reuse capacity)
        let buf2 = pool.get_grayscale_buffer(1000);
        assert_eq!(buf2.len(), 1000);

        // Capacity should be at least 1920*1080 (default)
        assert!(pool.grayscale_capacity() >= 1920 * 1080);
    }

    #[test]
    fn test_buffer_pool_growth() {
        let mut pool = BufferPool::with_capacity(100);

        // Get buffer larger than initial capacity
        let buf = pool.get_grayscale_buffer(500);
        assert_eq!(buf.len(), 500);
        assert!(pool.grayscale_capacity() >= 500);
    }
}
