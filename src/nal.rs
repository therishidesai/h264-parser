use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NalUnitType {
    Unspecified,
    NonIdrSlice,
    DataPartitionA,
    DataPartitionB,
    DataPartitionC,
    IdrSlice,
    Sei,
    Sps,
    Pps,
    Aud,
    EndOfSeq,
    EndOfStream,
    Filler,
    SpsExt,
    Prefix,
    SubsetSps,
    DepthParameterSet,
    Reserved(u8),
    UnspecifiedExt(u8),
}

impl NalUnitType {
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Unspecified => 0,
            Self::NonIdrSlice => 1,
            Self::DataPartitionA => 2,
            Self::DataPartitionB => 3,
            Self::DataPartitionC => 4,
            Self::IdrSlice => 5,
            Self::Sei => 6,
            Self::Sps => 7,
            Self::Pps => 8,
            Self::Aud => 9,
            Self::EndOfSeq => 10,
            Self::EndOfStream => 11,
            Self::Filler => 12,
            Self::SpsExt => 13,
            Self::Prefix => 14,
            Self::SubsetSps => 15,
            Self::DepthParameterSet => 16,
            Self::Reserved(v) => *v,
            Self::UnspecifiedExt(v) => *v,
        }
    }
}

impl From<u8> for NalUnitType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Unspecified,
            1 => Self::NonIdrSlice,
            2 => Self::DataPartitionA,
            3 => Self::DataPartitionB,
            4 => Self::DataPartitionC,
            5 => Self::IdrSlice,
            6 => Self::Sei,
            7 => Self::Sps,
            8 => Self::Pps,
            9 => Self::Aud,
            10 => Self::EndOfSeq,
            11 => Self::EndOfStream,
            12 => Self::Filler,
            13 => Self::SpsExt,
            14 => Self::Prefix,
            15 => Self::SubsetSps,
            16 => Self::DepthParameterSet,
            17..=23 => Self::Reserved(value),
            24..=31 => Self::UnspecifiedExt(value),
            _ => Self::Unspecified,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Nal {
    pub start_code_len: u8,
    pub ref_idc: u8,
    pub nal_type: NalUnitType,
    pub ebsp: Vec<u8>,
}

impl Nal {
    pub fn parse(start_code_len: u8, data: &[u8]) -> Result<Self> {
        if data.is_empty() {
            return Err(Error::InvalidNalHeader);
        }

        let header = data[0];
        
        let forbidden_zero_bit = (header >> 7) & 1;
        if forbidden_zero_bit != 0 {
            return Err(Error::InvalidNalHeader);
        }

        let ref_idc = (header >> 5) & 0b11;
        let nal_unit_type = header & 0b11111;
        let nal_type = NalUnitType::from(nal_unit_type);

        let ebsp = if data.len() > 1 {
            data[1..].to_vec()
        } else {
            Vec::new()
        };

        Ok(Nal {
            start_code_len,
            ref_idc,
            nal_type,
            ebsp,
        })
    }

    pub fn to_rbsp(&self) -> Vec<u8> {
        ebsp_to_rbsp(&self.ebsp)
    }

    pub fn is_slice(&self) -> bool {
        matches!(
            self.nal_type,
            NalUnitType::NonIdrSlice
                | NalUnitType::IdrSlice
                | NalUnitType::DataPartitionA
                | NalUnitType::DataPartitionB
                | NalUnitType::DataPartitionC
        )
    }

    pub fn is_vcl(&self) -> bool {
        match self.nal_type {
            NalUnitType::NonIdrSlice
            | NalUnitType::DataPartitionA
            | NalUnitType::DataPartitionB
            | NalUnitType::DataPartitionC
            | NalUnitType::IdrSlice => true,
            _ => false,
        }
    }
}

pub fn ebsp_to_rbsp(ebsp: &[u8]) -> Vec<u8> {
    let mut rbsp = Vec::with_capacity(ebsp.len());
    let mut i = 0;

    while i < ebsp.len() {
        if i + 2 < ebsp.len() && ebsp[i] == 0x00 && ebsp[i + 1] == 0x00 && ebsp[i + 2] == 0x03 {
            rbsp.push(0x00);
            rbsp.push(0x00);
            i += 3;
        } else {
            rbsp.push(ebsp[i]);
            i += 1;
        }
    }

    rbsp
}

pub fn rbsp_to_ebsp(rbsp: &[u8]) -> Vec<u8> {
    let mut ebsp = Vec::with_capacity(rbsp.len() + rbsp.len() / 3);
    let mut zero_count = 0;

    for &byte in rbsp {
        if zero_count == 2 && byte <= 0x03 {
            ebsp.push(0x03);
            zero_count = 0;
        }

        ebsp.push(byte);

        if byte == 0x00 {
            zero_count += 1;
        } else {
            zero_count = 0;
        }
    }

    ebsp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nal_parse() {
        let data = vec![0x67, 0x42, 0x00, 0x1f];
        let nal = Nal::parse(4, &data).unwrap();
        
        assert_eq!(nal.ref_idc, 3);
        assert_eq!(nal.nal_type, NalUnitType::Sps);
        assert_eq!(nal.ebsp, &[0x42, 0x00, 0x1f]);
    }

    #[test]
    fn test_ebsp_to_rbsp() {
        let ebsp = vec![0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x01, 0x00, 0x00, 0x03, 0x02];
        let rbsp = ebsp_to_rbsp(&ebsp);
        assert_eq!(rbsp, vec![0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x02]);
    }

    #[test]
    fn test_rbsp_to_ebsp() {
        let rbsp = vec![0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x02];
        let ebsp = rbsp_to_ebsp(&rbsp);
        assert_eq!(ebsp, vec![0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x01, 0x00, 0x00, 0x03, 0x02]);
    }

    #[test]
    fn test_nal_type_conversion() {
        assert_eq!(NalUnitType::from(5), NalUnitType::IdrSlice);
        assert_eq!(NalUnitType::from(7), NalUnitType::Sps);
        assert_eq!(NalUnitType::from(8), NalUnitType::Pps);
        assert!(matches!(NalUnitType::from(20), NalUnitType::Reserved(20)));
    }
}