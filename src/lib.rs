pub mod au;
pub mod bitreader;
pub mod bytescan;
pub mod eg;
pub mod nal;
pub mod parser;
pub mod pps;
pub mod sei;
pub mod slice;
pub mod sps;

pub use au::{AccessUnit, AccessUnitKind};
pub use nal::{Nal, NalUnitType};
pub use parser::AnnexBParser;
pub use pps::Pps;
pub use sps::Sps;

use std::error::Error as StdError;
use std::fmt;

#[derive(Debug, Clone)]
pub enum Error {
    InvalidNalHeader,
    MalformedSps(String),
    MalformedPps(String),
    SliceParseError(String),
    MissingPps(u8),
    MissingSps(u8),
    UnexpectedEof,
    InvalidStartCode,
    BitstreamError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidNalHeader => write!(f, "Invalid NAL header"),
            Error::MalformedSps(msg) => write!(f, "Malformed SPS: {}", msg),
            Error::MalformedPps(msg) => write!(f, "Malformed PPS: {}", msg),
            Error::SliceParseError(msg) => write!(f, "Slice parse error: {}", msg),
            Error::MissingPps(id) => write!(f, "Missing PPS with id {}", id),
            Error::MissingSps(id) => write!(f, "Missing SPS with id {}", id),
            Error::UnexpectedEof => write!(f, "Unexpected end of file"),
            Error::InvalidStartCode => write!(f, "Invalid start code"),
            Error::BitstreamError(msg) => write!(f, "Bitstream error: {}", msg),
        }
    }
}

impl StdError for Error {}

pub type Result<T> = std::result::Result<T, Error>;