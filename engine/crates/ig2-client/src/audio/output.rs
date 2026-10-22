//! The sound card: a cpal output stream that pulls frames from the shared mixer.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::mixer::Mixer;

pub struct Output {
    _stream: cpal::Stream,
}

/// Opens the default output device; the mixer runs at the device's rate. `None` when there is no device.
pub fn open(channels: usize) -> Option<(Output, Arc<Mutex<Mixer>>)> {
    let device = cpal::default_host().default_output_device()?;
    let config = device.default_output_config().ok()?;
    let rate = config.sample_rate().0;
    let out_ch = config.channels() as usize;
    let mixer = Arc::new(Mutex::new(Mixer::new(rate, channels)));
    let m = mixer.clone();
    let mut buf: Vec<[f32; 2]> = Vec::new();
    let mut fill = move |n: usize| -> Vec<[f32; 2]> {
        buf.resize(n, [0.0; 2]);
        if let Ok(mut m) = m.lock() {
            m.mix(&mut buf);
        } else {
            buf.fill([0.0; 2]);
        }
        buf.clone()
    };
    let err = |e| eprintln!("audio: {e}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| {
                let frames = fill(data.len() / out_ch);
                for (d, f) in data.chunks_mut(out_ch).zip(&frames) {
                    d[0] = f[0];
                    if out_ch > 1 {
                        d[1] = f[1];
                        d[2..].fill(0.0);
                    }
                }
            },
            err,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |data: &mut [i16], _| {
                let frames = fill(data.len() / out_ch);
                for (d, f) in data.chunks_mut(out_ch).zip(&frames) {
                    d[0] = (f[0] * 32767.0) as i16;
                    if out_ch > 1 {
                        d[1] = (f[1] * 32767.0) as i16;
                        d[2..].fill(0);
                    }
                }
            },
            err,
            None,
        ),
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some((Output { _stream: stream }, mixer))
}
