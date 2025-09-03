use crate::bitreader::BitReader;
use crate::eg::{read_se, read_ue};
use crate::nal::NalUnitType;
use crate::pps::Pps;
use crate::sps::Sps;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceType {
    P = 0,
    B = 1,
    I = 2,
    Sp = 3,
    Si = 4,
}

impl SliceType {
    pub fn from_value(value: u32) -> Option<Self> {
        match value % 5 {
            0 => Some(SliceType::P),
            1 => Some(SliceType::B),
            2 => Some(SliceType::I),
            3 => Some(SliceType::Sp),
            4 => Some(SliceType::Si),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SliceHeader {
    pub first_mb_in_slice: u32,
    pub slice_type: SliceType,
    pub pic_parameter_set_id: u8,
    pub colour_plane_id: u8,
    pub frame_num: u32,
    pub field_pic_flag: bool,
    pub bottom_field_flag: bool,
    pub idr_pic_id: u32,
    pub pic_order_cnt_lsb: u32,
    pub delta_pic_order_cnt_bottom: i32,
    pub delta_pic_order_cnt: [i32; 2],
    pub redundant_pic_cnt: u32,
    pub direct_spatial_mv_pred_flag: bool,
    pub num_ref_idx_active_override_flag: bool,
    pub num_ref_idx_l0_active_minus1: u32,
    pub num_ref_idx_l1_active_minus1: u32,
}

impl SliceHeader {
    pub fn parse(
        rbsp: &[u8],
        nal_type: NalUnitType,
        sps: &Sps,
        pps: &Pps,
    ) -> Result<Self> {
        let mut reader = BitReader::new(rbsp);
        
        let first_mb_in_slice = read_ue(&mut reader)?;
        
        let slice_type_value = read_ue(&mut reader)?;
        let slice_type = SliceType::from_value(slice_type_value)
            .ok_or_else(|| Error::SliceParseError("Invalid slice type".into()))?;
        
        let pic_parameter_set_id = read_ue(&mut reader)?;
        if pic_parameter_set_id > 255 {
            return Err(Error::SliceParseError("Invalid PPS ID".into()));
        }
        
        let mut colour_plane_id = 0;
        if sps.separate_colour_plane_flag {
            colour_plane_id = reader.read_bits(2)? as u8;
        }
        
        let frame_num_bits = sps.log2_max_frame_num_minus4 + 4;
        let frame_num = reader.read_bits(frame_num_bits as u32)?;
        
        let mut field_pic_flag = false;
        let mut bottom_field_flag = false;
        
        if !sps.frame_mbs_only_flag {
            field_pic_flag = reader.read_flag()?;
            if field_pic_flag {
                bottom_field_flag = reader.read_flag()?;
            }
        }
        
        let mut idr_pic_id = 0;
        if nal_type == NalUnitType::IdrSlice {
            idr_pic_id = read_ue(&mut reader)?;
        }
        
        let mut pic_order_cnt_lsb = 0;
        let mut delta_pic_order_cnt_bottom = 0;
        let mut delta_pic_order_cnt = [0, 0];
        
        if sps.pic_order_cnt_type == 0 {
            let pic_order_cnt_lsb_bits = sps.log2_max_pic_order_cnt_lsb_minus4 + 4;
            pic_order_cnt_lsb = reader.read_bits(pic_order_cnt_lsb_bits as u32)?;
            
            if pps.bottom_field_pic_order_in_frame_present_flag && !field_pic_flag {
                delta_pic_order_cnt_bottom = read_se(&mut reader)?;
            }
        } else if sps.pic_order_cnt_type == 1 && !sps.delta_pic_order_always_zero_flag {
            delta_pic_order_cnt[0] = read_se(&mut reader)?;
            
            if pps.bottom_field_pic_order_in_frame_present_flag && !field_pic_flag {
                delta_pic_order_cnt[1] = read_se(&mut reader)?;
            }
        }
        
        let mut redundant_pic_cnt = 0;
        if pps.redundant_pic_cnt_present_flag {
            redundant_pic_cnt = read_ue(&mut reader)?;
        }
        
        let mut direct_spatial_mv_pred_flag = false;
        if slice_type == SliceType::B {
            direct_spatial_mv_pred_flag = reader.read_flag()?;
        }
        
        let mut num_ref_idx_active_override_flag = false;
        let mut num_ref_idx_l0_active_minus1 = pps.num_ref_idx_l0_default_active_minus1 as u32;
        let mut num_ref_idx_l1_active_minus1 = pps.num_ref_idx_l1_default_active_minus1 as u32;
        
        if slice_type == SliceType::P || slice_type == SliceType::Sp || slice_type == SliceType::B {
            num_ref_idx_active_override_flag = reader.read_flag()?;
            
            if num_ref_idx_active_override_flag {
                num_ref_idx_l0_active_minus1 = read_ue(&mut reader)?;
                
                if slice_type == SliceType::B {
                    num_ref_idx_l1_active_minus1 = read_ue(&mut reader)?;
                }
            }
        }
        
        Ok(SliceHeader {
            first_mb_in_slice,
            slice_type,
            pic_parameter_set_id: pic_parameter_set_id as u8,
            colour_plane_id,
            frame_num,
            field_pic_flag,
            bottom_field_flag,
            idr_pic_id,
            pic_order_cnt_lsb,
            delta_pic_order_cnt_bottom,
            delta_pic_order_cnt,
            redundant_pic_cnt,
            direct_spatial_mv_pred_flag,
            num_ref_idx_active_override_flag,
            num_ref_idx_l0_active_minus1,
            num_ref_idx_l1_active_minus1,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PictureId {
    pub frame_num: u32,
    pub pic_parameter_set_id: u8,
    pub idr_pic_id: Option<u32>,
    pub pic_order_cnt_lsb: Option<u32>,
    pub delta_pic_order_cnt: Option<[i32; 2]>,
    pub field_pic_flag: bool,
    pub bottom_field_flag: bool,
}

impl PictureId {
    pub fn from_slice_header(header: &SliceHeader, nal_type: NalUnitType, sps: &Sps) -> Self {
        let idr_pic_id = if nal_type == NalUnitType::IdrSlice {
            Some(header.idr_pic_id)
        } else {
            None
        };
        
        let pic_order_cnt_lsb = if sps.pic_order_cnt_type == 0 {
            Some(header.pic_order_cnt_lsb)
        } else {
            None
        };
        
        let delta_pic_order_cnt = if sps.pic_order_cnt_type == 1 {
            Some(header.delta_pic_order_cnt)
        } else {
            None
        };
        
        PictureId {
            frame_num: header.frame_num,
            pic_parameter_set_id: header.pic_parameter_set_id,
            idr_pic_id,
            pic_order_cnt_lsb,
            delta_pic_order_cnt,
            field_pic_flag: header.field_pic_flag,
            bottom_field_flag: header.bottom_field_flag,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slice_type_conversion() {
        assert_eq!(SliceType::from_value(0), Some(SliceType::P));
        assert_eq!(SliceType::from_value(1), Some(SliceType::B));
        assert_eq!(SliceType::from_value(2), Some(SliceType::I));
        assert_eq!(SliceType::from_value(5), Some(SliceType::P));
        assert_eq!(SliceType::from_value(7), Some(SliceType::I));
    }
}