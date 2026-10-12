use anyhow::{anyhow, Context, Result};
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub fn decode_file(path: &Path, target_rate: u32) -> Result<Vec<f32>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open sound file {}", path.display()))?;
    let label = path.display().to_string();
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .with_context(|| format!("unsupported audio container: {label}"))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| anyhow!("no default audio track in {label}"))?;
    let track_id = track.id;
    let src_rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("no decoder available for track")?;
    let mut mono = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => append_mono(&buf, &mut mono),
            Err(SymError::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }
    if mono.is_empty() {
        return Err(anyhow!("no decodable samples in {label}"));
    }
    Ok(resample(&mono, src_rate, target_rate))
}

fn append_mono(buf: &AudioBufferRef, out: &mut Vec<f32>) {
    let spec = *buf.spec();
    let channels = spec.channels.count().max(1);
    let mut interleaved = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
    interleaved.copy_interleaved_ref(buf.clone());
    out.extend(
        interleaved
            .samples()
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32),
    );
}

pub fn resample(src: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || src.is_empty() {
        return src.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let len = (src.len() as f64 / ratio) as usize;
    let last = src.len() - 1;
    (0..len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos.floor() as usize;
            let frac = (pos - idx as f64) as f32;
            let a = src[idx.min(last)];
            let b = src[(idx + 1).min(last)];
            a + (b - a) * frac
        })
        .collect()
}
