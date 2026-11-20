//! Sound and music files (ADR 0091): the install's Ogg Vorbis (`lewton`, as the client always decoded it) and a mod's
//! FLAC or WAV (`symphonia`). The bytes decide the decoder, never the file's extension. Packets come out as stereo
//! frames at the file's own rate (a mono file on both sides); the client resamples them to the mixer's rate.

use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;

use lewton::inside_ogg::OggStreamReader;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

use crate::vfs::ReadSeek;

/// The file kinds a sound or music track may be, in the order one mod's copies of the same path are preferred.
pub const EXTENSIONS: [&str; 3] = ["ogg", "flac", "wav"];
/// Sample rates a file may have (Hz).
pub const RATES: std::ops::RangeInclusive<u32> = 8000..=192_000;
/// A sound decoded whole (effects, voices, speech) is cut after this many seconds; music is streamed and has no limit.
pub const MAX_SOUND_SECONDS: u32 = 120;

/// What a file's first bytes say it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Ogg,
    Flac,
    Wav,
}

impl Kind {
    pub fn sniff(head: &[u8]) -> Option<Self> {
        match head {
            [b'O', b'g', b'g', b'S', ..] => Some(Self::Ogg),
            [b'f', b'L', b'a', b'C', ..] => Some(Self::Flac),
            [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'A', b'V', b'E', ..] => Some(Self::Wav),
            _ => None,
        }
    }
}

/// A path's stem when it names a sound file (`.ogg`, `.flac`, `.wav`, or the 1999 names' `.mp2`), else `None`.
pub fn stem(path: &str) -> Option<&str> {
    let (stem, ext) = path.rsplit_once('.')?;
    (EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) || ext.eq_ignore_ascii_case("mp2")).then_some(stem)
}

/// symphonia wants a `Sync` source; the reader is only ever used through `&mut`.
struct Source(Mutex<Box<dyn ReadSeek>>);

impl Read for Source {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.get_mut().unwrap_or_else(|e| e.into_inner()).read(buf)
    }
}

impl Seek for Source {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.get_mut().unwrap_or_else(|e| e.into_inner()).seek(pos)
    }
}

impl MediaSource for Source {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

enum Inner {
    Ogg(Box<OggStreamReader<Box<dyn ReadSeek>>>),
    Other { format: Box<dyn FormatReader>, dec: Box<dyn AudioDecoder>, track: u32, buf: Vec<f32> },
}

/// A sound file being decoded, packet by packet.
pub struct Reader {
    inner: Inner,
    rate: u32,
    channels: usize,
    pub kind: Kind,
}

impl Reader {
    /// Opens `r` (positioned at the file's start). Refuses what isn't Ogg Vorbis, FLAC or PCM WAV, more than two
    /// channels, and a rate outside [`RATES`].
    pub fn new(r: impl Read + Seek + Send + 'static) -> Result<Self, String> {
        let mut r: Box<dyn ReadSeek> = Box::new(r);
        let mut head = [0u8; 12];
        let mut n = 0;
        while n < head.len() {
            match r.read(&mut head[n..]).map_err(|e| e.to_string())? {
                0 => break,
                k => n += k,
            }
        }
        r.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let kind = Kind::sniff(&head[..n]).ok_or("not an Ogg Vorbis, FLAC or WAV file")?;
        let (inner, rate, channels) = match kind {
            Kind::Ogg => {
                let ogg = OggStreamReader::new(r).map_err(|e| e.to_string())?;
                let (rate, ch) = (ogg.ident_hdr.audio_sample_rate, ogg.ident_hdr.audio_channels as usize);
                (Inner::Ogg(Box::new(ogg)), rate, ch)
            }
            Kind::Flac | Kind::Wav => {
                let mss = MediaSourceStream::new(Box::new(Source(Mutex::new(r))), Default::default());
                let mut hint = Hint::new();
                hint.with_extension(if kind == Kind::Flac { "flac" } else { "wav" });
                let format = symphonia::default::get_probe()
                    .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
                    .map_err(|e| e.to_string())?;
                let track = format.default_track(TrackType::Audio).ok_or("no audio track")?;
                let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or("no audio parameters")?.clone();
                let rate = params.sample_rate.ok_or("no sample rate")?;
                let ch = params.channels.as_ref().map_or(0, |c| c.count());
                let id = track.id;
                let dec = symphonia::default::get_codecs()
                    .make_audio_decoder(&params, &AudioDecoderOptions::default())
                    .map_err(|e| e.to_string())?;
                (Inner::Other { format, dec, track: id, buf: Vec::new() }, rate, ch)
            }
        };
        if !(1..=2).contains(&channels) {
            return Err(format!("{channels} channels (1 or 2 allowed)"));
        }
        if !RATES.contains(&rate) {
            return Err(format!("{rate} Hz (8000 to 192000 allowed)"));
        }
        Ok(Self { inner, rate, channels, kind })
    }

