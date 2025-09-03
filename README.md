# H.264 Annex B Parser

A Rust library for parsing H.264 Annex B bitstreams. This library provides zero-copy parsing of NAL units, parameter sets (SPS/PPS), slice headers, and access unit assembly.

## Features

- **Annex B parsing**: Handles start codes (0x000001 and 0x00000001)
- **NAL unit parsing**: Extracts and processes Network Abstraction Layer units
- **Parameter sets**: Decodes SPS (Sequence Parameter Set) and PPS (Picture Parameter Set)
- **Access Units**: Groups NAL units into frames/pictures
- **Keyframe detection**: Identifies IDR frames and recovery points
- **SEI parsing**: Basic support for Supplemental Enhancement Information
- **Streaming support**: Handles chunked input data
- **Zero-copy design**: Minimizes memory allocations where possible

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
h264-parser = "0.1.0"
```

Basic example:

```rust
use h264_parser::AnnexBParser;
use std::fs::File;
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open("video.h264")?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let mut parser = AnnexBParser::new();
    parser.push(&buffer);

    while let Ok(Some(au)) = parser.next_access_unit() {
        println!("Frame: keyframe={}", au.is_keyframe());
        
        if let Some(ref sps) = au.sps {
            println!("  Resolution: {}x{}", sps.width, sps.height);
        }
        
        for nal in au.nals() {
            println!("  NAL: {:?}", nal.nal_type);
        }
    }

    Ok(())
}
```

## Architecture

The library is organized into the following modules:

- **bytescan**: Start code detection and NAL unit tokenization
- **nal**: NAL header parsing and EBSP/RBSP conversion
- **bitreader**: Bit-level reading utilities
- **eg**: Exp-Golomb encoding/decoding
- **sps**: Sequence Parameter Set parsing
- **pps**: Picture Parameter Set parsing
- **sei**: SEI message parsing
- **slice**: Slice header parsing
- **au**: Access Unit assembly
- **parser**: Main parser facade

## Supported NAL Unit Types

- Non-IDR slices (P/B frames)
- IDR slices (I frames)
- SPS (Sequence Parameter Set)
- PPS (Picture Parameter Set)
- SEI (Supplemental Enhancement Information)
- AUD (Access Unit Delimiter)
- End of sequence/stream markers

## Limitations

This is a parsing-only library that:
- Does not perform full H.264 decoding
- Does not manage DPB (Decoded Picture Buffer)
- Does not handle RTP packetization
- Does not provide muxing/demuxing capabilities

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.