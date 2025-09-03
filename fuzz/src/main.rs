use afl::*;

use h264_parser::AnnexBParser;

fn main() {
    fuzz!(|data: &[u8]|{
        let mut parser = AnnexBParser::new();
        parser.push(&data);

        while let Ok(Some(_au)) = parser.next_access_unit() {
            // do nothing, just make sure nothing panics
        }
    })
}

