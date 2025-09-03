use crate::bitreader::BitReader;
use crate::eg::{read_se, read_ue};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct Sps {
    pub profile_idc: u8,
    pub constraint_set0_flag: bool,
    pub constraint_set1_flag: bool,
    pub constraint_set2_flag: bool,
    pub constraint_set3_flag: bool,
    pub constraint_set4_flag: bool,
    pub constraint_set5_flag: bool,
    pub level_idc: u8,
    pub seq_parameter_set_id: u8,
    
    pub chroma_format_idc: u8,
    pub separate_colour_plane_flag: bool,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub qpprime_y_zero_transform_bypass_flag: bool,
    pub seq_scaling_matrix_present_flag: bool,
    
    pub log2_max_frame_num_minus4: u8,
    pub pic_order_cnt_type: u8,
    pub log2_max_pic_order_cnt_lsb_minus4: u8,
    pub delta_pic_order_always_zero_flag: bool,
    pub offset_for_non_ref_pic: i32,
    pub offset_for_top_to_bottom_field: i32,
    pub num_ref_frames_in_pic_order_cnt_cycle: u8,
    
    pub max_num_ref_frames: u32,
    pub gaps_in_frame_num_value_allowed_flag: bool,
    pub pic_width_in_mbs_minus1: u32,
    pub pic_height_in_map_units_minus1: u32,
    pub frame_mbs_only_flag: bool,
    pub mb_adaptive_frame_field_flag: bool,
    pub direct_8x8_inference_flag: bool,
    
    pub frame_cropping_flag: bool,
    pub frame_crop_left_offset: u32,
    pub frame_crop_right_offset: u32,
    pub frame_crop_top_offset: u32,
    pub frame_crop_bottom_offset: u32,
    
    pub vui_parameters_present_flag: bool,
    
    pub width: u32,
    pub height: u32,
}

impl Sps {
    pub fn parse(rbsp: &[u8]) -> Result<Self> {
        let mut reader = BitReader::new(rbsp);
        
        let profile_idc = reader.read_u8()?;
        let constraint_set0_flag = reader.read_flag()?;
        let constraint_set1_flag = reader.read_flag()?;
        let constraint_set2_flag = reader.read_flag()?;
        let constraint_set3_flag = reader.read_flag()?;
        let constraint_set4_flag = reader.read_flag()?;
        let constraint_set5_flag = reader.read_flag()?;
        let _reserved_zero_2bits = reader.read_bits(2)?;
        let level_idc = reader.read_u8()?;
        
        let seq_parameter_set_id = read_ue(&mut reader)?;
        if seq_parameter_set_id > 31 {
            return Err(Error::MalformedSps("Invalid SPS ID".into()));
        }
        
        let mut chroma_format_idc = 1;
        let mut separate_colour_plane_flag = false;
        let mut bit_depth_luma_minus8 = 0;
        let mut bit_depth_chroma_minus8 = 0;
        let mut qpprime_y_zero_transform_bypass_flag = false;
        let mut seq_scaling_matrix_present_flag = false;
        
        if profile_idc == 100 || profile_idc == 110 || profile_idc == 122 || 
           profile_idc == 244 || profile_idc == 44 || profile_idc == 83 || 
           profile_idc == 86 || profile_idc == 118 || profile_idc == 128 ||
           profile_idc == 138 || profile_idc == 139 || profile_idc == 134 ||
           profile_idc == 135 {
            chroma_format_idc = read_ue(&mut reader)? as u8;
            if chroma_format_idc > 3 {
                return Err(Error::MalformedSps("Invalid chroma format".into()));
            }
            
            if chroma_format_idc == 3 {
                separate_colour_plane_flag = reader.read_flag()?;
            }
            
            bit_depth_luma_minus8 = read_ue(&mut reader)? as u8;
            bit_depth_chroma_minus8 = read_ue(&mut reader)? as u8;
            qpprime_y_zero_transform_bypass_flag = reader.read_flag()?;
            seq_scaling_matrix_present_flag = reader.read_flag()?;
            
            if seq_scaling_matrix_present_flag {
                let num_lists = if chroma_format_idc != 3 { 8 } else { 12 };
                for _ in 0..num_lists {
                    let seq_scaling_list_present_flag = reader.read_flag()?;
                    if seq_scaling_list_present_flag {
                        skip_scaling_list(&mut reader)?;
                    }
                }
            }
        }
        
        let log2_max_frame_num_minus4 = read_ue(&mut reader)? as u8;
        if log2_max_frame_num_minus4 > 12 {
            return Err(Error::MalformedSps("Invalid log2_max_frame_num".into()));
        }
        
        let pic_order_cnt_type = read_ue(&mut reader)? as u8;
        
        let mut log2_max_pic_order_cnt_lsb_minus4 = 0;
        let mut delta_pic_order_always_zero_flag = false;
        let mut offset_for_non_ref_pic = 0;
        let mut offset_for_top_to_bottom_field = 0;
        let mut num_ref_frames_in_pic_order_cnt_cycle = 0;
        
        match pic_order_cnt_type {
            0 => {
                log2_max_pic_order_cnt_lsb_minus4 = read_ue(&mut reader)? as u8;
                if log2_max_pic_order_cnt_lsb_minus4 > 12 {
                    return Err(Error::MalformedSps("Invalid log2_max_pic_order_cnt_lsb".into()));
                }
            }
            1 => {
                delta_pic_order_always_zero_flag = reader.read_flag()?;
                offset_for_non_ref_pic = read_se(&mut reader)?;
                offset_for_top_to_bottom_field = read_se(&mut reader)?;
                num_ref_frames_in_pic_order_cnt_cycle = read_ue(&mut reader)? as u8;
                
                for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
                    let _offset_for_ref_frame = read_se(&mut reader)?;
                }
            }
            2 => {}
            _ => return Err(Error::MalformedSps("Invalid pic_order_cnt_type".into())),
        }
        
