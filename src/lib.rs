//! Safe (and optionally no-std) Rust crate for encoding and decoding Xbox ADPCM blocks.
//!
//! # Decoding example
//!
//! Here is example code for decoding stereo audio.
//!
//! ```
//! # fn main() -> Result<(), ()> {
//! use xbox_adpcm::{XboxADPCMDecoder, XboxADPCMDecodeSink};
//!
//! let adpcm_data = read_some_adpcm_blocks();
//! let mut output = [Vec::new(), Vec::new()];
//!
//! // Two channel
//! let mut encoder = XboxADPCMDecoder::new(2, &mut output);
//!
//! // Decode
//! encoder.decode(&adpcm_data).unwrap();
//!
//! assert!(!output.is_empty());
//! # Ok(())
//! # }
//! # fn read_some_adpcm_blocks() -> (Vec<u8>) {
//! #    return (vec![0u8; 72])
//! # }
//! ```
//!
//! # Encoding example
//!
//! Here is example code for encoding stereo audio.
//!
//! ```
//! # fn main() -> Result<(), ()> {
//! use xbox_adpcm::{XboxADPCMEncoder, XboxADPCMEncodeSink};
//!
//! let (left_channel, right_channel) = read_some_pcm_samples();
//! let mut output = Vec::new();
//!
//! // Two channels with a lookahead of three samples
//! let mut encoder = XboxADPCMEncoder::new(2, 3, &mut output);
//!
//! // Encode
//! encoder.encode(&[&left_channel, &right_channel]).unwrap();
//!
//! // Finish encoding
//! encoder.finish().unwrap();
//!
//! assert!(!output.is_empty());
//! # Ok(())
//! # }
//! # fn read_some_pcm_samples() -> (Vec<i16>, Vec<i16>) {
//! #    return (vec![0i16; 5], vec![0i16; 5])
//! # }

#![no_std]

#[cfg(feature = "std")]
extern crate std;

mod util;
use util::*;

mod encoder;
pub use encoder::*;

mod decoder;
pub use decoder::*;
