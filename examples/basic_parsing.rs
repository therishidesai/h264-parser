use h264_parser::AnnexBParser;
use std::fs::File;
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <h264_file>", args[0]);
        return Ok(());
    }

    let mut file = File::open(&args[1])?;
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
            println!("Frame {}: KEYFRAME", frame_count);
        } else {
            println!("Frame {}: Regular frame", frame_count);
        }

        if let Some(ref sps) = au.sps {
            println!("  Resolution: {}x{}", sps.width, sps.height);
            println!("  Profile: {}, Level: {}", sps.profile_idc, sps.level_idc);
        }

        println!("  NAL units in frame: {}", au.nals.len());
        for nal in au.nals() {
            println!("    - {:?}", nal.nal_type);
        }
    }

    println!("\nSummary:");
    println!("Total frames: {}", frame_count);
    println!("Keyframes: {}", keyframe_count);

    Ok(())
}