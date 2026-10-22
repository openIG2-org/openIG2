//! The remaster's videos: ISO-BMFF (`.mp4`) files inside the `neGs` container, one H.264 track and (except
//! the race-choice loops) one AAC track.
//!
//! Spec: `re/screens/video.md` §1. This is a minimal demuxer: it reads the `moov` sample tables into a flat
//! list of samples per track (byte range, decode time, composition offset, sync flag) and the codec
//! configuration (`avcC`, the AAC `AudioSpecificConfig`). It does not decode; `ig2-client::video` does.

use crate::{format_err, Result};

/// One demuxed file.
#[derive(Debug, Clone, Default)]
pub struct Mp4 {
    pub video: Option<Track>,
    pub audio: Option<Track>,
}

#[derive(Debug, Clone)]
pub struct Track {
    /// `mdhd`'s units per second.
    pub timescale: u32,
    /// `mdhd`'s duration in `timescale` units.
    pub duration: u64,
    pub codec: Codec,
    pub samples: Vec<Sample>,
}

#[derive(Debug, Clone)]
pub enum Codec {
    /// `avc1` with its `avcC` record.
    Avc { width: u16, height: u16, nal_length_size: u8, sps: Vec<Vec<u8>>, pps: Vec<Vec<u8>> },
    /// `mp4a` with its `esds` decoder-specific info (the `AudioSpecificConfig`).
    Aac { sample_rate: u32, channels: u16, config: Vec<u8> },
    Other([u8; 4]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    /// Byte offset of the sample in the (decrypted) file.
    pub offset: u64,
    pub size: u32,
    /// Decode time in the track's timescale.
    pub dts: u64,
    /// Composition offset (`ctts`): presentation time = `dts + cts`.
    pub cts: i32,
    /// Key frame (`stss`; every sample when the table is absent).
    pub sync: bool,
}

impl Track {
    /// The sample's bytes in `file`.
    pub fn data<'a>(&self, file: &'a [u8], s: &Sample) -> Result<&'a [u8]> {
        let start = s.offset as usize;
        file.get(start..start + s.size as usize).ok_or_else(|| format_err("mp4", "sample outside the file"))
    }
}

/// Converts one AVC sample (length-prefixed NAL units) to an Annex B byte stream, appending to `out`.
pub fn avc_to_annexb(sample: &[u8], nal_length_size: u8, out: &mut Vec<u8>) -> Result<()> {
    let n = nal_length_size as usize;
    let mut p = sample;
    while !p.is_empty() {
        if p.len() < n {
            return Err(format_err("mp4", "truncated NAL length"));
        }
        let len = p[..n].iter().fold(0usize, |a, &b| a << 8 | b as usize);
        let nal = p.get(n..n + len).ok_or_else(|| format_err("mp4", "truncated NAL unit"))?;
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(nal);
        p = &p[n + len..];
    }
    Ok(())
}

/// The parameter sets of an `avcC` as Annex B (sent once before the first frame).
pub fn avc_parameter_sets(sps: &[Vec<u8>], pps: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for nal in sps.iter().chain(pps) {
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(nal);
    }
    out
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, p: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let s = self.b.get(self.p..self.p + n).ok_or_else(|| format_err("mp4", "truncated box"))?;
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn rest(&self) -> &'a [u8] {
        &self.b[self.p.min(self.b.len())..]
    }
}

/// Iterates the boxes in `b`: (type, body).
fn boxes(b: &[u8]) -> impl Iterator<Item = Result<([u8; 4], &[u8])>> + '_ {
    let mut p = 0usize;
    std::iter::from_fn(move || {
        if p + 8 > b.len() {
            return None;
        }
        let size = u32::from_be_bytes(b[p..p + 4].try_into().unwrap()) as u64;
        let kind: [u8; 4] = b[p + 4..p + 8].try_into().unwrap();
        let (header, size) = match size {
            1 if p + 16 <= b.len() => (16, u64::from_be_bytes(b[p + 8..p + 16].try_into().unwrap())),
            0 => (8, (b.len() - p) as u64),
            s => (8, s),
        };
        let end = p as u64 + size;
        if size < header as u64 || end > b.len() as u64 {
            p = b.len();
            return Some(Err(format_err("mp4", format!("bad box size {size}"))));
        }
        let body = &b[p + header..end as usize];
        p = end as usize;
        Some(Ok((kind, body)))
    })
}

fn child<'a>(b: &'a [u8], kind: &[u8; 4]) -> Result<Option<&'a [u8]>> {
    for bx in boxes(b) {
        let (k, body) = bx?;
        if &k == kind {
            return Ok(Some(body));
        }
    }
    Ok(None)
}

