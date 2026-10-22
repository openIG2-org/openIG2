//! The videos' decoders (re/screens/video.md §1): H.264 through OpenH264 (built from source by `openh264-sys2`, no
//! system library) and AAC-LC through Symphonia, both fed by `ig2_formats::mp4`'s demuxer. Frames come out in
//! display order and are turned into RGBA with the remaster's own `video_fragment.shader` arithmetic (BT.601,
//! limited range, chroma sampled bilinearly as GL samples the half-size planes).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ig2_formats::mp4::{self, Codec, Mp4, Track};
use openh264::decoder::{DecodedYUV, Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions};

use crate::assets::Image;
use crate::audio::mixer::Stream;

/// One file read and demuxed.
pub struct Clip {
    pub data: Arc<Vec<u8>>,
    pub mp4: Mp4,
}

impl Clip {
    pub fn new(data: Vec<u8>) -> Result<Self, String> {
        let mp4 = mp4::parse(&data).map_err(|e| e.to_string())?;
        if mp4.video.is_none() {
            return Err("no video track".into());
        }
        Ok(Self { data: Arc::new(data), mp4 })
    }

    pub fn video(&self) -> &Track {
        self.mp4.video.as_ref().unwrap()
    }

    pub fn size(&self) -> (u32, u32) {
        match self.video().codec {
            Codec::Avc { width, height, .. } => (width as u32, height as u32),
            _ => (0, 0),
        }
    }

    /// The stream's frame rate: the timescale over the first sample's duration (`stts`).
    pub fn fps(&self) -> f32 {
        let v = self.video();
        match v.samples.get(1) {
            Some(s) if s.dts > v.samples[0].dts => v.timescale as f32 / (s.dts - v.samples[0].dts) as f32,
            _ => 25.0,
        }
    }

}

/// The video track decoded frame by frame.
pub struct Frames {
    clip: Arc<Clip>,
    dec: Decoder,
    next: usize,
    annexb: Vec<u8>,
    pending: std::collections::VecDeque<Image>,
    flushed: bool,
}

impl Frames {
    pub fn new(clip: Arc<Clip>) -> Result<Self, String> {
        let cfg = DecoderConfig::new().flush_after_decode(Flush::NoFlush);
        let dec = Decoder::with_api_config(openh264::OpenH264API::from_source(), cfg).map_err(|e| e.to_string())?;
        let Codec::Avc { sps, pps, .. } = &clip.video().codec else { return Err("not AVC".into()) };
        let annexb = mp4::avc_parameter_sets(sps, pps);
        Ok(Self { clip, dec, next: 0, annexb, pending: Default::default(), flushed: false })
    }

    /// The next frame in display order; `None` at the end.
    pub fn next_frame(&mut self) -> Option<Image> {
        loop {
            if let Some(f) = self.pending.pop_front() {
                return Some(f);
            }
            let v = self.clip.video();
            let Codec::Avc { nal_length_size, .. } = v.codec else { return None };
            if let Some(s) = v.samples.get(self.next) {
                self.next += 1;
                let Ok(bytes) = v.data(&self.clip.data, s) else { continue };
                if mp4::avc_to_annexb(bytes, nal_length_size, &mut self.annexb).is_err() {
                    self.annexb.clear();
                    continue;
                }
                // A damaged frame is skipped (the decoder conceals it in the next reference).
                if let Ok(Some(y)) = self.dec.decode(&self.annexb) {
                    self.pending.push_back(rgba(&y));
                }
                self.annexb.clear();
            } else if !self.flushed {
                self.flushed = true;
                for y in self.dec.flush_remaining().unwrap_or_default() {
                    self.pending.push_back(rgba(&y));
                }
            } else {
                return None;
            }
        }
    }
}

