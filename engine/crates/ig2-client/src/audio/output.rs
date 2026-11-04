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
    let mut feed = Feed { mixer: mixer.clone(), buf: Vec::new() };
    let mut feed16 = Feed { mixer: mixer.clone(), buf: Vec::new() };
    let err = |e| eprintln!("audio: {e}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| feed.write(data, out_ch, |s| s),
            err,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |data: &mut [i16], _| feed16.write(data, out_ch, |s| (s * 32767.0) as i16),
            err,
            None,
        ),
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some((Output { _stream: stream }, mixer))
}

/// The audio callback's side of the mixer: its stereo buffer is kept between callbacks, so a callback doesn't
/// allocate.
struct Feed {
    mixer: Arc<Mutex<Mixer>>,
    buf: Vec<[f32; 2]>,
}

impl Feed {
    /// Mixes `data.len() / out_ch` frames into the device's interleaved buffer: left and right, any further
    /// channels silent.
    fn write<T: Copy + Default>(&mut self, data: &mut [T], out_ch: usize, sample: impl Fn(f32) -> T) {
        self.buf.resize(data.len() / out_ch, [0.0; 2]);
        if let Ok(mut m) = self.mixer.lock() {
            m.mix(&mut self.buf);
        } else {
            self.buf.fill([0.0; 2]);
        }
        for (d, f) in data.chunks_mut(out_ch).zip(&self.buf) {
            d[0] = sample(f[0]);
            if out_ch > 1 {
                d[1] = sample(f[1]);
                d[2..].fill(T::default());
            }
        }
    }
}
