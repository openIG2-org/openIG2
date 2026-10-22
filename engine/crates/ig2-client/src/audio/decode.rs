//! Ogg Vorbis from the user's install (re/screens/audio.md §1): every sound, voice and music track of the GOG release
//! is Ogg Vorbis, the `dataraw/` ones inside the remaster's `neGs` container (`Install::read`/`open_stream` take it
//! off). Decoded with `lewton`, made stereo and resampled linearly to the mixer's rate.

use std::io::{Read, Seek};

use lewton::inside_ogg::OggStreamReader;

use super::mixer::{Chunk, Stream};

fn frames_of(packet: &[Vec<i16>]) -> impl Iterator<Item = [f32; 2]> + '_ {
    let n = packet.first().map_or(0, Vec::len);
    (0..n).map(move |i| {
        let l = packet[0][i] as f32 / 32768.0;
        let r = packet.get(1).map_or(l, |c| c[i] as f32 / 32768.0);
        [l, r]
    })
}

/// Linear resampler from `from` Hz to `to` Hz over a stream of frames.
#[derive(Debug, Clone)]
pub struct Resampler {
    step: f64,
    /// Position of the next output frame, in input frames past `prev`.
    pos: f64,
    prev: [f32; 2],
}

impl Resampler {
    pub fn new(from: u32, to: u32) -> Self {
        Self { step: from as f64 / to as f64, pos: 0.0, prev: [0.0; 2] }
    }

    /// Feeds one input frame; pushes the output frames that fall between the previous one and this.
    pub fn push(&mut self, f: [f32; 2], out: &mut Vec<[f32; 2]>) {
        while self.pos < 1.0 {
            let t = self.pos as f32;
            out.push([self.prev[0] + (f[0] - self.prev[0]) * t, self.prev[1] + (f[1] - self.prev[1]) * t]);
            self.pos += self.step;
        }
        self.pos -= 1.0;
        self.prev = f;
    }
}

/// A whole file decoded into a chunk at `rate`.
pub fn chunk(data: Vec<u8>, rate: u32) -> Result<Chunk, String> {
    let mut ogg = OggStreamReader::new(std::io::Cursor::new(data)).map_err(|e| e.to_string())?;
    let mut rs = Resampler::new(ogg.ident_hdr.audio_sample_rate, rate);
    let mut out = Vec::new();
    while let Some(p) = ogg.read_dec_packet().map_err(|e| e.to_string())? {
        for f in frames_of(&p) {
            rs.push(f, &mut out);
        }
    }
    Ok(Chunk::new(out))
}

/// A music track decoded as the mixer asks for it.
pub struct OggStream<R: Read + Seek> {
    ogg: OggStreamReader<R>,
    rs: Resampler,
    pending: Vec<[f32; 2]>,
    at: usize,
    ended: bool,
}

impl<R: Read + Seek> OggStream<R> {
    pub fn new(r: R, rate: u32) -> Result<Self, String> {
        let ogg = OggStreamReader::new(r).map_err(|e| e.to_string())?;
        let rs = Resampler::new(ogg.ident_hdr.audio_sample_rate, rate);
        Ok(Self { ogg, rs, pending: Vec::new(), at: 0, ended: false })
    }
}

impl<R: Read + Seek + Send> Stream for OggStream<R> {
    fn read(&mut self, out: &mut [[f32; 2]]) -> usize {
        let mut n = 0;
        while n < out.len() {
            if self.at == self.pending.len() {
                if self.ended {
                    break;
                }
                self.pending.clear();
                self.at = 0;
                match self.ogg.read_dec_packet() {
                    Ok(Some(p)) => {
                        let mut buf = std::mem::take(&mut self.pending);
                        for f in frames_of(&p) {
                            self.rs.push(f, &mut buf);
                        }
                        self.pending = buf;
                    }
                    _ => self.ended = true,
                }
                continue;
            }
            let k = (out.len() - n).min(self.pending.len() - self.at);
            out[n..n + k].copy_from_slice(&self.pending[self.at..self.at + k]);
            n += k;
            self.at += k;
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_keeps_the_duration() {
        let mut rs = Resampler::new(22050, 44100);
        let mut out = Vec::new();
        for i in 0..100 {
            rs.push([i as f32, 0.0], &mut out);
        }
        assert_eq!(out.len(), 200);
        assert_eq!(out[3][0], 0.5);
        let mut rs = Resampler::new(48000, 44100);
        let mut out = Vec::new();
        for _ in 0..48000 {
            rs.push([0.0; 2], &mut out);
        }
        assert!((out.len() as i64 - 44100).abs() <= 1);
    }
}
