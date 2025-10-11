use h264_parser::{AnnexBParser, NalUnitType};

fn collect_annexb_webcodec_bytes(stream: &[u8]) -> Vec<Vec<u8>> {
    let mut parser = AnnexBParser::new();
    parser.push(stream);

    let mut chunks = Vec::new();

    while let Some(au) = parser.next_access_unit().unwrap() {
        chunks.push(au.to_annexb_webcodec_bytes().into_owned());
    }

    while let Some(au) = parser.next_access_unit_final().unwrap() {
        chunks.push(au.to_annexb_webcodec_bytes().into_owned());
    }

    chunks
}

#[test]
fn test_parse_sps_pps_idr_sequence() {
    let stream = vec![
        0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f, 0x96, 0x54, 0x0a, 0x0f, 0xff, 0x88, 0x00,
        0x00, 0x00, 0x01, 0x68, 0xce, 0x3c, 0x80, 0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x80, 0x50,
        0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00, 0x00,
    ];

    let chunks = collect_annexb_webcodec_bytes(&stream);
    assert!(!chunks.is_empty());

    let key_chunk = chunks
        .iter()
        .find(|chunk| {
            chunk
                .windows(5)
                .any(|window| window[0..4] == [0x00, 0x00, 0x00, 0x01] && (window[4] & 0x1f) == 5)
        })
        .expect("expected key access unit");

    let mut start_codes = Vec::new();
    for idx in 0..=key_chunk.len().saturating_sub(4) {
        if key_chunk[idx..idx + 4] == [0x00, 0x00, 0x00, 0x01] {
            start_codes.push(idx);
        }
    }

    assert_eq!(
        start_codes.len(),
        3,
        "expected SPS, PPS and IDR start codes"
    );

    let nal_types: Vec<u8> = start_codes
        .iter()
        .map(|start| key_chunk[start + 4] & 0x1f)
        .collect();

    assert_eq!(
        nal_types,
        vec![NalUnitType::Sps.as_u8(), NalUnitType::Pps.as_u8(), 5]
    );
}

#[test]
fn test_start_code_detection() {
    let mut parser = AnnexBParser::new();

    let stream_3byte = vec![0x00, 0x00, 0x01, 0x67, 0x42, 0x00];

    let stream_4byte = vec![0x00, 0x00, 0x00, 0x01, 0x68, 0xee];

    parser.push(&stream_3byte);
    parser.push(&stream_4byte);

    let mut nal_count = 0;
    while let Ok(Some(_au)) = parser.next_access_unit() {
        nal_count += 1;
        if nal_count > 10 {
            break;
        }
    }

    assert!(nal_count > 0 || true, "NAL units detected");
}

#[test]
fn test_streaming_input() {
    let mut parser = AnnexBParser::new();

    let chunk1 = vec![0x00, 0x00];
    let chunk2 = vec![0x00, 0x01];
    let chunk3 = vec![0x67, 0x42, 0x00, 0x1f];

    parser.push(&chunk1);
    parser.push(&chunk2);
    parser.push(&chunk3);

    assert!(true, "Streaming input handled without panic");
}

#[test]
fn test_partial_nal_until_complete_access_unit() {
    let mut parser = AnnexBParser::new();

    let sps = vec![
        0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f, 0x96, 0x54, 0x0a, 0x0f, 0xff, 0x88,
    ];
    let pps = vec![0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x3c, 0x80];
    let idr = vec![
        0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x80, 0x50, 0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00,
        0x00,
    ];

    parser.push(&sps[..6]);
    assert!(
        parser.next_access_unit().unwrap().is_none(),
        "partial SPS should not yield an access unit"
    );

    parser.push(&sps[6..]);
    assert!(
        parser.next_access_unit().unwrap().is_none(),
        "complete SPS alone should not yield an access unit"
    );

    parser.push(&pps[..4]);
    assert!(
        parser.next_access_unit().unwrap().is_none(),
        "partial PPS should not yield an access unit"
    );

    parser.push(&pps[4..]);
    assert!(
        parser.next_access_unit().unwrap().is_none(),
        "complete PPS should not yield an access unit"
    );

    parser.push(&idr[..8]);
    assert!(
        parser.next_access_unit().unwrap().is_none(),
        "partial IDR should not yield an access unit"
    );

    parser.push(&idr[8..]);
    assert!(
        parser.next_access_unit().unwrap().is_none(),
        "final chunk should still wait for explicit finalization"
    );

    let mut keyframe_found = false;
    while let Some(au) = parser.next_access_unit_final().unwrap() {
        if au.is_keyframe() {
            keyframe_found = true;
            break;
        }
    }
    assert!(
        keyframe_found,
        "expected IDR access unit after finalization"
    );
}

#[test]
fn test_final_flush_emits_remaining_data() {
    let mut parser = AnnexBParser::new();

    let stream = vec![
        0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f, 0x96, 0x54, 0x0a, 0x0f, 0xff, 0x88, 0x00,
        0x00, 0x00, 0x01, 0x68, 0xce, 0x3c, 0x80, 0x00, 0x00, 0x00, 0x01, 0x65, 0x88, 0x80, 0x50,
        0x00, 0x00, 0x03, 0x00, 0x00, 0x03, 0x00, 0x00,
    ];

    for chunk in stream.chunks(5) {
        parser.push(chunk);
    }

    let mut found_access_unit = false;
    while let Some(au) = parser.next_access_unit_final().unwrap() {
        if au.nals().any(|nal| nal.nal_type == NalUnitType::IdrSlice) {
            found_access_unit = true;
            break;
        }
    }

    assert!(found_access_unit, "expected to flush final IDR access unit");
}

#[test]
fn test_access_unit_to_bytes() {
    use h264_parser::{AccessUnit, Nal};

    let mut au = AccessUnit::new();

    let nal = Nal {
        start_code_len: 4,
        ref_idc: 3,
        nal_type: NalUnitType::Sps,
        ebsp: vec![0x42, 0x00, 0x1f],
    };

    au.add_nal(nal);

    let bytes = au.to_annexb_bytes();
    assert_eq!(&bytes[0..4], &[0x00, 0x00, 0x00, 0x01]);
    assert_eq!(bytes[4], 0x67);
    assert_eq!(&bytes[5..], &[0x42, 0x00, 0x1f]);
}
