/// Number of samples per chunk
pub(crate) const SAMPLES_PER_CHUNK: usize = 8;

/// Number of chunks per chunk
pub(crate) const CHUNKS_PER_BLOCK: usize = 8;

/// Number of compressed samples per block (the first sample is stored uncompressed)
pub(crate) const HALF_BYTE_SAMPLES_PER_ADPCM_BLOCK: usize = SAMPLES_PER_CHUNK * CHUNKS_PER_BLOCK;

/// Number of samples per block
pub(crate) const SAMPLES_PER_ADPCM_BLOCK: usize = HALF_BYTE_SAMPLES_PER_ADPCM_BLOCK;

/// Size of an ADPCM block in bytes
pub(crate) const ADPCM_BLOCK_SIZE: usize = 4 + (HALF_BYTE_SAMPLES_PER_ADPCM_BLOCK * 4 / 8); // 4 bits per sample

/// Max channel count supported by the encoder
pub(crate) const MAX_AUDIO_CHANNEL_COUNT: usize = 8;

pub(crate) const STEP_TABLE: [u16; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17,
    19, 21, 23, 25, 28, 31, 34, 37, 41, 45,
    50, 55, 60, 66, 73, 80, 88, 97, 107, 118,
    130, 143, 157, 173, 190, 209, 230, 253, 279, 307,
    337, 371, 408, 449, 494, 544, 598, 658, 724, 796,
    876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358,
    5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899,
    15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767
];

pub(crate) const INDEX_TABLE: [isize; 16] = [
    -1, -1, -1, -1, 2, 4, 6, 8,
    -1, -1, -1, -1, 2, 4, 6, 8
];

#[derive(Default, Copy, Clone)]
pub(crate) struct ADPCMChannel {
    pub pcmdata: i32,
    pub index: usize
}

/// Clamp the sample to a 16-bit width
pub(crate) fn clamp_sample(input: i32) -> i32 {
    input.clamp(i16::MIN as i32, i16::MAX as i32)
}

/// Clamp the table index to the step table length
pub(crate) fn clamp_table_index(index: isize) -> usize {
    index.clamp(0, STEP_TABLE.len() as isize - 1) as usize
}

/// Calculate sample delta to decode an ADPCM sample.
pub(crate) fn calculate_delta(step: u16, code: u8) -> i32 {
    let step = step as i32;
    let mut delta = step >> 3;
    if (code & 1) != 0 { delta += step >> 2; }
    if (code & 2) != 0 { delta += step >> 1; }
    if (code & 4) != 0 { delta += step; }
    if (code & 8) != 0 { delta = -delta; }
    delta
}
