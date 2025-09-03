use crate::bitreader::BitReader;
use crate::eg::{read_se, read_ue};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct Pps {
    pub pic_parameter_set_id: u8,
    pub seq_parameter_set_id: u8,
    pub entropy_coding_mode_flag: bool,
    pub bottom_field_pic_order_in_frame_present_flag: bool,
    
    pub num_slice_groups_minus1: u32,
    pub slice_group_map_type: u32,
    
    pub num_ref_idx_l0_default_active_minus1: u8,
    pub num_ref_idx_l1_default_active_minus1: u8,
    pub weighted_pred_flag: bool,
    pub weighted_bipred_idc: u8,
    pub pic_init_qp_minus26: i8,
    pub pic_init_qs_minus26: i8,
    pub chroma_qp_index_offset: i8,
    pub deblocking_filter_control_present_flag: bool,
    pub constrained_intra_pred_flag: bool,
    pub redundant_pic_cnt_present_flag: bool,
    
    pub transform_8x8_mode_flag: bool,
    pub pic_scaling_matrix_present_flag: bool,
    pub second_chroma_qp_index_offset: i8,
}

impl Pps {
    pub fn parse(rbsp: &[u8]) -> Result<Self> {
        let mut reader = BitReader::new(rbsp);
        
        let pic_parameter_set_id = read_ue(&mut reader)?;
        if pic_parameter_set_id > 255 {
            return Err(Error::MalformedPps("Invalid PPS ID".into()));
        }
        
        let seq_parameter_set_id = read_ue(&mut reader)?;
        if seq_parameter_set_id > 31 {
            return Err(Error::MalformedPps("Invalid SPS ID reference".into()));
        }
        
        let entropy_coding_mode_flag = reader.read_flag()?;
        let bottom_field_pic_order_in_frame_present_flag = reader.read_flag()?;
        
        let num_slice_groups_minus1 = read_ue(&mut reader)?;
        let mut slice_group_map_type = 0;
        
        if num_slice_groups_minus1 > 0 {
            slice_group_map_type = read_ue(&mut reader)?;
            
            match slice_group_map_type {
                0 => {
                    for _ in 0..=num_slice_groups_minus1 {
                        let _run_length_minus1 = read_ue(&mut reader)?;
                    }
                }
                2 => {
                    for _ in 0..num_slice_groups_minus1 {
                        let _top_left = read_ue(&mut reader)?;
                        let _bottom_right = read_ue(&mut reader)?;
                    }
                }
                3 | 4 | 5 => {
                    let _slice_group_change_direction_flag = reader.read_flag()?;
                    let _slice_group_change_rate_minus1 = read_ue(&mut reader)?;
                }
                6 => {
                    let pic_size_in_map_units_minus1 = read_ue(&mut reader)?;
                    let num_bits = (num_slice_groups_minus1 + 1).ilog2() as u32;
                    for _ in 0..=pic_size_in_map_units_minus1 {
                        reader.read_bits(num_bits)?;
                    }
                }
                _ => {}
            }
        }
        
        let num_ref_idx_l0_default_active_minus1 = read_ue(&mut reader)?;
        if num_ref_idx_l0_default_active_minus1 > 31 {
            return Err(Error::MalformedPps("Invalid num_ref_idx_l0".into()));
        }
        
        let num_ref_idx_l1_default_active_minus1 = read_ue(&mut reader)?;
        if num_ref_idx_l1_default_active_minus1 > 31 {
            return Err(Error::MalformedPps("Invalid num_ref_idx_l1".into()));
        }
        
        let weighted_pred_flag = reader.read_flag()?;
        let weighted_bipred_idc = reader.read_bits(2)? as u8;
        
        let pic_init_qp_minus26 = read_se(&mut reader)?;
        if pic_init_qp_minus26 < -26 || pic_init_qp_minus26 > 25 {
            return Err(Error::MalformedPps("Invalid pic_init_qp".into()));
        }
        
        let pic_init_qs_minus26 = read_se(&mut reader)?;
        if pic_init_qs_minus26 < -26 || pic_init_qs_minus26 > 25 {
            return Err(Error::MalformedPps("Invalid pic_init_qs".into()));
        }
        
        let chroma_qp_index_offset = read_se(&mut reader)?;
        if chroma_qp_index_offset < -12 || chroma_qp_index_offset > 12 {
            return Err(Error::MalformedPps("Invalid chroma_qp_index_offset".into()));
        }
        
        let deblocking_filter_control_present_flag = reader.read_flag()?;
        let constrained_intra_pred_flag = reader.read_flag()?;
        let redundant_pic_cnt_present_flag = reader.read_flag()?;
        
        let mut transform_8x8_mode_flag = false;
        let mut pic_scaling_matrix_present_flag = false;
        let mut second_chroma_qp_index_offset = chroma_qp_index_offset;
        
        if reader.more_rbsp_data() {
            transform_8x8_mode_flag = reader.read_flag()?;
            pic_scaling_matrix_present_flag = reader.read_flag()?;
            
            if pic_scaling_matrix_present_flag {
                let num_lists = 6 + if transform_8x8_mode_flag { 2 } else { 0 };
                for i in 0..num_lists {
                    let pic_scaling_list_present_flag = reader.read_flag()?;
                    if pic_scaling_list_present_flag {
                        let size = if i < 6 { 16 } else { 64 };
                        skip_scaling_list(&mut reader, size)?;
                    }
                }
            }
            
            second_chroma_qp_index_offset = read_se(&mut reader)?;
            if second_chroma_qp_index_offset < -12 || second_chroma_qp_index_offset > 12 {
                return Err(Error::MalformedPps("Invalid second_chroma_qp_index_offset".into()));
            }
        }
        
        Ok(Pps {
            pic_parameter_set_id: pic_parameter_set_id as u8,
            seq_parameter_set_id: seq_parameter_set_id as u8,
            entropy_coding_mode_flag,
            bottom_field_pic_order_in_frame_present_flag,
            num_slice_groups_minus1,
            slice_group_map_type,
            num_ref_idx_l0_default_active_minus1: num_ref_idx_l0_default_active_minus1 as u8,
            num_ref_idx_l1_default_active_minus1: num_ref_idx_l1_default_active_minus1 as u8,
            weighted_pred_flag,
            weighted_bipred_idc,
            pic_init_qp_minus26: pic_init_qp_minus26 as i8,
            pic_init_qs_minus26: pic_init_qs_minus26 as i8,
            chroma_qp_index_offset: chroma_qp_index_offset as i8,
            deblocking_filter_control_present_flag,
            constrained_intra_pred_flag,
            redundant_pic_cnt_present_flag,
            transform_8x8_mode_flag,
            pic_scaling_matrix_present_flag,
            second_chroma_qp_index_offset: second_chroma_qp_index_offset as i8,
        })
    }
}

fn skip_scaling_list(reader: &mut BitReader, size: usize) -> Result<()> {
    let mut last_scale = 8;
    let mut next_scale = 8;
    
    for _ in 0..size {
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
    fn test_basic_pps_parse() {
        let ebsp = vec![0xee, 0x3c, 0x80];
        let rbsp = ebsp_to_rbsp(&ebsp);
        let pps = Pps::parse(&rbsp).unwrap();
        
        assert_eq!(pps.pic_parameter_set_id, 0);
        assert_eq!(pps.seq_parameter_set_id, 0);
    }
}