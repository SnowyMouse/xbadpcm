use crate::*;

/// Writer outputting ADPCM blocks.
///
/// This is automatically implemented for [`Vec<u8>`](std::vec::Vec) if the `"std"` feature is enabled (which it is by default).
#[allow(unused_variables)]
pub trait XboxADPCMEncodeSink {
    type Error: Sized;

    /// Reserve an amount of bytes at the end of the output.
    ///
    /// Implementing this is optional, but it can be used to hint the amount of bytes to be written for allocations such as for memory-based buffers.
    fn reserve(&mut self, bytes_amount: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Write the bytes to the end of the output.
    ///
    /// Implementing this is **required**.
    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
}

#[cfg(feature = "std")]
impl XboxADPCMEncodeSink for std::vec::Vec<u8> {
    type Error = ();

    fn reserve(&mut self, bytes_amount: usize) -> Result<(), Self::Error> {
        Ok(self.reserve_exact(bytes_amount))
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        Ok(self.extend_from_slice(&bytes))
    }
}

// Buffer size to use in the encoder. We keep extra samples at the end so we have a few extra samples to go by at the end.
pub(crate) const PCM_BUFFER_EXTRA: usize = 2;
pub(crate) const PCM_BUFFER_CAPACITY: usize = SAMPLES_PER_ADPCM_BLOCK + PCM_BUFFER_EXTRA;

/// XboxADPCM encoder implementation.
pub struct XboxADPCMEncoder<'a, E> {
    /// Channel data (from adpcm-xq)
    channels: [ADPCMChannel; MAX_AUDIO_CHANNEL_COUNT],

    /// Number of channels
    num_channels: usize,

    /// Lookahead value
    lookahead: usize,

    /// Buffer containing the next samples to be processed
    buffer: [[i16; PCM_BUFFER_CAPACITY]; MAX_AUDIO_CHANNEL_COUNT],

    /// Current size of the buffer
    buffer_size: usize,

    /// Did we initialize the predictors?
    predictors_initialized: bool,

    /// Output buffer
    sink: &'a mut dyn XboxADPCMEncodeSink<Error = E>
}

