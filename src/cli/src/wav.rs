use std::path::Path;

pub struct MonoWav {
    pub sample_rate: u32,
    pub channels: usize,
    pub samples: Vec<f32>,
}

pub fn read_mono(path: &Path) -> hound::Result<MonoWav> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / max))
                .collect::<Result<_, _>>()?
        }
    };
    let samples = if channels <= 1 {
        raw
    } else {
        crate::audio::downmix(&raw, channels, |sample| sample)
    };
    Ok(MonoWav {
        sample_rate: spec.sample_rate,
        channels,
        samples,
    })
}

pub fn write_mono(
    path: impl AsRef<Path>,
    sample_rate: u32,
    samples: impl IntoIterator<Item = f32>,
) -> hound::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for sample in samples {
        writer.write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
    }
    writer.finalize()
}
