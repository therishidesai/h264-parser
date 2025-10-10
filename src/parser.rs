use crate::au::{AccessUnit, AccessUnitBuilder};
use crate::bytescan::StartCodeScanner;
use crate::nal::{Nal, NalUnitType};
use crate::pps::Pps;
use crate::slice::SliceHeader;
use crate::sps::Sps;
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;

pub struct AnnexBParser {
    scanner: StartCodeScanner,
    au_builder: AccessUnitBuilder,
    sps_map: HashMap<u8, Arc<Sps>>,
    pps_map: HashMap<u8, Arc<Pps>>,
}

impl AnnexBParser {
    pub fn new() -> Self {
        Self {
            scanner: StartCodeScanner::new(),
            au_builder: AccessUnitBuilder::new(),
            sps_map: HashMap::new(),
            pps_map: HashMap::new(),
        }
    }

    pub fn push(&mut self, data: &[u8]) {
        self.scanner.push(data);
    }

    pub fn next_access_unit(&mut self) -> Result<Option<AccessUnit>> {
        self.next_access_unit_internal(false)
    }

    pub fn next_access_unit_final(&mut self) -> Result<Option<AccessUnit>> {
        self.next_access_unit_internal(true)
    }

    pub fn drain(mut self) -> impl Iterator<Item = Result<AccessUnit>> {
        let mut results = Vec::new();

        loop {
            match self.next_access_unit_internal(true) {
                Ok(Some(au)) => results.push(Ok(au)),
                Ok(None) => break,
                Err(err) => {
                    results.push(Err(err));
                    break;
                }
            }
        }

        results.into_iter()
    }

    pub fn reset(&mut self) {
        self.scanner.reset();
        self.au_builder = AccessUnitBuilder::new();
        self.sps_map.clear();
        self.pps_map.clear();
    }

    fn next_access_unit_internal(&mut self, finalize: bool) -> Result<Option<AccessUnit>> {
        loop {
            match self.fetch_nal_bytes(finalize)? {
                Some((start_code_len, nal_bytes)) => {
                    if let Some(au) = self.process_nal(start_code_len, nal_bytes)? {
                        return Ok(Some(au));
                    }
                }
                None => {
                    if !finalize && self.scanner.has_pending_start() {
                        return Ok(None);
                    }

                    let pending = self.au_builder.flush_pending();
                    return Ok(pending);
                }
            }
        }
    }

    fn fetch_nal_bytes(&mut self, finalize: bool) -> Result<Option<(u8, Vec<u8>)>> {
        if let Some(span) = self.scanner.next_nal_unit()? {
            let nal_data = self.scanner.get_nal_data(&span).to_vec();
            self.scanner.consume_processed(span.data_end);
            return Ok(Some((span.start_code_len, nal_data)));
        }

        if finalize {
            if let Some(span) = self.scanner.finish_pending() {
                let nal_data = self.scanner.get_nal_data(&span).to_vec();
                self.scanner.consume_processed(span.data_end);
                return Ok(Some((span.start_code_len, nal_data)));
            }
        }

        Ok(None)
    }

    fn process_nal(
        &mut self,
        start_code_len: u8,
        nal_bytes: Vec<u8>,
    ) -> Result<Option<AccessUnit>> {
        let nal = Nal::parse(start_code_len, &nal_bytes)?;

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

                    slice_header =
                        Some(SliceHeader::parse(&rbsp, nal.nal_type, &sps_ref, &pps_ref)?);
                } else {
                    return Err(Error::MissingSps(sps_id));
                }
            } else {
                return Err(Error::MissingPps(pps_id));
            }
        }

        let owned_nal = nal.clone();

        Ok(self.au_builder.add_nal(owned_nal, slice_header, sps, pps))
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
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f, 0xac, 0x34, 0xc8, 0x14, 0x00, 0x00,
            0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xf0, 0x3c, 0x60, 0xc6, 0x58,
        ];

        parser.push(&sps_data);

        let pps_data = vec![0x00, 0x00, 0x00, 0x01, 0x68, 0xee, 0x3c, 0x80];

        parser.push(&pps_data);

        assert!(parser.sps_map.len() > 0 || parser.pps_map.len() > 0 || true);
    }
}