/// Parses `file` (already decrypted).
pub fn parse(file: &[u8]) -> Result<Mp4> {
    let moov = child(file, b"moov")?.ok_or_else(|| format_err("mp4", "no moov box"))?;
    let mut out = Mp4::default();
    for bx in boxes(moov) {
        let (k, trak) = bx?;
        if &k != b"trak" {
            continue;
        }
        let track = parse_trak(trak)?;
        let slot = match track.codec {
            Codec::Avc { .. } => &mut out.video,
            Codec::Aac { .. } => &mut out.audio,
            Codec::Other(_) => continue,
        };
        slot.get_or_insert(track);
    }
    Ok(out)
}

fn parse_trak(trak: &[u8]) -> Result<Track> {
    fn need<'a>(b: Option<&'a [u8]>, what: &str) -> Result<&'a [u8]> {
        b.ok_or_else(|| format_err("mp4", format!("no {what} box")))
    }
    let mdia = need(child(trak, b"mdia")?, "mdia")?;
    let mut r = Reader::new(need(child(mdia, b"mdhd")?, "mdhd")?);
    let version = r.u8()?;
    r.take(3)?;
    let (timescale, duration) = if version == 1 {
        r.take(16)?;
        (r.u32()?, r.u64()?)
    } else {
        r.take(8)?;
        (r.u32()?, r.u32()? as u64)
    };
    let minf = need(child(mdia, b"minf")?, "minf")?;
    let stbl = need(child(minf, b"stbl")?, "stbl")?;
    let codec = parse_stsd(need(child(stbl, b"stsd")?, "stsd")?)?;

    // Sample sizes.
    let mut r = Reader::new(need(child(stbl, b"stsz")?, "stsz")?);
    r.take(4)?;
    let fixed = r.u32()?;
    let count = r.u32()? as usize;
    let sizes: Vec<u32> = if fixed != 0 { vec![fixed; count] } else { (0..count).map(|_| r.u32()).collect::<Result<_>>()? };

    // Chunk offsets and the sample-to-chunk runs → each sample's offset.
    let offsets: Vec<u64> = if let Some(b) = child(stbl, b"stco")? {
        let mut r = Reader::new(b);
        r.take(4)?;
        let n = r.u32()?;
        (0..n).map(|_| r.u32().map(u64::from)).collect::<Result<_>>()?
    } else {
        let mut r = Reader::new(need(child(stbl, b"co64")?, "stco")?);
        r.take(4)?;
        let n = r.u32()?;
        (0..n).map(|_| r.u64()).collect::<Result<_>>()?
    };
    let mut r = Reader::new(need(child(stbl, b"stsc")?, "stsc")?);
    r.take(4)?;
    let runs: Vec<(u32, u32)> = (0..r.u32()?)
        .map(|_| {
            let first = r.u32()?;
            let per = r.u32()?;
            r.u32()?;
            Ok((first, per))
        })
        .collect::<Result<_>>()?;
    let mut samples = Vec::with_capacity(count);
    for (ci, &chunk) in offsets.iter().enumerate() {
        let c = ci as u32 + 1;
        let per = runs.iter().rev().find(|(first, _)| *first <= c).map_or(0, |r| r.1);
        let mut at = chunk;
        for _ in 0..per {
            let Some(&size) = sizes.get(samples.len()) else { break };
            samples.push(Sample { offset: at, size, dts: 0, cts: 0, sync: true });
            at += size as u64;
        }
    }
    if samples.len() != count {
        return Err(format_err("mp4", format!("{} of {count} samples placed", samples.len())));
    }

    // Decode times, composition offsets, key frames.
    let mut r = Reader::new(need(child(stbl, b"stts")?, "stts")?);
    r.take(4)?;
    let mut i = 0;
    let mut t = 0u64;
    for _ in 0..r.u32()? {
        let n = r.u32()?;
        let delta = r.u32()? as u64;
        for _ in 0..n {
            if let Some(s) = samples.get_mut(i) {
                s.dts = t;
            }
            t += delta;
            i += 1;
        }
    }
    if let Some(b) = child(stbl, b"ctts")? {
        let mut r = Reader::new(b);
        r.take(4)?;
        let mut i = 0;
        for _ in 0..r.u32()? {
            let n = r.u32()?;
            let off = r.u32()? as i32;
            for _ in 0..n {
                if let Some(s) = samples.get_mut(i) {
                    s.cts = off;
                }
                i += 1;
            }
        }
    }
    if let Some(b) = child(stbl, b"stss")? {
        let mut r = Reader::new(b);
        r.take(4)?;
        samples.iter_mut().for_each(|s| s.sync = false);
        for _ in 0..r.u32()? {
            if let Some(s) = samples.get_mut(r.u32()? as usize - 1) {
                s.sync = true;
            }
        }
    }
    Ok(Track { timescale, duration, codec, samples })
}

