use crate::{Error, Result};

pub struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    pub fn position(&self) -> (usize, u8) {
        (self.byte_pos, self.bit_pos)
    }

    pub fn seek(&mut self, byte_pos: usize, bit_pos: u8) -> Result<()> {
        if byte_pos >= self.data.len() || (byte_pos == self.data.len() - 1 && bit_pos > 7) {
            return Err(Error::BitstreamError("Seek position out of bounds".into()));
        }
        self.byte_pos = byte_pos;
        self.bit_pos = bit_pos;
        Ok(())
    }

    pub fn available_bits(&self) -> usize {
        if self.byte_pos >= self.data.len() {
            return 0;
        }
        (self.data.len() - self.byte_pos - 1) * 8 + (8 - self.bit_pos as usize)
    }

    pub fn read_bit(&mut self) -> Result<bool> {
        if self.byte_pos >= self.data.len() {
            return Err(Error::UnexpectedEof);
        }

        let bit = (self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1;
        
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }

        Ok(bit != 0)
    }

    pub fn read_bits(&mut self, n: u32) -> Result<u32> {
        if n > 32 {
            return Err(Error::BitstreamError("Cannot read more than 32 bits".into()));
        }

        let mut value = 0u32;
        for _ in 0..n {
            value = (value << 1) | (self.read_bit()? as u32);
        }
        Ok(value)
    }

    pub fn read_flag(&mut self) -> Result<bool> {
        self.read_bit()
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        self.read_bits(8).map(|v| v as u8)
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        self.read_bits(16).map(|v| v as u16)
    }

    pub fn peek_bits(&mut self, n: u32) -> Result<u32> {
        let saved_byte = self.byte_pos;
        let saved_bit = self.bit_pos;
        
        let value = self.read_bits(n)?;
        
        self.byte_pos = saved_byte;
        self.bit_pos = saved_bit;
        
        Ok(value)
    }

    pub fn skip_bits(&mut self, n: u32) -> Result<()> {
        for _ in 0..n {
            self.read_bit()?;
        }
        Ok(())
    }

    pub fn byte_aligned(&self) -> bool {
        self.bit_pos == 0
    }

    pub fn align_to_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }

    pub fn more_rbsp_data(&self) -> bool {
        if self.byte_pos >= self.data.len() {
            return false;
        }

        if self.byte_pos == self.data.len() - 1 {
            let remaining_byte = self.data[self.byte_pos];
            if self.bit_pos >= 8 {
                return false;
            }
            let bits_left = 8 - self.bit_pos;
            if bits_left == 0 || bits_left > 8 {
                return false;
            }
            
            // Get the remaining bits from current position
            let shift_amount = self.bit_pos;
            let remaining_bits = remaining_byte << shift_amount;
            
            // Check if remaining bits match the RBSP stop bit pattern
            // The stop bit pattern is a single 1 followed by zeros
            // In the most significant position after shifting
            let stop_pattern = 0x80; // 10000000
            
            return remaining_bits != stop_pattern;
        }

        true
    }

    pub fn rbsp_trailing_bits(&mut self) -> Result<()> {
        if !self.read_flag()? {
            return Err(Error::BitstreamError("Expected rbsp_stop_one_bit".into()));
        }

        while !self.byte_aligned() {
            if self.read_flag()? {
                return Err(Error::BitstreamError("Expected rbsp_alignment_zero_bit".into()));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_bits() {
        let data = vec![0b10110011, 0b01010101];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_bits(4).unwrap(), 0b1011);
        assert_eq!(reader.read_bits(4).unwrap(), 0b0011);
        assert_eq!(reader.read_bits(8).unwrap(), 0b01010101);
    }

    #[test]
    fn test_read_flag() {
        let data = vec![0b10000000, 0b01000000];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_flag().unwrap(), true);
        assert_eq!(reader.read_flag().unwrap(), false);
    }

    #[test]
    fn test_peek_bits() {
        let data = vec![0b11110000];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.peek_bits(4).unwrap(), 0b1111);
        assert_eq!(reader.read_bits(4).unwrap(), 0b1111);
        assert_eq!(reader.read_bits(4).unwrap(), 0b0000);
    }

    #[test]
    fn test_byte_alignment() {
        let data = vec![0xff, 0x00];
        let mut reader = BitReader::new(&data);

        assert!(reader.byte_aligned());
        reader.read_bits(3).unwrap();
        assert!(!reader.byte_aligned());
        reader.align_to_byte();
        assert!(reader.byte_aligned());
        assert_eq!(reader.byte_pos, 1);
    }

    #[test]
    fn test_more_rbsp_data() {
        // Test case: 0x80 = 10000000
        // This is the RBSP stop bit (1) followed by alignment zeros
        let data = vec![0x80];
        let reader = BitReader::new(&data);
        
        // At the beginning with byte_pos=0, bit_pos=0
        // We're looking at the last byte with 8 bits remaining: 10000000
        // This exactly matches the stop bit pattern, so no more RBSP data
        assert!(!reader.more_rbsp_data());
        
        // Test another case: actual data before stop bit
        let data = vec![0xC0]; // 11000000 - has actual data before stop bit
        let reader = BitReader::new(&data);
        assert!(reader.more_rbsp_data());
    }
}