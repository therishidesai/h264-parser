use crate::nal::{Nal, NalUnitType};
use crate::pps::Pps;
use crate::sei::{SeiMessage, SeiPayload};
use crate::slice::{PictureId, SliceHeader};
use crate::sps::Sps;
use std::borrow::Cow;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessUnitKind {
    Idr,
    RecoveryPoint(u32),
    NonIdr,
}

#[derive(Debug, Clone)]
pub struct AccessUnit {
    pub nals: Vec<Nal>,
    pub is_keyframe: bool,
    pub kind: AccessUnitKind,
    pub sps: Option<Arc<Sps>>,
    pub pps: Option<Arc<Pps>>,
    pub picture_id: Option<PictureId>,
}

impl AccessUnit {
    pub fn new() -> Self {
        Self {
            nals: Vec::new(),
            is_keyframe: false,
            kind: AccessUnitKind::NonIdr,
            sps: None,
            pps: None,
            picture_id: None,
        }
    }

    pub fn is_keyframe(&self) -> bool {
        self.is_keyframe
    }

    pub fn nals(&self) -> impl Iterator<Item = &Nal> {
        self.nals.iter()
    }

    pub fn to_annexb_bytes(&self) -> Cow<'_, [u8]> {
        let mut bytes = Vec::new();
        
        for nal in &self.nals {
            let start_code = if nal.start_code_len == 4 {
                &[0x00, 0x00, 0x00, 0x01][..]
            } else {
                &[0x00, 0x00, 0x01][..]
            };
            
            bytes.extend_from_slice(start_code);
            
            let header = ((nal.ref_idc & 0b11) << 5) | (nal.nal_type.as_u8() & 0b11111);
            bytes.push(header);
            
            bytes.extend_from_slice(&nal.ebsp);
        }
        
        Cow::Owned(bytes)
    }

    pub fn add_nal(&mut self, nal: Nal) {
        if nal.nal_type == NalUnitType::IdrSlice {
            self.kind = AccessUnitKind::Idr;
            self.is_keyframe = true;
        }
        
        self.nals.push(nal);
    }

    pub fn set_sps(&mut self, sps: Arc<Sps>) {
        self.sps = Some(sps);
    }

    pub fn set_pps(&mut self, pps: Arc<Pps>) {
        self.pps = Some(pps);
    }

    pub fn check_recovery_point(&mut self) {
        for nal in &self.nals {
            if nal.nal_type == NalUnitType::Sei {
                let rbsp = nal.to_rbsp();
                if let Ok(messages) = SeiMessage::parse(&rbsp) {
                    for msg in messages {
                        if let SeiPayload::RecoveryPoint { recovery_frame_cnt, .. } = msg.payload {
                            if recovery_frame_cnt == 0 {
                                self.kind = AccessUnitKind::RecoveryPoint(0);
                                self.is_keyframe = true;
                            } else {
                                self.kind = AccessUnitKind::RecoveryPoint(recovery_frame_cnt);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn set_picture_id_from_slice(
        &mut self,
        slice_header: &SliceHeader,
        nal_type: NalUnitType,
        sps: &Sps,
    ) {
        self.picture_id = Some(PictureId::from_slice_header(slice_header, nal_type, sps));
    }
}

pub struct AccessUnitBuilder {
    current_au: Option<AccessUnit>,
    current_picture_id: Option<PictureId>,
}

impl AccessUnitBuilder {
    pub fn new() -> Self {
        Self {
            current_au: None,
            current_picture_id: None,
        }
    }

    pub fn is_au_boundary(
        &self,
        nal: &Nal,
        slice_header: Option<&SliceHeader>,
        sps: Option<&Sps>,
    ) -> bool {
        if nal.nal_type == NalUnitType::Aud {
            return true;
        }

        if !nal.is_vcl() {
            return false;
        }

        if self.current_picture_id.is_none() {
            return true;
        }

        if let (Some(header), Some(sps)) = (slice_header, sps) {
            let new_picture_id = PictureId::from_slice_header(header, nal.nal_type, sps);
            
            if let Some(ref current_id) = self.current_picture_id {
                return &new_picture_id != current_id;
            }
        }

        false
    }

    pub fn add_nal(
        &mut self,
        nal: Nal,
        slice_header: Option<SliceHeader>,
        sps: Option<Arc<Sps>>,
        pps: Option<Arc<Pps>>,
    ) -> Option<AccessUnit> {
        let is_boundary = if let (Some(ref header), Some(ref sps_ref)) = (&slice_header, &sps) {
            self.is_au_boundary(&nal, Some(header), Some(sps_ref))
        } else {
            self.is_au_boundary(&nal, None, None)
        };

        let mut completed_au = None;

        if is_boundary && self.current_au.is_some() {
            if let Some(mut au) = self.current_au.take() {
                au.check_recovery_point();
                completed_au = Some(au);
            }
            self.current_picture_id = None;
        }

        if self.current_au.is_none() {
            self.current_au = Some(AccessUnit::new());
        }

        if let Some(ref mut au) = self.current_au {
            if let Some(sps) = sps {
                au.set_sps(sps);
            }
            
            if let Some(pps) = pps {
                au.set_pps(pps);
            }

            if let (Some(header), Some(ref sps_ref)) = (slice_header, &au.sps) {
                let picture_id = PictureId::from_slice_header(&header, nal.nal_type, sps_ref);
                self.current_picture_id = Some(picture_id.clone());
                au.picture_id = Some(picture_id);
            }

            au.add_nal(nal);
        }

        completed_au
    }

    pub fn flush(mut self) -> Option<AccessUnit> {
        if let Some(mut au) = self.current_au.take() {
            au.check_recovery_point();
            Some(au)
        } else {
            None
        }
    }
    
    pub fn flush_pending(&mut self) -> Option<AccessUnit> {
        if let Some(mut au) = self.current_au.take() {
            au.check_recovery_point();
            self.current_picture_id = None;
            Some(au)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_unit_keyframe_detection() {
        let mut au = AccessUnit::new();
        assert!(!au.is_keyframe());
        
        let idr_nal = Nal {
            start_code_len: 4,
            ref_idc: 3,
            nal_type: NalUnitType::IdrSlice,
            ebsp: &[],
        };
        
        au.add_nal(idr_nal);
        assert!(au.is_keyframe());
        assert_eq!(au.kind, AccessUnitKind::Idr);
    }

    #[test]
    fn test_to_annexb_bytes() {
        let mut au = AccessUnit::new();
        
        let nal = Nal {
            start_code_len: 3,
            ref_idc: 2,
            nal_type: NalUnitType::Sps,
            ebsp: &[0x42, 0x00, 0x1f],
        };
        
        au.add_nal(nal);
        
        let bytes = au.to_annexb_bytes();
        assert_eq!(&bytes[0..3], &[0x00, 0x00, 0x01]);
        assert_eq!(bytes[3], 0x47);
        assert_eq!(&bytes[4..], &[0x42, 0x00, 0x1f]);
    }
}