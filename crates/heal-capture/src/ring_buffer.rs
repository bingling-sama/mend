/// Fixed-capacity in-memory Ring Buffer to prevent unbounded memory growth while capturing
/// terminal streams (default 64KB as specified in PRD).
#[derive(Debug, Clone)]
pub struct RingBuffer {
    buffer: Vec<u8>,
    capacity: usize,
    write_pos: usize,
    is_full: bool,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than 0");
        Self {
            buffer: vec![0; capacity],
            capacity,
            write_pos: 0,
            is_full: false,
        }
    }

    pub fn with_default_capacity() -> Self {
        // 64 KB default as specified in PLAN.md and PRD
        Self::new(64 * 1024)
    }

    pub fn push_byte(&mut self, byte: u8) {
        self.buffer[self.write_pos] = byte;
        self.write_pos = (self.write_pos + 1) % self.capacity;
        if self.write_pos == 0 {
            self.is_full = true;
        }
    }

    pub fn write_all(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.push_byte(b);
        }
    }

    pub fn len(&self) -> usize {
        if self.is_full {
            self.capacity
        } else {
            self.write_pos
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.is_full && self.write_pos == 0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        if !self.is_full {
            self.buffer[..self.write_pos].to_vec()
        } else {
            let mut out = Vec::with_capacity(self.capacity);
            out.extend_from_slice(&self.buffer[self.write_pos..]);
            out.extend_from_slice(&self.buffer[..self.write_pos]);
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_under_capacity() {
        let mut rb = RingBuffer::new(5);
        rb.write_all(b"abc");
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.to_vec(), b"abc");
    }

    #[test]
    fn test_ring_buffer_over_capacity() {
        let mut rb = RingBuffer::new(5);
        rb.write_all(b"abcdefgh");
        assert_eq!(rb.len(), 5);
        assert_eq!(rb.to_vec(), b"defgh");
    }
}