/// `video_fragment.shader`: `Y' = 1.1643·(Y − 1/16)`, `R = Y' + 1.596·Cr`, `G = Y' − 0.391·Cb − 0.813·Cr`,
/// `B = Y' + 2.018·Cb` (the shader's `video_u_texture` carries Cr, `video_v_texture` Cb), clamped by the frame
/// buffer. The half-size chroma planes are sampled bilinearly at GL's texel centres.
pub fn rgba(y: &DecodedYUV) -> Image {
    let (w, h) = y.dimensions();
    let (ys, us, vs) = y.strides();
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let (yp, up, vp) = (y.y(), y.u(), y.v());
    let mut out = vec![255u8; w * h * 4];
    // Chroma sample positions per luma column/row: (x + 0.5)/2 − 0.5, clamped to the plane (GL's CLAMP_TO_EDGE).
    let taps = |n: usize, cn: usize| -> Vec<(usize, usize, f32)> {
        (0..n)
            .map(|x| {
                let c = ((x as f32 + 0.5) * 0.5 - 0.5).max(0.0);
                let i = (c as usize).min(cn - 1);
                (i, (i + 1).min(cn - 1), c - i as f32)
            })
            .collect()
    };
    let (tx, ty) = (taps(w, cw), taps(h, ch));
    for (row, &(r0, r1, fy)) in ty.iter().enumerate() {
        for (col, &(c0, c1, fx)) in tx.iter().enumerate() {
            let bil = |p: &[u8], s: usize| {
                let a = p[r0 * s + c0] as f32 + (p[r0 * s + c1] as f32 - p[r0 * s + c0] as f32) * fx;
                let b = p[r1 * s + c0] as f32 + (p[r1 * s + c1] as f32 - p[r1 * s + c0] as f32) * fx;
                (a + (b - a) * fy) / 255.0
            };
            let o = (row * w + col) * 4;
            out[o..o + 3].copy_from_slice(&shade(yp[row * ys + col] as f32 / 255.0, bil(up, us), bil(vp, vs)));
        }
    }
    Image { width: w as u32, height: h as u32, rgba: out }
}

/// One pixel of `video_fragment.shader` from the sampled planes (0..=1 each).
fn shade(y: f32, cb: f32, cr: f32) -> [u8; 3] {
    let l = 1.1643 * (y - 0.0625);
    let (cb, cr) = (cb - 0.5, cr - 0.5);
    [l + 1.596 * cr, l - 0.391 * cb - 0.813 * cr, l + 2.018 * cb].map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
}

/// `soundhook` exe 0x7100e0: the first 0xe5b frames of a play at 44.1 kHz are silence.
pub const LEAD_IN: u64 = 0xe5b;

/// The video's sound as a mixer stream: AAC packets decoded as the mixer asks, made stereo and resampled, after
/// [`LEAD_IN`]'s silence. `played` counts the frames handed out at the mixer's rate.
pub struct Sound {
    clip: Arc<Clip>,
    dec: Box<dyn AudioDecoder>,
    next: usize,
    rs: crate::audio::decode::Resampler,
    pending: Vec<[f32; 2]>,
    at: usize,
    buf: Vec<f32>,
    /// Silent frames left before the sound.
    lead: u64,
    pub played: Arc<AtomicU64>,
}

impl Sound {
    /// `None` when the clip has no sound (the race-choice loops).
    pub fn new(clip: Arc<Clip>, rate: u32) -> Option<Self> {
        let a = clip.mp4.audio.as_ref()?;
        let Codec::Aac { sample_rate, config, .. } = &a.codec else { return None };
        let mut p = AudioCodecParameters::new();
        p.for_codec(symphonia::core::codecs::audio::well_known::CODEC_ID_AAC)
            .with_sample_rate(*sample_rate)
            .with_extra_data(config.clone().into_boxed_slice());
        let dec = symphonia::default::codecs::AacDecoder::try_new(&p, &AudioDecoderOptions::default()).ok()?;
        let rs = crate::audio::decode::Resampler::new(*sample_rate, rate);
        Some(Self { clip, dec: Box::new(dec), next: 0, rs, pending: Vec::new(), at: 0, buf: Vec::new(), lead: LEAD_IN * rate as u64 / 44100, played: Arc::new(AtomicU64::new(0)) })
    }

