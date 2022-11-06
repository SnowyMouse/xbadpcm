use core::convert::TryInto;

use super::*;

/// Maximum block size for all channels assuming max channels.
pub(crate) const ADPCM_BUFFER_SIZE: usize = ADPCM_BLOCK_SIZE * MAX_AUDIO_CHANNEL_COUNT;

/// Writer for outputting PCM samples.
///
/// This is automatically implemented for [`Vec<i16>`](std::vec::Vec) arrays between 1 and 8 if the `"std"` feature is enabled (which it is by default).
#[allow(unused_variables)]
pub trait XboxADPCMDecodeSink {
    type Error: Sized;

    /// Reserve an amount of samples for all channels.
    ///
    /// Implementing this is optional, but it can be used to hint the amount of samples to be written for allocations such as for memory-based buffers.
    fn reserve(&mut self, samples_amount: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Write the samples to the end of the output for each channel.
    ///
    /// Implementing this is **required**.
    fn write(&mut self, samples: &[[i16; SAMPLES_PER_ADPCM_BLOCK]]) -> Result<(), Self::Error>;
}

macro_rules! define_output_audio_sink {
    ($channel_count:expr) => {
        #[cfg(feature = "std")]
        impl XboxADPCMDecodeSink for [std::vec::Vec<i16>; $channel_count] {
            type Error = ();

            fn reserve(&mut self, samples_amount: usize) -> Result<(), Self::Error> {
                for i in 0..$channel_count {
                    self[i].reserve_exact(samples_amount);
                }
                Ok(())
            }

            fn write(&mut self, samples: &[[i16; SAMPLES_PER_ADPCM_BLOCK]]) -> Result<(), Self::Error> {
                for i in 0..$channel_count {
                    self[i].extend_from_slice(&samples[i]);
                }
                Ok(())
            }
        }
    }
}

// Define some default audio sinks
define_output_audio_sink!(1);
define_output_audio_sink!(2);
define_output_audio_sink!(3);
define_output_audio_sink!(4);
define_output_audio_sink!(5);
define_output_audio_sink!(6);
define_output_audio_sink!(7);
define_output_audio_sink!(8);

/// Xbox ADPCM decoder implementation.
pub struct XboxADPCMDecoder<'a, E> {
    /// Number of channels
    num_channels: usize,

    /// Buffer to write
    buffer: [u8; ADPCM_BUFFER_SIZE],

    /// Number of bytes used
    buffer_size: usize,

    /// Sink
    sink: &'a mut dyn XboxADPCMDecodeSink<Error = E>
}

impl<'a, E: Sized> XboxADPCMDecoder<'a, E> {
    /// Initialize an Xbox ADPCM decoder with the given channel count and the output.
    pub fn new(num_channels: usize, sink: &'a mut dyn XboxADPCMDecodeSink<Error = E>) -> XboxADPCMDecoder<'a, E> {
        assert!(num_channels > 0 && num_channels <= MAX_AUDIO_CHANNEL_COUNT, "num_channels must be between 1 and {}", MAX_AUDIO_CHANNEL_COUNT);

        XboxADPCMDecoder {
            num_channels,
            buffer: [0u8; ADPCM_BLOCK_SIZE * MAX_AUDIO_CHANNEL_COUNT],
            buffer_size: 0,
            sink
        }
    }

    /// Decode the given byte array of Xbox ADPCM blocks.
    pub fn decode(&mut self, input: &[u8]) -> Result<(), E> {
        let input_len = input.len();
        let max_buffer_size = ADPCM_BLOCK_SIZE * self.num_channels;

        // Calculate how many samples we will process.
        let total_bytes_after_this = input_len + self.buffer_size;

        // Calculate how many bytes to reserve, even if we may not include everything
        let blocks_to_reserve = (total_bytes_after_this + (max_buffer_size - 1)) / max_buffer_size;
        if blocks_to_reserve > 0 {
            self.sink.reserve(blocks_to_reserve * SAMPLES_PER_ADPCM_BLOCK)?;
        }

        // Load the bytes
        let mut bytes_loaded = 0;
        while bytes_loaded != input_len {
            let bytes_free = max_buffer_size - self.buffer_size;
            let bytes_that_can_be_loaded = bytes_free.min(input_len);
            for b in 0..bytes_that_can_be_loaded {
                self.buffer[self.buffer_size + b] = input[bytes_loaded + b];
            }
            self.buffer_size += bytes_that_can_be_loaded;
            bytes_loaded += bytes_that_can_be_loaded;
            if self.buffer_size == max_buffer_size {
                self.decode_block()?;
            }
        }

        Ok(())
    }

    /// Decode bytes from the buffer.
    fn decode_block(&mut self) -> Result<(), E> {
        let mut samples_to_output = [[0i16; SAMPLES_PER_ADPCM_BLOCK]; MAX_AUDIO_CHANNEL_COUNT];

        let mut last_samples = [0i16; MAX_AUDIO_CHANNEL_COUNT];
        let mut last_step_index = [0usize; MAX_AUDIO_CHANNEL_COUNT];

        // Initialize with the header
        let mut input_offset = 0;
        for ch in 0..self.num_channels {
            let header = &self.buffer[input_offset..input_offset+4];

            let low = header[0] as u16;
            let high = header[1] as u16;
            let sample = ((low) | (high << 8)) as i16;

            last_samples[ch] = sample;
            last_step_index[ch] = clamp_table_index(header[2] as isize);

            input_offset += 4;
        }

        // Decode it
        for c in 0..CHUNKS_PER_BLOCK {
            let output_offset = c * CHUNKS_PER_BLOCK;
            for ch in 0..self.num_channels {

                let mut data = u32::from_le_bytes(self.buffer[input_offset..input_offset+4].try_into().unwrap());
                for s in 0..SAMPLES_PER_CHUNK {
                    let nibble = (data & 0xF) as u8;
                    let new_sample = clamp_sample(last_samples[ch] as i32 + calculate_delta(STEP_TABLE[last_step_index[ch]], nibble)) as i16;
                    last_step_index[ch] = clamp_table_index((last_step_index[ch] as isize) + INDEX_TABLE[nibble as usize]);
                    last_samples[ch] = new_sample;
                    samples_to_output[ch][output_offset + s] = new_sample;

                    data >>= 4; // right shift to get the next four bits
                }

                input_offset += 4;
            }
        }

        // Write it
        self.sink.write(&samples_to_output)?;
        self.buffer_size = 0;
        Ok(())
    }
}
