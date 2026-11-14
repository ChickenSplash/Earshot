//! WAV loading for the examples and tests (not part of the library).

/// Mono samples (channels averaged) and the sample rate.
pub fn load(path: &str) -> (Vec<f32>, f32) {
    let mut reader =
        hound::WavReader::open(path).unwrap_or_else(|e| panic!("can't open {path}: {e}"));
    let spec = reader.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let scale = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / scale)
                .collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
    };
    let channels = spec.channels as usize;
    let mono = raw
        .chunks(channels)
        .map(|f| f.iter().sum::<f32>() / channels as f32)
        .collect();
    (mono, spec.sample_rate as f32)
}
