// tests/proptests.rs
#![allow(clippy::needless_return)]

use proptest::prelude::*;

// Bring your crate into scope. Adjust if the crate name differs.
use h264_parser::AnnexBParser;

/// ------------------------------------
/// 2) EOF semantics / Draining behavior
/// ------------------------------------
/// After we've consumed everything and called end_of_stream(), repeated calls to
/// next_access_unit() should keep returning Ok(None).
#[test]
fn eof_semantics_next_access_unit_is_none_after_drain() {
    let mut parser = AnnexBParser::new();

    // Minimal empty push, then EOS.
    parser.push(&[]);

    // Drain fully.
    assert!(matches!(parser.next_access_unit(), Ok(None)));
    assert!(matches!(parser.next_access_unit(), Ok(None)));
    assert!(matches!(parser.next_access_unit(), Ok(None)));
}

/// -------------------------------------------------------------------------
/// 3) Structured generator: produce tiny valid SPS/PPS + (IDR|non-IDR) slice
/// -------------------------------------------------------------------------
/// We build a minimal, valid Annex B stream using:
///   - POC type 2 (to avoid extra POC fields)
///   - log2_max_frame_num_minus4 = 0  => frame_num bit width = 4
/// Slice header contains only fields needed for AU grouping.
/// This lets us assert is_keyframe correctness deterministically.
proptest! {
    #[test]
    fn keyframe_flag_matches_idr(idr in any::<bool>()) {
        // Build a two-frame stream: first AU can be IDR or non-IDR,
        // second AU is always non-IDR. Then check flags.

        let sps = build_min_sps_pic_order_cnt_type2(/*sps_id=*/0);
        let pps = build_min_pps(/*pps_id=*/0, /*sps_id=*/0);
        let first = build_min_slice(/*idr=*/idr, /*pps_id=*/0, /*frame_num=*/0, /*idr_pic_id=*/0);
        let second = build_min_slice(/*idr=*/false, /*pps_id=*/0, /*frame_num=*/1, /*idr_pic_id=*/0);
        

        // Annex B with start codes, NO AUD to avoid extra boundaries
        let mut stream = Vec::new();
        push_start_code(&mut stream); stream.extend_from_slice(&sps);
        push_start_code(&mut stream); stream.extend_from_slice(&pps);
        push_start_code(&mut stream); stream.extend_from_slice(&first);
        
        // Second AU (different frame_num will create boundary)
        push_start_code(&mut stream); stream.extend_from_slice(&second);

        
        let mut parser = AnnexBParser::new();
        parser.push(&stream);
        // Signal end of stream to flush any pending data
        parser.push(&[]);

        let mut aus = Vec::new();
        while let Ok(Some(au)) = parser.next_access_unit() {
            aus.push(au);
        }
        
        // Filter to only AUs that contain slices (VCL NALs)
        let slice_aus: Vec<_> = aus.into_iter()
            .filter(|au| au.nals.iter().any(|n| n.is_vcl()))
            .collect();
        
        // We should have 2 AUs with slices
        assert_eq!(slice_aus.len(), 2, "Expected exactly 2 AUs with slices, got {}", slice_aus.len());

        // The first slice AU keyframe flag should match whether we used IDR
        assert_eq!(slice_aus[0].is_keyframe(), idr, "First slice AU keyframe mismatch");

        // The second slice AU is non-IDR
        assert_eq!(slice_aus[1].is_keyframe(), false, "Second slice AU must be non-keyframe");
    }
}

// ------------------------------------------------------
// 4) Chunking invariants: split at arbitrary boundaries
// ------------------------------------------------------
// Pushing the same bytestream as one big chunk vs. many tiny chunks
// should yield the same number of AUs.
proptest! {
    #[test]
    fn chunking_yields_same_au_count(splits in proptest::collection::vec(1usize..50usize, 0..50)) {
        // Build a simple stream with two AUs (IDR then non-IDR)
        let sps = build_min_sps_pic_order_cnt_type2(0);
        let pps = build_min_pps(0, 0);
        let idr  = build_min_slice(true,  0, 0, 0);
        let p    = build_min_slice(false, 0, 1, 0);

        let mut stream = Vec::new();
        push_start_code(&mut stream); stream.extend_from_slice(&sps);
        push_start_code(&mut stream); stream.extend_from_slice(&pps);
        push_start_code(&mut stream); stream.extend_from_slice(&idr);
        push_start_code(&mut stream); stream.extend_from_slice(&p);

        // Parse all at once
        let mut p1 = AnnexBParser::new();
        p1.push(&stream);
        let mut count_all_at_once = 0;
        while let Ok(Some(_)) = p1.next_access_unit() {
            count_all_at_once += 1;
        }

        // Parse in many chunks
        let mut p2 = AnnexBParser::new();
        let mut i = 0usize;
        for step in splits {
            if i >= stream.len() { break; }
            let end = (i + step).min(stream.len());
            p2.push(&stream[i..end]);
            i = end;
        }
        if i < stream.len() {
            p2.push(&stream[i..]);
        }

        let mut count_chunked = 0;
        while let Ok(Some(_)) = p2.next_access_unit() {
            count_chunked += 1;
        }

        assert_eq!(count_all_at_once, count_chunked, "AU count differs with chunking");
    }
}