        let max_num_ref_frames = read_ue(&mut reader)?;
        let gaps_in_frame_num_value_allowed_flag = reader.read_flag()?;
        
        let pic_width_in_mbs_minus1 = read_ue(&mut reader)?;
        let pic_height_in_map_units_minus1 = read_ue(&mut reader)?;
        
        let frame_mbs_only_flag = reader.read_flag()?;
        let mut mb_adaptive_frame_field_flag = false;
        if !frame_mbs_only_flag {
            mb_adaptive_frame_field_flag = reader.read_flag()?;
        }
        
        let direct_8x8_inference_flag = reader.read_flag()?;
        
        let frame_cropping_flag = reader.read_flag()?;
        let mut frame_crop_left_offset = 0;
        let mut frame_crop_right_offset = 0;
        let mut frame_crop_top_offset = 0;
        let mut frame_crop_bottom_offset = 0;
        
        if frame_cropping_flag {
            frame_crop_left_offset = read_ue(&mut reader)?;
            frame_crop_right_offset = read_ue(&mut reader)?;
            frame_crop_top_offset = read_ue(&mut reader)?;
            frame_crop_bottom_offset = read_ue(&mut reader)?;
        }
        
        let vui_parameters_present_flag = reader.read_flag()?;
        
        let width = (pic_width_in_mbs_minus1 + 1) * 16;
        let height = (pic_height_in_map_units_minus1 + 1) * 16 * if frame_mbs_only_flag { 1 } else { 2 };
        
        let (sub_width_c, sub_height_c) = match chroma_format_idc {
            0 => (0, 0),
            1 => (2, 2),
            2 => (2, 1),
            3 => (1, 1),
            _ => (0, 0),
        };
        
        let width = if frame_cropping_flag && sub_width_c > 0 {
            width - sub_width_c * (frame_crop_left_offset + frame_crop_right_offset)
        } else {
            width
        };
        
        let height = if frame_cropping_flag && sub_height_c > 0 {
            let mult = if frame_mbs_only_flag { 1 } else { 2 };
            height - sub_height_c * mult * (frame_crop_top_offset + frame_crop_bottom_offset)
        } else {
            height
        };
        
        Ok(Sps {
            profile_idc,
            constraint_set0_flag,
            constraint_set1_flag,
            constraint_set2_flag,
            constraint_set3_flag,
            constraint_set4_flag,
            constraint_set5_flag,
            level_idc,
            seq_parameter_set_id: seq_parameter_set_id as u8,
            chroma_format_idc,
            separate_colour_plane_flag,
            bit_depth_luma_minus8,
            bit_depth_chroma_minus8,
            qpprime_y_zero_transform_bypass_flag,
            seq_scaling_matrix_present_flag,
            log2_max_frame_num_minus4,
            pic_order_cnt_type,
            log2_max_pic_order_cnt_lsb_minus4,
            delta_pic_order_always_zero_flag,
            offset_for_non_ref_pic,
            offset_for_top_to_bottom_field,
            num_ref_frames_in_pic_order_cnt_cycle,
            max_num_ref_frames,
            gaps_in_frame_num_value_allowed_flag,
            pic_width_in_mbs_minus1,
            pic_height_in_map_units_minus1,
            frame_mbs_only_flag,
            mb_adaptive_frame_field_flag,
            direct_8x8_inference_flag,
            frame_cropping_flag,
            frame_crop_left_offset,
            frame_crop_right_offset,
            frame_crop_top_offset,
            frame_crop_bottom_offset,
            vui_parameters_present_flag,
            width,
            height,
        })
    }
}

fn skip_scaling_list(reader: &mut BitReader) -> Result<()> {
    let mut last_scale = 8;
    let mut next_scale = 8;
    
    for _ in 0..16 {
        if next_scale != 0 {
            let delta_scale = read_se(reader)?;
            next_scale = (last_scale + delta_scale + 256) % 256;
        }
        last_scale = if next_scale == 0 { last_scale } else { next_scale };
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nal::ebsp_to_rbsp;

    #[test]
    fn test_basic_sps_parse() {
        let ebsp = vec![
            0x42, 0x00, 0x1f, 0xac, 0x34, 0xc8, 0x14, 0x00,
            0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00,
            0xf0, 0x3c, 0x60, 0xc6, 0x58
        ];
        
        let rbsp = ebsp_to_rbsp(&ebsp);
        let sps = Sps::parse(&rbsp).unwrap();
        
        assert_eq!(sps.profile_idc, 66);
        assert_eq!(sps.level_idc, 31);
        assert!(sps.width > 0);
        assert!(sps.height > 0);
    }
}