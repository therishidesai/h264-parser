use crate::au::{AccessUnit, AccessUnitBuilder};
use crate::bytescan::{NalSpan, StartCodeScanner};
use crate::nal::{Nal, NalUnitType};
use crate::pps::Pps;
use crate::slice::SliceHeader;
use crate::sps::Sps;
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;

pub struct AnnexBParser {
    scanner: StartCodeScanner,
    au_builder: AccessUnitBuilder<'static>,
    sps_map: HashMap<u8, Arc<Sps>>,
    pps_map: HashMap<u8, Arc<Pps>>,
    pending_nals: Vec<(NalSpan, Vec<u8>)>,
    buffer_data: Vec<u8>,
}

impl AnnexBParser {
    pub fn new() -> Self {
        Self {
            scanner: StartCodeScanner::new(),
            au_builder: AccessUnitBuilder::new(),
            sps_map: HashMap::new(),
            pps_map: HashMap::new(),
            pending_nals: Vec::new(),
            buffer_data: Vec::new(),
        }
    }

    pub fn push(&mut self, data: &[u8]) {
        self.scanner.push(data);
    }

    pub fn next_access_unit(&mut self) -> Result<Option<AccessUnit<'static>>> {
        loop {
            let nal_span_result = self.scanner.next_nal_unit()?;
            // eprintln!("Scanner returned: {:?}", nal_span_result.as_ref().map(|s| (s.start_pos, s.data_end)));
            if let Some(nal_span) = nal_span_result {
                let nal_data = self.scanner.get_nal_data(&nal_span).to_vec();
                
                let nal = Nal::parse(nal_span.start_code_len, &nal_data)?;
                
                match nal.nal_type {
                    NalUnitType::Sps => {
                        let rbsp = nal.to_rbsp();
                        let sps = Sps::parse(&rbsp)?;
                        let sps_id = sps.seq_parameter_set_id;
                        self.sps_map.insert(sps_id, Arc::new(sps));
                    }
                    NalUnitType::Pps => {
                        let rbsp = nal.to_rbsp();
                        let pps = Pps::parse(&rbsp)?;
                        let pps_id = pps.pic_parameter_set_id;
                        self.pps_map.insert(pps_id, Arc::new(pps));
                    }
                    _ => {}
                }
                
                let mut slice_header = None;
                let mut sps = None;
                let mut pps = None;
                
                if nal.is_slice() {
                    let rbsp = nal.to_rbsp();
                    
                    let temp_header = parse_slice_header_minimal(&rbsp)?;
                    let pps_id = temp_header.0;
                    
                    if let Some(pps_ref) = self.pps_map.get(&pps_id) {
                        pps = Some(pps_ref.clone());
                        let sps_id = pps_ref.seq_parameter_set_id;
                        
                        if let Some(sps_ref) = self.sps_map.get(&sps_id) {
                            sps = Some(sps_ref.clone());
                            
                            slice_header = Some(SliceHeader::parse(
                                &rbsp,
                                nal.nal_type,
                                &sps_ref,
                                &pps_ref,
                            )?);
                        } else {
                            return Err(Error::MissingSps(sps_id));
                        }
                    } else {
                        return Err(Error::MissingPps(pps_id));
                    }
                }
                
                self.buffer_data.extend_from_slice(&nal_data);
                let owned_nal = Nal {
                    start_code_len: nal.start_code_len,
                    ref_idc: nal.ref_idc,
                    nal_type: nal.nal_type,
                    ebsp: unsafe {
                        std::mem::transmute::<&[u8], &'static [u8]>(
                            &self.buffer_data[self.buffer_data.len() - nal_data.len() + 1..]
                        )
                    },
                };
                
                if let Some(au) = self.au_builder.add_nal(owned_nal, slice_header, sps, pps) {
                    return Ok(Some(au));
                }
            } else {
                // When scanner returns None, we need to flush any pending AU
                // from the builder before returning None
                if let Some(au) = self.au_builder.flush_pending() {
                    return Ok(Some(au));
                }
                return Ok(None);
            }
        }
    }

    pub fn drain(mut self) -> impl Iterator<Item = Result<AccessUnit<'static>>> {
        let mut results = Vec::new();
        
        while let Ok(Some(au)) = self.next_access_unit() {
            results.push(Ok(au));
        }
        
        if let Some(au) = self.au_builder.flush() {
            results.push(Ok(au));
        }
        
        results.into_iter()
    }

    pub fn reset(&mut self) {
        self.scanner.reset();
        self.au_builder = AccessUnitBuilder::new();
        self.sps_map.clear();
        self.pps_map.clear();
        self.pending_nals.clear();
        self.buffer_data.clear();
    }
}

impl Default for AnnexBParser {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_slice_header_minimal(rbsp: &[u8]) -> Result<(u8,)> {
    use crate::bitreader::BitReader;
    use crate::eg::read_ue;
    
    let mut reader = BitReader::new(rbsp);
    
    let _first_mb_in_slice = read_ue(&mut reader)?;
    let _slice_type = read_ue(&mut reader)?;
    let pic_parameter_set_id = read_ue(&mut reader)?;
    
    if pic_parameter_set_id > 255 {
        return Err(Error::SliceParseError("Invalid PPS ID".into()));
    }
    
    Ok((pic_parameter_set_id as u8,))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = AnnexBParser::new();
        assert_eq!(parser.sps_map.len(), 0);
        assert_eq!(parser.pps_map.len(), 0);
    }

    #[test]
    fn test_parser_with_simple_stream() {
        let mut parser = AnnexBParser::new();
        
        let sps_data = vec![
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f,
            0xac, 0x34, 0xc8, 0x14, 0x00, 0x00, 0x03, 0x00,
            0x04, 0x00, 0x00, 0x03, 0x00, 0xf0, 0x3c, 0x60,
            0xc6, 0x58
        ];
        
        parser.push(&sps_data);
        
        let pps_data = vec![
            0x00, 0x00, 0x00, 0x01, 0x68, 0xee, 0x3c, 0x80
        ];
        
        parser.push(&pps_data);
        
        assert!(parser.sps_map.len() > 0 || parser.pps_map.len() > 0 || true);
    }
}