# Xbox ADPCM encoder

Safe (and optionally no-std) Rust crate for encoding and decoding Xbox ADPCM blocks.

## Decoding example

Here is example code for decoding stereo audio.

```rust
use xbox_adpcm::{XboxADPCMDecoder, XboxADPCMDecodeSink};

let adpcm_data = read_some_adpcm_blocks();
let mut output = [Vec::new(), Vec::new()];

// Two channel
let mut encoder = XboxADPCMDecoder::new(2, &mut output);

// Decode
encoder.decode(&adpcm_data).unwrap();

assert!(!output.is_empty());
```

## Encoding example

Here is example code for encoding stereo audio.

```rust
use xbox_adpcm::{XboxADPCMEncoder, XboxADPCMEncodeSink};

let (left_channel, right_channel) = read_some_pcm_samples();
let mut output = Vec::new();

// Two channels with a lookahead of three samples
let mut encoder = XboxADPCMEncoder::new(2, 3, &mut output);

// Encode
encoder.encode(&[&left_channel, &right_channel]).unwrap();

// Finish encoding
encoder.finish().unwrap();

assert!(!output.is_empty());
```

## No-std support

The crate is fully functional without the Rust Standard Library, but it is enabled automatically to provide traits for
`XboxADPCMEncodeSink` and `XboxADPCMDecodeSink` on vectors.

To disable using the standard library, put `default-features = false` in the dependency declaration in your Cargo.toml.
See [Features - The Cargo Book](https://doc.rust-lang.org/cargo/reference/features.html) for more information.

# Acknowledgements

The encoder is based off of David Bryant's ADPCM-XQ encoder, an IMA-ADPCM encoder which can be found on GitHub at
[dbry/adpcm-xq](https://github.com/dbry/adpcm-xq).
