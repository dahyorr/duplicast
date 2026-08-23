use bytes::{Bytes, BytesMut, Buf};

/// FLV tag parser that handles incomplete chunks and reassembles complete tags
pub struct FlvTagParser {
    buffer: BytesMut,
}

impl FlvTagParser {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(8192),
        }
    }

    /// Add data to the parser buffer
    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Extract the next complete FLV tag if available
    /// Returns (tag_with_previous_tag_size, remaining) or None if incomplete
    pub fn next_tag(&mut self) -> Option<Bytes> {
        // Skip FLV header if present (first 13 bytes: 9 byte header + 4 byte previous tag size)
        if self.buffer.len() >= 13 && &self.buffer[0..3] == b"FLV" {
            // Skip FLV header (9 bytes) + first PreviousTagSize (4 bytes)
            self.buffer.advance(13);
        }

        // Need at least 11 bytes for FLV tag header
        if self.buffer.len() < 11 {
            return None;
        }

        // Parse FLV tag header
        let tag_type = self.buffer[0];
        let data_size = u32::from_be_bytes([0, self.buffer[1], self.buffer[2], self.buffer[3]]) as usize;
        
        // Total tag size: 11 (header) + data_size + 4 (previous tag size)
        let total_size = 11 + data_size + 4;

        // Check if we have the complete tag
        if self.buffer.len() < total_size {
            return None;
        }

        // Validate tag type
        if !matches!(tag_type, 0x08 | 0x09 | 0x12) {
            eprintln!("⚠️ Invalid FLV tag type: 0x{:02x}, skipping byte", tag_type);
            self.buffer.advance(1);
            return self.next_tag(); // Try again with next byte
        }

        // Extract complete tag including the following PreviousTagSize
        let tag_data = self.buffer.split_to(total_size);
        
        Some(tag_data.freeze())
    }

    /// Get the current buffer size
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    /// Clear the internal buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incomplete_tag() {
        let mut parser = FlvTagParser::new();
        
        // Feed incomplete tag header
        parser.feed(&[0x09, 0x00, 0x01, 0x00]); // Only 4 bytes
        
        assert_eq!(parser.next_tag(), None);
    }

    #[test]
    fn test_complete_tag() {
        let mut parser = FlvTagParser::new();
        
        // Create a minimal valid FLV tag (11 byte header + 1 byte data + 4 byte prev size)
        let mut tag = vec![
            0x09,       // Tag type (video)
            0x00, 0x00, 0x01, // Data size (1 byte)
            0x00, 0x00, 0x00, // Timestamp
            0x00,       // Timestamp extended
            0x00, 0x00, 0x00, // Stream ID
            0xFF,       // Data (1 byte)
            0x00, 0x00, 0x00, 0x10, // Previous tag size (16)
        ];
        
        parser.feed(&tag);
        
        let result = parser.next_tag();
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 16);
    }
}