    /// The file's sample rate (Hz).
    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Appends the next packet's frames to `out`; `Ok(false)` at the end.
    pub fn next(&mut self, out: &mut Vec<[f32; 2]>) -> Result<bool, String> {
        match &mut self.inner {
            Inner::Ogg(ogg) => {
                let Some(p) = ogg.read_dec_packet().map_err(|e| e.to_string())? else { return Ok(false) };
                let n = p.first().map_or(0, Vec::len);
                out.extend((0..n).map(|i| {
                    let l = p[0][i] as f32 / 32768.0;
                    let r = p.get(1).map_or(l, |c| c[i] as f32 / 32768.0);
                    [l, r]
                }));
                Ok(true)
            }
            Inner::Other { format, dec, track, buf } => loop {
                let Some(pkt) = format.next_packet().map_err(|e| e.to_string())? else { return Ok(false) };
                if pkt.track_id != *track {
                    continue;
                }
                let b = dec.decode(&pkt).map_err(|e| e.to_string())?;
                let ch = b.spec().channels().count().max(1);
                buf.clear();
                b.copy_to_vec_interleaved(buf);
                out.extend(buf.chunks_exact(ch).map(|f| [f[0], f.get(1).copied().unwrap_or(f[0])]));
                return Ok(true);
            },
        }
    }

    /// Decodes the rest of the file (at most `max_frames` frames; the rest is ignored).
    pub fn read_all(&mut self, max_frames: usize) -> Result<Vec<[f32; 2]>, String> {
        let mut out = Vec::new();
        while out.len() < max_frames && self.next(&mut out)? {}
        out.truncate(max_frames);
        Ok(out)
    }
}

/// Test files made in memory (no asset is committed): PCM WAV and verbatim-coded FLAC.
#[cfg(test)]
pub(crate) mod make {
    /// A 16-bit PCM WAV of interleaved `samples`.
    pub fn wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data = samples.len() as u32 * 2;
        let mut v = Vec::new();
        v.extend(b"RIFF");
        v.extend((36 + data).to_le_bytes());
        v.extend(b"WAVEfmt ");
        v.extend(16u32.to_le_bytes());
        v.extend(1u16.to_le_bytes());
        v.extend(channels.to_le_bytes());
        v.extend(rate.to_le_bytes());
        v.extend((rate * channels as u32 * 2).to_le_bytes());
        v.extend((channels * 2).to_le_bytes());
        v.extend(16u16.to_le_bytes());
        v.extend(b"data");
        v.extend(data.to_le_bytes());
        for s in samples {
            v.extend(s.to_le_bytes());
        }
        v
    }

    struct Bits {
        v: Vec<u8>,
        acc: u64,
        n: u32,
    }

    impl Bits {
        fn put(&mut self, x: u64, bits: u32) {
            for i in (0..bits).rev() {
                self.acc = self.acc << 1 | (x >> i & 1);
                self.n += 1;
                if self.n == 8 {
                    self.v.push(self.acc as u8);
                    self.acc = 0;
                    self.n = 0;
                }
            }
        }

        fn align(&mut self) {
            while self.n != 0 {
                self.put(0, 1);
            }
        }
    }

    fn crc8(d: &[u8]) -> u8 {
        d.iter().fold(0u8, |mut c, &b| {
            c ^= b;
            for _ in 0..8 {
                c = if c & 0x80 != 0 { c << 1 ^ 0x07 } else { c << 1 };
            }
            c
        })
    }

    fn crc16(d: &[u8]) -> u16 {
        d.iter().fold(0u16, |mut c, &b| {
            c ^= (b as u16) << 8;
            for _ in 0..8 {
                c = if c & 0x8000 != 0 { c << 1 ^ 0x8005 } else { c << 1 };
            }
            c
        })
    }