/* -----------------------------
   Helpers: minimal bit/UE writer
   ----------------------------- */

fn push_start_code(dst: &mut Vec<u8>) {
    dst.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
}

fn nal_header(ref_idc: u8, nal_unit_type: u8) -> u8 {
    // forbidden_zero_bit=0, nal_ref_idc in bits 5-6, nal_unit_type in bits 0-4
    ((ref_idc & 0b11) << 5) | (nal_unit_type & 0b1_1111)
}

/// Build minimal SPS with:
///  - profile/level fixed
///  - seq_parameter_set_id = sps_id
///  - log2_max_frame_num_minus4 = 0  => frame_num uses 4 bits
///  - pic_order_cnt_type = 2 (simplest)
///  - frame_mbs_only_flag = 1  => progressive
///  - 16x16 picture (cropping disabled)
fn build_min_sps_pic_order_cnt_type2(sps_id: u32) -> Vec<u8> {
    let mut rbsp = BitWriter::new();
    // nal header comes from outer caller; here only RBSP
    // profile_idc (8), constraint flags (8), level_idc (8)
    rbsp.write_u8(66);  // baseline
    rbsp.write_u8(0);   // constraint flags
    rbsp.write_u8(30);  // level 3.0

    ue(&mut rbsp, sps_id);            // seq_parameter_set_id
    ue(&mut rbsp, 0);                 // log2_max_frame_num_minus4 = 0  (frame_num: 4 bits)
    ue(&mut rbsp, 2);                 // pic_order_cnt_type = 2
    // for type 2: no extra POC fields
    ue(&mut rbsp, 0);                 // max_num_ref_frames
    rbsp.write_flag(false);           // gaps_in_frame_num_value_allowed_flag

    // Width/height in macroblocks: 1x1 => 16x16
    ue(&mut rbsp, 0);                 // pic_width_in_mbs_minus1
    ue(&mut rbsp, 0);                 // pic_height_in_map_units_minus1
    rbsp.write_flag(true);            // frame_mbs_only_flag = 1
    // if frame_mbs_only_flag==0 then mb_adaptive_frame_field_flag
    rbsp.write_flag(false);           // direct_8x8_inference_flag
    rbsp.write_flag(false);           // frame_cropping_flag = 0
    rbsp.write_flag(false);           // vui_parameters_present_flag = 0

    let mut out = Vec::new();
    out.push(nal_header(3, 7)); // SPS (type=7), ref_idc=3
    out.extend_from_slice(&rbsp.finish_trailing_bits());
    out
}

/// Build minimal PPS referencing given SPS.
fn build_min_pps(pps_id: u32, sps_id: u32) -> Vec<u8> {
    let mut rbsp = BitWriter::new();
    ue(&mut rbsp, pps_id);          // pic_parameter_set_id
    ue(&mut rbsp, sps_id);          // seq_parameter_set_id
    rbsp.write_flag(false);         // entropy_coding_mode_flag (CAVLC)
    rbsp.write_flag(false);         // pic_order_present_flag
    ue(&mut rbsp, 0);               // num_slice_groups_minus1 == 0
    ue(&mut rbsp, 0);               // num_ref_idx_l0_default_active_minus1
    ue(&mut rbsp, 0);               // num_ref_idx_l1_default_active_minus1
    rbsp.write_flag(false);         // weighted_pred_flag
    rbsp.write_bits(2, 0);          // weighted_bipred_idc = 0 (was incorrectly 2)
    se(&mut rbsp, 0);               // pic_init_qp_minus26
    se(&mut rbsp, 0);               // pic_init_qs_minus26
    se(&mut rbsp, 0);               // chroma_qp_index_offset
    rbsp.write_flag(false);         // deblocking_filter_control_present_flag
    rbsp.write_flag(false);         // constrained_intra_pred_flag
    rbsp.write_flag(false);         // redundant_pic_cnt_present_flag

    let mut out = Vec::new();
    out.push(nal_header(3, 8)); // PPS (type=8)
    out.extend_from_slice(&rbsp.finish_trailing_bits());
    out
}