    /// Decodes the next packet into `pending`; false at the end.
    fn refill(&mut self) -> bool {
        use symphonia::core::packet::Packet;
        use symphonia::core::units::{Duration, Timestamp};
        let a = self.clip.mp4.audio.as_ref().unwrap();
        while let Some(s) = a.samples.get(self.next) {
            self.next += 1;
            let Ok(bytes) = a.data(&self.clip.data, s) else { continue };
            let pkt = Packet::new(0, Timestamp::from(s.dts as i64), Duration::from(1024u64), bytes);
            let Ok(b) = self.dec.decode(&pkt) else { continue };
            let channels = b.spec().channels().count().max(1);
            self.buf.clear();
            b.copy_to_vec_interleaved(&mut self.buf);
            self.pending.clear();
            self.at = 0;
            for f in self.buf.chunks(channels) {
                let l = f[0];
                let r = f.get(1).copied().unwrap_or(l);
                self.rs.push([l, r], &mut self.pending);
            }
            return true;
        }
        false
    }
}

impl Stream for Sound {
    fn read(&mut self, out: &mut [[f32; 2]]) -> usize {
        let mut n = (self.lead as usize).min(out.len());
        out[..n].fill([0.0; 2]);
        self.lead -= n as u64;
        while n < out.len() {
            if self.at == self.pending.len() && !self.refill() {
                break;
            }
            let k = (out.len() - n).min(self.pending.len() - self.at);
            out[n..n + k].copy_from_slice(&self.pending[self.at..self.at + k]);
            n += k;
            self.at += k;
        }
        self.played.fetch_add(n as u64, Ordering::Relaxed);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(path: &str) -> Option<Arc<Clip>> {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return None };
        Some(Arc::new(Clip::new(install.read(path).unwrap()).unwrap()))
    }

    /// Every sample of a short clip with B-frames comes out as a frame, in display order at 25 fps.
    #[test]
    fn decodes_every_frame() {
        let Some(c) = clip("anim/mpeg4/misc/end_x.mp4") else { return };
        let n = c.video().samples.len();
        assert_eq!(c.fps(), 25.0);
        let mut f = Frames::new(c.clone()).unwrap();
        let mut got = 0;
        while let Some(img) = f.next_frame() {
            assert_eq!((img.width, img.height), c.size());
            got += 1;
        }
        assert_eq!(got, n);
    }

    /// The sound decodes to the AAC track's length at the mixer's rate.
    #[test]
    fn sound_length() {
        let Some(c) = clip("anim/mpeg4/arriving.mp4") else { return };
        let a = c.mp4.audio.as_ref().unwrap();
        let mut s = Sound::new(c.clone(), 48000).unwrap();
        let mut buf = vec![[0.0f32; 2]; 4096];
        while s.read(&mut buf) == buf.len() {}
        let secs = s.played.load(Ordering::Relaxed) as f64 / 48000.0;
        let want = (a.samples.len() as f64 * 1024.0 + LEAD_IN as f64) / 44100.0;
        assert!((secs - want).abs() < 0.01, "{secs} vs {want}");
    }

    /// The remaster's shader arithmetic: 8-bit 128 is 0.502 in GL, so grey carries a faint magenta (R and B +1/255, white G 254).
    #[test]
    fn yuv_to_rgb() {
        let px = |y: f32, cb: f32, cr: f32| shade(y / 255.0, cb / 255.0, cr / 255.0);
        assert_eq!(px(16.0, 128.0, 128.0), [1, 0, 1]);
        assert_eq!(px(235.0, 128.0, 128.0), [255, 254, 255]);
        assert_eq!(px(81.0, 90.0, 240.0), [255, 0, 0]);
    }
}