    /// A 16-bit FLAC of interleaved `samples`, one verbatim frame of up to 4096 frames per block.
    pub fn flac(rate: u32, channels: u8, samples: &[i16]) -> Vec<u8> {
        let ch = channels as usize;
        let total = samples.len() / ch;
        let mut b = Bits { v: b"fLaC".to_vec(), acc: 0, n: 0 };
        b.put(1, 1);
        b.put(0, 7);
        b.put(34, 24);
        b.put(4096, 16);
        b.put(4096, 16);
        b.put(0, 24);
        b.put(0, 24);
        b.put(rate as u64, 20);
        b.put(ch as u64 - 1, 3);
        b.put(15, 5);
        b.put(total as u64, 36);
        b.put(0, 64);
        b.put(0, 64);
        for (k, block) in samples.chunks(4096 * ch).enumerate() {
            let start = b.v.len();
            let frames = block.len() / ch;
            b.put(0b11111111111110, 14);
            b.put(0, 2);
            b.put(7, 4); // 16-bit block size after the header
            b.put(0, 4); // rate from STREAMINFO
            b.put(ch as u64 - 1, 4); // independent channels
            b.put(0b100, 3); // 16 bits per sample
            b.put(0, 1);
            assert!(k < 128);
            b.put(k as u64, 8);
            b.put(frames as u64 - 1, 16);
            let crc = crc8(&b.v[start..]);
            b.put(crc as u64, 8);
            for c in 0..ch {
                b.put(0, 1);
                b.put(1, 6); // verbatim
                b.put(0, 1);
                for f in 0..frames {
                    b.put(block[f * ch + c] as u16 as u64, 16);
                }
            }
            b.align();
            let crc = crc16(&b.v[start..]);
            b.put(crc as u64, 16);
        }
        b.v
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn tone(frames: usize, channels: usize) -> Vec<i16> {
        (0..frames).flat_map(|i| (0..channels).map(move |c| ((i as f32 * 0.05 + c as f32).sin() * 12000.0) as i16)).collect()
    }

    fn decode(bytes: Vec<u8>) -> (Reader, Vec<[f32; 2]>) {
        let mut r = Reader::new(Cursor::new(bytes)).unwrap();
        let v = r.read_all(usize::MAX).unwrap();
        (r, v)
    }

    #[test]
    fn wav_decodes_exactly() {
        let s = tone(5000, 2);
        let (r, v) = decode(make::wav(22050, 2, &s));
        assert_eq!((r.kind, r.rate(), r.channels(), v.len()), (Kind::Wav, 22050, 2, 5000));
        assert_eq!(v[123], [s[246] as f32 / 32768.0, s[247] as f32 / 32768.0]);
        let s = tone(300, 1);
        let (_, v) = decode(make::wav(8000, 1, &s));
        assert_eq!(v[7], [s[7] as f32 / 32768.0; 2]);
    }

    #[test]
    fn flac_decodes_exactly() {
        let s = tone(10_000, 2);
        let (r, v) = decode(make::flac(48000, 2, &s));
        assert_eq!((r.kind, r.rate(), r.channels(), v.len()), (Kind::Flac, 48000, 2, 10_000));
        assert_eq!(v[9999], [s[19998] as f32 / 32768.0, s[19999] as f32 / 32768.0]);
        let s = tone(700, 1);
        let (_, v) = decode(make::flac(44100, 1, &s));
        assert_eq!((v.len(), v[500]), (700, [s[500] as f32 / 32768.0; 2]));
    }

    #[test]
    fn the_bytes_decide_not_the_name() {
        assert_eq!(Kind::sniff(b"OggS\0\0"), Some(Kind::Ogg));
        assert_eq!(Kind::sniff(b"RIFF\0\0\0\0WAVEfmt "), Some(Kind::Wav));
        assert_eq!(Kind::sniff(b"RIFF\0\0\0\0AVI "), None);
        assert_eq!(stem("sounds/a/b.WAV"), Some("sounds/a/b"));
        assert_eq!(stem("mfx/x.mp2"), Some("mfx/x"));
        assert_eq!(stem("music/x.mp3"), None);
    }

    #[test]
    fn malformed_files_are_errors_not_panics() {
        let wav = make::wav(22050, 1, &tone(1000, 1));
        let flac = make::flac(22050, 1, &tone(1000, 1));
        for bad in [b"".to_vec(), b"OggS".to_vec(), b"fLaC\x80\0\0\x05zz".to_vec(), wav[..20].to_vec(), flac[..50].to_vec(), b"ID3\x04garbage".to_vec()] {
            if let Ok(mut r) = Reader::new(Cursor::new(bad)) {
                let _ = r.read_all(1 << 20);
            }
        }
        // Truncated data decodes what is there or fails, never panics; a cut-off header refuses.
        assert!(Reader::new(Cursor::new(wav[..20].to_vec())).is_err());
        assert!(Reader::new(Cursor::new(make::wav(4000, 1, &[0; 10]))).err().unwrap().contains("Hz"));
        assert!(Reader::new(Cursor::new(make::wav(22050, 3, &[0; 30]))).err().unwrap().contains("channels"));
    }

    #[test]
    fn read_all_stops_at_the_limit() {
        let mut r = Reader::new(Cursor::new(make::wav(8000, 1, &tone(9000, 1)))).unwrap();
        assert_eq!(r.read_all(8000).unwrap().len(), 8000);
    }
}
