use crate::bitreader::BitReader;
use crate::{Error, Result};

pub fn read_ue(reader: &mut BitReader) -> Result<u32> {
    let mut leading_zeros = 0;
    
    while !reader.read_bit()? {
        leading_zeros += 1;
        if leading_zeros > 31 {
            return Err(Error::BitstreamError("Invalid exp-golomb code".into()));
        }
    }

    if leading_zeros == 0 {
        return Ok(0);
    }

    let code_value = reader.read_bits(leading_zeros)?;
    Ok((1 << leading_zeros) - 1 + code_value)
}

pub fn read_se(reader: &mut BitReader) -> Result<i32> {
    let code_num = read_ue(reader)?;
    
    // H.264 spec mapping:
    // code_num = 0 => 0
    // code_num = 1 => 1
    // code_num = 2 => -1
    // code_num = 3 => 2
    // code_num = 4 => -2
    // Pattern: odd values are positive, even values (except 0) are negative
    
    if code_num == 0 {
        Ok(0)
    } else if code_num & 1 == 1 {
        // Odd: positive value
        Ok(((code_num + 1) >> 1) as i32)
    } else {
        // Even: negative value
        Ok(-((code_num >> 1) as i32))
    }
}

pub fn read_me(reader: &mut BitReader, chroma_format_idc: u8) -> Result<u32> {
    match chroma_format_idc {
        1 | 2 => {
            let code_num = read_ue(reader)?;
            if code_num > 2 {
                return Err(Error::BitstreamError("Invalid mapped exp-golomb code".into()));
            }
            Ok(code_num)
        }
        _ => read_ue(reader),
    }
}

pub fn read_te(reader: &mut BitReader, max_value: u32) -> Result<u32> {
    if max_value == 0 {
        return Ok(0);
    }

    if max_value == 1 {
        let bit = reader.read_bit()?;
        return Ok(1 - bit as u32);
    }

    read_ue(reader)
}

pub fn write_ue(value: u32) -> Vec<bool> {
    if value == 0 {
        return vec![true];
    }

    let code_num = value + 1;
    let num_bits = 32 - code_num.leading_zeros();
    let total_bits = 2 * num_bits - 1;
    
    let mut bits = Vec::with_capacity(total_bits as usize);
    
    for _ in 0..(num_bits - 1) {
        bits.push(false);
    }
    
    for i in (0..num_bits).rev() {
        bits.push((code_num >> i) & 1 != 0);
    }
    
    bits
}

pub fn write_se(value: i32) -> Vec<bool> {
    // Inverse of read_se mapping:
    // 0 => code_num = 0
    // 1 => code_num = 1
    // -1 => code_num = 2
    // 2 => code_num = 3
    // -2 => code_num = 4
    
    let code_num = if value == 0 {
        0
    } else if value > 0 {
        (value as u32) * 2 - 1
    } else {
        ((-value) as u32) * 2
    };
    
    write_ue(code_num)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_ue() {
        let data = vec![0b10100000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_ue(&mut reader).unwrap(), 0);

        let data = vec![0b01010000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_ue(&mut reader).unwrap(), 1);

        let data = vec![0b01100000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_ue(&mut reader).unwrap(), 2);

        let data = vec![0b00101100];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_ue(&mut reader).unwrap(), 4);

        let data = vec![0b00011110];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_ue(&mut reader).unwrap(), 14);
    }

    #[test]
    fn test_read_se() {
        // SE(0) = UE(0) = 1 => 0
        let data = vec![0b10100000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_se(&mut reader).unwrap(), 0);

        // SE(1) = UE(1) = 010 => 1
        let data = vec![0b01010000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_se(&mut reader).unwrap(), 1);

        // SE(-1) = UE(2) = 011 => -1
        let data = vec![0b01100000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_se(&mut reader).unwrap(), -1);

        // SE(2) = UE(3) = 00100 => 2
        let data = vec![0b00100000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_se(&mut reader).unwrap(), 2);

        // SE(-2) = UE(4) = 00101 => -2
        let data = vec![0b00101000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_se(&mut reader).unwrap(), -2);
    }

    #[test]
    fn test_write_ue() {
        assert_eq!(write_ue(0), vec![true]);
        assert_eq!(write_ue(1), vec![false, true, false]);
        assert_eq!(write_ue(2), vec![false, true, true]);
        assert_eq!(write_ue(3), vec![false, false, true, false, false]);
    }

    #[test]
    fn test_write_se() {
        assert_eq!(write_se(0), vec![true]);
        assert_eq!(write_se(1), vec![false, true, false]);
        assert_eq!(write_se(-1), vec![false, true, true]);
        assert_eq!(write_se(2), vec![false, false, true, false, false]);
        assert_eq!(write_se(-2), vec![false, false, true, false, true]);
    }

    #[test]
    fn test_read_te() {
        let data = vec![0b10000000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_te(&mut reader, 0).unwrap(), 0);

        let data = vec![0b00000000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_te(&mut reader, 1).unwrap(), 1);

        let data = vec![0b10000000];
        let mut reader = BitReader::new(&data);
        assert_eq!(read_te(&mut reader, 1).unwrap(), 0);
    }
}