impl<'a, E> XboxADPCMEncoder<'a, E> where E: Sized {
    /// Initialize an encoder with the given channel count, and lookahead for the given sink.
    ///
    /// Higher lookahead may slightly reduce noise, but it will also exponentially increase encoding time.
    ///
    /// # Panics
    ///
    /// Panics if `num_channels` is not between 1 and 8
    pub fn new(num_channels: usize, lookahead: u8, sink: &'a mut dyn XboxADPCMEncodeSink<Error = E>) -> XboxADPCMEncoder<'a, E> {
        assert!(num_channels > 0 && num_channels <= MAX_AUDIO_CHANNEL_COUNT, "num_channels must be between 1 and {}", MAX_AUDIO_CHANNEL_COUNT);

        XboxADPCMEncoder {
            channels: <[ADPCMChannel; MAX_AUDIO_CHANNEL_COUNT]>::default(),
            num_channels,
            lookahead: lookahead as usize,
            buffer_size: 0,
            buffer: [[0i16; PCM_BUFFER_CAPACITY]; MAX_AUDIO_CHANNEL_COUNT],
            predictors_initialized: false,
            sink
        }
    }

    /// Encode with the given samples using some samples.
    ///
    /// Note that this may not always encode all samples passed and may store some in a buffer. To flush the buffer, run [`XboxADPCMEncoder::finish`].
    ///
    /// # Panics
    ///
    /// Panics if the input has the wrong number of channels or the samples are wrong.
    pub fn encode<B: AsRef<[C]>, C: AsRef<[i16]>>(&mut self, input: B) -> Result<(), E> {
        let input_arr = input.as_ref();
        assert_eq!(self.num_channels, input_arr.len(), "input channel count is incorrect");

        let sample_count = input_arr[0].as_ref().len();
        for i in 1..self.num_channels {
            assert_eq!(sample_count, input_arr[i].as_ref().len(), "sample count of channel {i} does not match the sample count of channel 0");
        }

        // Calculate how many samples we will process.
        let total_samples_after_this = sample_count + self.buffer_size;

        // Predict how many bytes we will need to reserve, always rounding up to the next block.
        //
        // If we have any samples, we need at least one block even if we may not immediately encode them yet.
        if total_samples_after_this != 0 {
            self.sink.reserve((total_samples_after_this + (SAMPLES_PER_ADPCM_BLOCK - 1)) / SAMPLES_PER_ADPCM_BLOCK * ADPCM_BLOCK_SIZE * self.num_channels)?;
        }

        // Process all samples.
        let mut samples_loaded = 0;
        while samples_loaded != sample_count {
            let samples_left_to_load = sample_count - samples_loaded;
            let samples_free = PCM_BUFFER_CAPACITY - self.buffer_size;
            let samples_that_can_be_loaded = samples_free.min(samples_left_to_load);
            for c in 0..self.num_channels {
                let input_samples = &input_arr[c].as_ref()[samples_loaded..];
                let buff_samples = &mut self.buffer[c][self.buffer_size..];
                for i in 0..samples_that_can_be_loaded {
                    buff_samples[i] = input_samples[i];
                }
            }

            samples_loaded += samples_that_can_be_loaded;
            self.buffer_size += samples_that_can_be_loaded;

            if self.buffer_size == PCM_BUFFER_CAPACITY {
                self.initialize_predictors();
                self.encode_block()?;
            }
        }

        Ok(())
    }

    /// Finish encoding and then resets the encoder.
    ///
    /// This will encode all remaining samples, filling any unused samples with silence. If a simple reset is desired without any further writes, call [`XboxADPCMEncoder::reset`] instead.
    pub fn finish(&mut self) -> Result<(), E> {
        if self.buffer_size != 0 {
            // Init predictors
            self.initialize_predictors();

            // Zero-out everything at the end and set our buffer size.
            for c in &mut self.buffer[0..self.num_channels] {
                for b in &mut c[self.buffer_size..PCM_BUFFER_CAPACITY] {
                    *b = 0;
                }
            }
            self.buffer_size = PCM_BUFFER_CAPACITY;

            // Encode what is left
            self.encode_block()?;
        }
        self.reset();
        Ok(())
    }

    /// Reset the encoder immediately without writing any more samples.
    ///
    /// Any samples yet to be encoded will be dropped. If this is not desired, call [`XboxADPCMEncoder::finish`] instead.
    pub fn reset(&mut self) {
        self.predictors_initialized = false;
        self.buffer_size = 0;
    }

    /// Encode the contents of the buffer.
    fn encode_block(&mut self) -> Result<(), E> {
        debug_assert_eq!(PCM_BUFFER_CAPACITY, self.buffer_size, "called encode_block on a non-populated sample buffer");
        debug_assert!(self.predictors_initialized, "called encode_block but predictors not initialized");

        let mut bytes_to_write = [0u8; ADPCM_BLOCK_SIZE * MAX_AUDIO_CHANNEL_COUNT];
        let total_bytes_to_write = ADPCM_BLOCK_SIZE * self.num_channels;

        // Write the header
        for ch in 0..self.num_channels {
            // Get our first sample and set it since it's uncompressed.
            let s = self.buffer[ch][0];
            bytes_to_write[0 + ch * 4] = (s & 0xFF) as u8; // write the first sample uncompressed
            bytes_to_write[1 + ch * 4] = ((s >> 8) & 0xFF) as u8;
            bytes_to_write[2 + ch * 4] = self.channels[ch].index as u8;
            self.channels[ch].pcmdata = s as i32;
        }

        // Write the chunks
        self.encode_chunks(&mut bytes_to_write[self.num_channels * 4..total_bytes_to_write]);
        for ch in 0..self.num_channels {
            for b in 0..PCM_BUFFER_EXTRA {
                self.buffer[ch][b] = self.buffer[ch][PCM_BUFFER_CAPACITY - PCM_BUFFER_EXTRA + b]; // copy the last samples back to the beginning
            }
        }
        self.buffer_size = PCM_BUFFER_EXTRA;

        // Write all of it
        self.sink.write(&bytes_to_write[..total_bytes_to_write])
    }

    /// Encode all chunks
    fn encode_chunks(&mut self, output: &mut [u8]) {
        const BYTES_PER_CHANNEL_PER_BLOCK: usize = SAMPLES_PER_CHUNK / 2;
        let output_channel_stride = self.num_channels * BYTES_PER_CHANNEL_PER_BLOCK;

        for chunk in 0..CHUNKS_PER_BLOCK {
            let chunk_start = 1 + chunk * SAMPLES_PER_CHUNK;
            let output_offset = output_channel_stride * chunk;
            for channel in 0..self.num_channels {
                let output_offset = output_offset + channel * BYTES_PER_CHANNEL_PER_BLOCK;
                let chunk_samples = &self.buffer[channel][chunk_start..];
                for i in 0..BYTES_PER_CHANNEL_PER_BLOCK {
                    let pchan = &mut self.channels[channel];
                    let buff_offset = i * 2;
                    let low = encode_sample(pchan, self.lookahead, &chunk_samples[buff_offset..]);
                    let high = encode_sample(pchan, self.lookahead, &chunk_samples[buff_offset + 1..]);
                    output[output_offset + i] = low | (high << 4);
                }
            }
        }
    }

    /// Initialize predictors with the contents of the buffer.
    ///
    /// This should be called whenever a block is encoded.
    fn initialize_predictors(&mut self) {
        if self.predictors_initialized {
            return
        }
        for c in 0..self.num_channels {
            // Calculate initial ADPCM predictors using decaying average
            let mut avg = 0;
            let buffer = &self.buffer[c];
            for i in 1..self.buffer_size {
                let this_sample = buffer[i] as i32;
                let prev_sample = buffer[i-1] as i32;
                avg = (avg + (this_sample - prev_sample)) / 8;
            }

            // Set our initial step index to this
            let mut initial_index = STEP_TABLE.len() - 1;
            for i in 0..STEP_TABLE.len() - 1 {
                let table_val = STEP_TABLE[i];
                let table_val_next = STEP_TABLE[i + 1];
                let table_avg = ((table_val as i32) + (table_val_next as i32)) / 2;
                if avg < table_avg {
                    initial_index = i;
                    break;
                }
            }

            self.channels[c] = ADPCMChannel {
                pcmdata: 0,
                index: initial_index
            };
        }
        self.predictors_initialized = true
    }
}

/// Calculate minimum error recursively.
fn calculate_minimum_error(index: usize, pcmdata: i32, sample: i32, samples: &[i16], lookahead: usize, best_nibble: &mut u8) -> f64 {
    let calculate_minimum_error_next = |index: usize, pcmdata: i32, nibble: u8| -> f64 {
        let index = clamp_table_index(index as isize + INDEX_TABLE[nibble as usize & 0x7]);
        calculate_minimum_error(index, pcmdata, samples[0] as i32, &samples[1..], lookahead - 1, &mut 0)
    };

    // Get our delta!
    let delta = sample - pcmdata;
    let step = STEP_TABLE[index] as u16;

    // Encode our nibble
    let nibble = if delta < 0 {
        ((-delta << 2) as u32 / step as u32).min(7) as u8 | 0x8
    }
    else {
        ((delta << 2) as u32 / step as u32).min(7) as u8
    };
    *best_nibble = nibble;

    // Calculate the minimum error. Return if base case.
    let pcmdata_a = clamp_sample(pcmdata + calculate_delta(step, nibble));
    let mut min_error = pcmdata_a.abs_diff(sample).pow(2) as f64;
    if lookahead == 0 {
        return min_error;
    }
    min_error += calculate_minimum_error_next(index, pcmdata_a, nibble);

    // Calculate all other possible nibbles
    for nibble2 in 0..=0xF {
        if nibble2 == nibble {
            continue
        }

        let pcmdata_b = clamp_sample(pcmdata + calculate_delta(step, nibble2));
        let error = pcmdata_b.abs_diff(sample).pow(2) as f64;

        // If the error is already too high, skip so we don't do any (possibly) slow recursion
        if error >= min_error {
            continue
        }

        let error = error + calculate_minimum_error_next(index, pcmdata_b, nibble2);
        if error >= min_error {
            continue
        }

        *best_nibble = nibble2;
        min_error = error;
    }

    min_error
}

/// Encode the samples.
fn encode_sample(pchan: &mut ADPCMChannel, lookahead: usize, samples: &[i16]) -> u8 {
    let current_sample = samples[0] as i32;
    let next_samples = &samples[1..];
    let step = STEP_TABLE[pchan.index];

    let mut nibble = 0;
    calculate_minimum_error(pchan.index, pchan.pcmdata, current_sample, next_samples, lookahead.min(next_samples.len()), &mut nibble);
    pchan.index = clamp_table_index(pchan.index as isize + INDEX_TABLE[(nibble & 0x7) as usize]) as usize;
    pchan.pcmdata = clamp_sample(pchan.pcmdata + calculate_delta(step, nibble));

    nibble
}
