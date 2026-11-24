//! Fixtures the crate's own tests share.

/// A 16-bit PCM WAV of `samples`, written by hand.
///
/// Hand-written rather than encoded by a crate: the point of the tests that
/// use it is that the decoder reads what a file actually contains, and a
/// fixture produced by another library would test that library's idea of a
/// WAV against `symphonia`'s.
pub(crate) fn wav(samples: &[i16], channels: u16, rate: u32) -> Vec<u8> {
    let bytes_per_frame = u32::from(channels) * 2;
    let data_len = samples.len() as u32 * 2;

    let mut file = Vec::new();
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(&(36 + data_len).to_le_bytes());
    file.extend_from_slice(b"WAVE");

    file.extend_from_slice(b"fmt ");
    file.extend_from_slice(&16u32.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes()); // PCM
    file.extend_from_slice(&channels.to_le_bytes());
    file.extend_from_slice(&rate.to_le_bytes());
    file.extend_from_slice(&(rate * bytes_per_frame).to_le_bytes());
    file.extend_from_slice(&(bytes_per_frame as u16).to_le_bytes());
    file.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    file.extend_from_slice(b"data");
    file.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        file.extend_from_slice(&sample.to_le_bytes());
    }
    file
}
