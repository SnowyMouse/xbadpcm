# Xbox ADPCM encoder

Safe (and optionally no-std) Rust crate for encoding Xbox ADPCM samples.

This is originally based off of David Bryant's ADPCM-XQ encoder, an IMA-ADPCM encoder which can be found at [https://github.com/dbry/adpcm-xq](ADPCM-XQ).

Currently only lookahead is implemented as opposed to noise shaping.

## Examples

Here is example code for encoding stereo audio.

```rust
use xbox_adpcm::{XboxADPCMEncoder, XboxADPCMEncodeSink};

let (left_channel, right_channel) = read_some_samples();
let mut output = Vec::new();

// Two channels with a lookahead of three samples
let mut encoder = XboxADPCMEncoder::new(2, 3, &mut output);

// Encode
encoder.encode(&[&left_channel, &right_channel]).unwrap();

// Finish encoding
encoder.finish().unwrap();

assert!(!output.is_empty());
```

