//! Sound files (re/screens/audio.md §1, ADR 0091): every sound, voice and music track of the GOG release is Ogg
//! Vorbis, the `dataraw/` ones inside the remaster's `neGs` container (`Install::read`/`open_stream` take it off); a
//! mod's may also be FLAC or WAV. Decoded by `ig2_formats::sound`, made stereo and resampled linearly to the mixer's
//! rate.

use std::io::{Read, Seek};

use ig2_formats::sound::{Reader, MAX_SOUND_SECONDS};

use super::mixer::{Chunk, Stream};

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

/// A whole file decoded into a chunk at `rate` (cut after `MAX_SOUND_SECONDS`).
pub fn chunk(data: Vec<u8>, rate: u32) -> Result<Chunk, String> {
    let mut file = Reader::new(std::io::Cursor::new(data))?;
    let frames = file.read_all((file.rate() * MAX_SOUND_SECONDS) as usize)?;
    let mut rs = Resampler::new(file.rate(), rate);
    let mut out = Vec::with_capacity(frames.len() * rate as usize / file.rate() as usize + 1);
    for f in frames {
        rs.push(f, &mut out);
    }
    Ok(Chunk::new(out))
}

/// A music track decoded as the mixer asks for it.
pub struct FileStream {
    file: Reader,
    rs: Resampler,
    decoded: Vec<[f32; 2]>,
    pending: Vec<[f32; 2]>,
    at: usize,
    ended: bool,
}

impl FileStream {
    pub fn new(r: impl Read + Seek + Send + 'static, rate: u32) -> Result<Self, String> {
        let file = Reader::new(r)?;
        let rs = Resampler::new(file.rate(), rate);
        Ok(Self { file, rs, decoded: Vec::new(), pending: Vec::new(), at: 0, ended: false })
    }
}

impl Stream for FileStream {
    fn read(&mut self, out: &mut [[f32; 2]]) -> usize {
        let mut n = 0;
        while n < out.len() {
            if self.at == self.pending.len() {
                if self.ended {
                    break;
                }
                self.pending.clear();
                self.at = 0;
                self.decoded.clear();
                // A broken packet ends the track (a mod's file; the install's never are).
                match self.file.next(&mut self.decoded) {
                    Ok(true) => {
                        for &f in &self.decoded {
                            self.rs.push(f, &mut self.pending);
                        }
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
