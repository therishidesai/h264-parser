use h264_parser::AnnexBParser;
use std::fs::File;
use std::io::{Read, Write};


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <h264_file> <output_h264_file>", args[0]);
        return Ok(());
    }

    let mut file = File::open(&args[1])?;
    let mut out_file = File::create(&args[2])?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let mut parser = AnnexBParser::new();
    parser.push(&buffer);

    let mut frame_count = 0;
    let mut keyframe_count = 0;

    while let Ok(Some(au)) = parser.next_access_unit() {
        frame_count += 1;
        
        if au.is_keyframe() {
            keyframe_count += 1;
            eprintln!("Frame {}: KEYFRAME", frame_count);
        } else {
            eprintln!("Frame {}: Regular frame", frame_count);
        }

        if let Some(ref sps) = au.sps {
            eprintln!("  Resolution: {}x{}", sps.width, sps.height);
            eprintln!("  Profile: {}, Level: {}", sps.profile_idc, sps.level_idc);
        }

        eprintln!("  NAL units in frame: {}", au.nals.len());
        for nal in au.nals() {
            eprintln!("    - {:?}", nal.nal_type);
        }

        out_file.write_all(&au.to_annexb_bytes())?;
    }

    eprintln!("\nSummary:");
    eprintln!("Total frames: {}", frame_count);
    eprintln!("Keyframes: {}", keyframe_count);

    Ok(())
}