/// Build minimal slice RBSP for either IDR (nal_unit_type=5) or non-IDR (1).
/// Assumes:
///  - PPS id 0, SPS with log2_max_frame_num_minus4=0 (so frame_num is 4 bits)
///  - POC type 2
fn build_min_slice(idr: bool, pps_id: u32, frame_num: u32, idr_pic_id: u32) -> Vec<u8> {
    let mut rbsp = BitWriter::new();
    ue(&mut rbsp, 0);                // first_mb_in_slice
    ue(&mut rbsp, if idr { 2 } else { 0 }); // slice_type base: 2=I, 0=P
    ue(&mut rbsp, pps_id);           // pic_parameter_set_id
    rbsp.write_bits(4, frame_num);   // frame_num (4 bits because log2_max_frame_num_minus4=0)
    if idr {
        ue(&mut rbsp, idr_pic_id);   // idr_pic_id (only for IDR)
    }
    // POC type 2 => no POC fields
    
    // Add minimal slice data to make it a valid slice
    // For simplicity, we'll add some dummy bits to make the slice look valid
    // These represent the simplest macroblock data
    if !idr {
        // For P slices, add num_ref_idx_active_override_flag
        rbsp.write_flag(false);
    }
    
    // Add a simple macroblock (mb_skip_run for P, or I macroblock for I)
    if idr {
        // I slice: mb_type (Intra 16x16)
        ue(&mut rbsp, 0);  // mb_type for I_16x16
    } else {
        // P slice: mb_skip_run
        ue(&mut rbsp, 1);  // Skip one macroblock
    }

    let mut out = Vec::new();
    out.push(nal_header(3, if idr { 5 } else { 1 })); // IdrSlice or NonIdrSlice
    out.extend_from_slice(&rbsp.finish_trailing_bits());
    out
}

/* --------------------------
   Tiny RBSP bit writer utils
   -------------------------- */

struct BitWriter {
    bytes: Vec<u8>,
    cur: u8,
    nbits: u8,
}

impl BitWriter {
    fn new() -> Self { Self { bytes: Vec::new(), cur: 0, nbits: 0 } }

    fn write_bit(&mut self, bit: bool) {
        self.cur <<= 1;
        if bit { self.cur |= 1; }
        self.nbits += 1;
        if self.nbits == 8 {
            self.bytes.push(self.cur);
            self.cur = 0;
            self.nbits = 0;
        }
    }

    fn write_bits(&mut self, n: u32, val: u32) {
        for i in (0..n).rev() {
            let b = ((val >> i) & 1) != 0;
            self.write_bit(b);
        }
    }

    fn write_flag(&mut self, b: bool) { self.write_bit(b); }

    fn write_u8(&mut self, v: u8) {
        for i in (0..8).rev() {
            self.write_bit(((v >> i) & 1) != 0);
        }
    }

    fn finish_trailing_bits(mut self) -> Vec<u8> {
        // RBSP trailing bits: a single '1' bit then pad with '0' to next byte
        self.write_bit(true);
        while self.nbits != 0 {
            self.write_bit(false);
        }
        self.bytes
    }
}

// Unsigned Exp-Golomb
fn ue(w: &mut BitWriter, v: u32) {
    if v == 0 {
        w.write_bit(true);  // Write '1' for value 0
        return;
    }
    
    let code_num = v + 1;
    let bits = 32 - code_num.leading_zeros();
    let prefix_zeros = (bits - 1) as usize;
    for _ in 0..prefix_zeros { w.write_bit(false); }
    // write info bits (code_num in 'bits' bits)
    for i in (0..bits).rev() {
        w.write_bit(((code_num >> i) & 1) != 0);
    }
}

// Signed Exp-Golomb
fn se(w: &mut BitWriter, v: i32) {
    let k = if v > 0 { (v as u32) * 2 - 1 } else { (-v as u32) * 2 };
    ue(w, k);
}