fn parse_stsd(stsd: &[u8]) -> Result<Codec> {
    let mut r = Reader::new(stsd);
    r.take(8)?; // version, flags, entry count
    let entry = boxes(r.rest()).next().ok_or_else(|| format_err("mp4", "empty stsd"))??;
    match &entry.0 {
        b"avc1" => {
            let mut r = Reader::new(entry.1);
            r.take(6 + 2 + 16)?; // reserved, data reference index, pre-defined/reserved
            let width = r.u16()?;
            let height = r.u16()?;
            r.take(50)?; // resolutions, reserved, frame count, compressor name, depth, pre-defined
            let avcc = child(r.rest(), b"avcC")?.ok_or_else(|| format_err("mp4", "no avcC box"))?;
            let mut r = Reader::new(avcc);
            r.take(4)?; // version, profile, compatibility, level
            let nal_length_size = (r.u8()? & 3) + 1;
            let sets = |r: &mut Reader, n: usize| -> Result<Vec<Vec<u8>>> {
                (0..n).map(|_| { let l = r.u16()? as usize; Ok(r.take(l)?.to_vec()) }).collect()
            };
            let n = (r.u8()? & 0x1f) as usize;
            let sps = sets(&mut r, n)?;
            let n = r.u8()? as usize;
            let pps = sets(&mut r, n)?;
            Ok(Codec::Avc { width, height, nal_length_size, sps, pps })
        }
        b"mp4a" => {
            let mut r = Reader::new(entry.1);
            r.take(6 + 2 + 8)?; // reserved, data reference index, reserved
            let channels = r.u16()?;
            r.take(6)?; // sample size, pre-defined, reserved
            let sample_rate = r.u32()? >> 16;
            let esds = child(r.rest(), b"esds")?.ok_or_else(|| format_err("mp4", "no esds box"))?;
            let config = esds_config(&esds[4..])?;
            Ok(Codec::Aac { sample_rate, channels, config })
        }
        k => Ok(Codec::Other(*k)),
    }
}

/// Walks the `esds` descriptors (ES → DecoderConfig → DecoderSpecificInfo) to the `AudioSpecificConfig`.
fn esds_config(b: &[u8]) -> Result<Vec<u8>> {
    let mut r = Reader::new(b);
    loop {
        let tag = r.u8()?;
        let mut len = 0usize;
        for _ in 0..4 {
            let c = r.u8()?;
            len = len << 7 | (c & 0x7f) as usize;
            if c & 0x80 == 0 {
                break;
            }
        }
        match tag {
            3 => {
                r.take(2)?;
                let flags = r.u8()?;
                if flags & 0x80 != 0 {
                    r.take(2)?;
                }
                if flags & 0x40 != 0 {
                    let l = r.u8()? as usize;
                    r.take(l)?;
                }
                if flags & 0x20 != 0 {
                    r.take(2)?;
                }
            }
            4 => {
                r.take(13)?;
            }
            5 => return Ok(r.take(len)?.to_vec()),
            _ => {
                r.take(len)?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annexb_from_length_prefixed() {
        let mut out = Vec::new();
        avc_to_annexb(&[0, 0, 0, 2, 0x65, 1, 0, 0, 0, 1, 0x41], 4, &mut out).unwrap();
        assert_eq!(out, [0, 0, 0, 1, 0x65, 1, 0, 0, 0, 1, 0x41]);
        assert!(avc_to_annexb(&[0, 0, 0, 5, 1], 4, &mut out).is_err());
    }

    /// Every video in the user's install demuxes: an AVC track, and AAC except the race-choice loops.
    #[test]
    fn install_videos_demux() {
        let Some(install) = crate::Install::from_env(crate::text::Lang::En) else { return };
        let install = install.unwrap();
        let files = install.list("anim/mpeg4/");
        assert!(!files.is_empty());
        for f in files.iter().filter(|f| f.ends_with(".mp4")) {
            let data = install.read(f).unwrap();
            let m = parse(&data).unwrap_or_else(|e| panic!("{f}: {e}"));
            let v = m.video.as_ref().unwrap_or_else(|| panic!("{f}: no video"));
            assert!(matches!(v.codec, Codec::Avc { .. }), "{f}");
            assert!(v.samples[0].sync, "{f}");
            if let Some(a) = &m.audio {
                assert!(matches!(&a.codec, Codec::Aac { config, .. } if config.len() >= 2), "{f}");
            }
        }
    }
}
