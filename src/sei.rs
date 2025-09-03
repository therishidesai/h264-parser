use crate::Result;

#[derive(Debug, Clone)]
pub enum SeiPayload {
    BufferingPeriod,
    PicTiming,
    PanScanRect,
    FillerPayload,
    UserDataRegistered,
    UserDataUnregistered(Vec<u8>),
    RecoveryPoint {
        recovery_frame_cnt: u32,
        exact_match_flag: bool,
        broken_link_flag: bool,
        changing_slice_group_idc: u8,
    },
    DecRefPicMarkingRepetition,
    SparePic,
    SceneInfo,
    SubSeqInfo,
    SubSeqLayerCharacteristics,
    SubSeqCharacteristics,
    FullFrameFreeze,
    FullFrameFreezeRelease,
    FullFrameSnapshot,
    ProgressiveRefinementSegmentStart,
    ProgressiveRefinementSegmentEnd,
    MotionConstrainedSliceGroupSet,
    FilmGrainCharacteristics,
    DeblockingFilterDisplayPreference,
    StereoVideoInfo,
    PostFilterHint,
    ToneMappingInfo,
    ScalabilityInfo,
    SubPicScalableLayer,
    NonRequiredLayerRep,
    PriorityLayerInfo,
    LayersNotPresent,
    LayerDependencyChange,
    ScalableNesting,
    BaseLayerTemporalHrd,
    QualityLayerIntegrityCheck,
    RedundantPicProperty,
    Tl0DepRepIndex,
    TlSwitchingPoint,
    ParallelDecodingInfo,
    MvcScalableNesting,
    ViewScalabilityInfo,
    MultiviewSceneInfo,
    MultiviewAcquisitionInfo,
    NonRequiredViewComponent,
    ViewDependencyChange,
    OperationPointsNotPresent,
    BaseViewTemporalHrd,
    FramePackingArrangement,
    MultiviewViewPosition,
    DisplayOrientation,
    MvcdScalableNesting,
    MvcdViewScalabilityInfo,
    DepthRepresentationInfo,
    ThreeDimensionalReferenceDisplaysInfo,
    DepthTiming,
    DepthSamplingInfo,
    ConstrainedDepthParameterSetIdentifier,
    Unknown(u32, Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct SeiMessage {
    pub payload_type: u32,
    pub payload_size: u32,
    pub payload: SeiPayload,
}

impl SeiMessage {
    pub fn parse(rbsp: &[u8]) -> Result<Vec<SeiMessage>> {
        let mut messages = Vec::new();
        let mut pos = 0;
        
        while pos < rbsp.len() && rbsp[pos] != 0x80 {
            let mut payload_type = 0u32;
            while pos < rbsp.len() && rbsp[pos] == 0xFF {
                payload_type += 255;
                pos += 1;
            }
            if pos < rbsp.len() {
                payload_type += rbsp[pos] as u32;
                pos += 1;
            }
            
            let mut payload_size = 0u32;
            while pos < rbsp.len() && rbsp[pos] == 0xFF {
                payload_size += 255;
                pos += 1;
            }
            if pos < rbsp.len() {
                payload_size += rbsp[pos] as u32;
                pos += 1;
            }
            
            let payload_end = (pos + payload_size as usize).min(rbsp.len());
            let payload_data = &rbsp[pos..payload_end];
            
            let payload = match payload_type {
                6 => parse_recovery_point(payload_data)?,
                5 => {
                    if payload_data.len() >= 16 {
                        SeiPayload::UserDataUnregistered(payload_data.to_vec())
                    } else {
                        SeiPayload::Unknown(payload_type, payload_data.to_vec())
                    }
                }
                _ => SeiPayload::Unknown(payload_type, payload_data.to_vec()),
            };
            
            messages.push(SeiMessage {
                payload_type,
                payload_size,
                payload,
            });
            
            pos = payload_end;
        }
        
        Ok(messages)
    }
}

fn parse_recovery_point(data: &[u8]) -> Result<SeiPayload> {
    if data.is_empty() {
        return Ok(SeiPayload::Unknown(6, data.to_vec()));
    }
    
    let mut recovery_frame_cnt = 0u32;
    let mut pos = 0;
    
    while pos < data.len() {
        let byte = data[pos];
        recovery_frame_cnt = (recovery_frame_cnt << 7) | ((byte & 0x7F) as u32);
        pos += 1;
        if (byte & 0x80) == 0 {
            break;
        }
    }
    
    let mut flags = 0u8;
    if pos < data.len() {
        flags = data[pos];
    }
    
    let exact_match_flag = (flags & 0x80) != 0;
    let broken_link_flag = (flags & 0x40) != 0;
    let changing_slice_group_idc = (flags & 0x30) >> 4;
    
    Ok(SeiPayload::RecoveryPoint {
        recovery_frame_cnt,
        exact_match_flag,
        broken_link_flag,
        changing_slice_group_idc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sei_parse_empty() {
        let rbsp = vec![0x80];
        let messages = SeiMessage::parse(&rbsp).unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_sei_parse_recovery_point() {
        let rbsp = vec![
            0x06,
            0x02,
            0x00,
            0x40,
            0x80,
        ];
        
        let messages = SeiMessage::parse(&rbsp).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].payload_type, 6);
        
        if let SeiPayload::RecoveryPoint { recovery_frame_cnt, .. } = &messages[0].payload {
            assert_eq!(*recovery_frame_cnt, 0);
        } else {
            panic!("Expected RecoveryPoint payload");
        }
    }
}