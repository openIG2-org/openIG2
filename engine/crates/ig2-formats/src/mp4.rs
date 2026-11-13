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
        let end = (p as u64).saturating_add(size);
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
        let track = parse_trak(trak, file.len())?;
        let slot = match track.codec {
            Codec::Avc { .. } => &mut out.video,
            Codec::Aac { .. } => &mut out.audio,
            Codec::Other(_) => continue,
        };
        slot.get_or_insert(track);
    }
    Ok(out)
}

/// `file_len` bounds the sample count of a fixed-size `stsz`, which has no table to run out.
fn parse_trak(trak: &[u8], file_len: usize) -> Result<Track> {
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
    let fits = if fixed != 0 { count as u64 * fixed as u64 <= file_len as u64 } else { count <= r.rest().len() / 4 };
    if !fits {
        return Err(format_err("mp4", format!("{count} samples do not fit")));
    }
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
        if i >= samples.len() {
            break;
        }
        let n = r.u32()? as usize;
        let delta = r.u32()? as u64;
        for s in samples.iter_mut().skip(i).take(n) {
            s.dts = t;
            t = t.saturating_add(delta);
        }
        i = i.saturating_add(n);
    }
    if let Some(b) = child(stbl, b"ctts")? {
        let mut r = Reader::new(b);
        r.take(4)?;
        let mut i = 0;
        for _ in 0..r.u32()? {
            if i >= samples.len() {
                break;
            }
            let n = r.u32()? as usize;
            let off = r.u32()? as i32;
            samples.iter_mut().skip(i).take(n).for_each(|s| s.cts = off);
            i = i.saturating_add(n);
        }
    }
    if let Some(b) = child(stbl, b"stss")? {
        let mut r = Reader::new(b);
        r.take(4)?;
        samples.iter_mut().for_each(|s| s.sync = false);
        for _ in 0..r.u32()? {
            let n = r.u32()? as usize;
            if let Some(s) = n.checked_sub(1).and_then(|i| samples.get_mut(i)) {
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

/// One track for [`write`].
pub struct TrackOut<'a> {
    pub timescale: u32,
    /// `Avc` (its `nal_length_size` is that of the samples) or `Aac`.
    pub codec: Codec,
    pub samples: Vec<SampleOut<'a>>,
}

/// One sample for [`write`]: its bytes (length-prefixed NAL units for video), its duration in the track's timescale,
/// whether it is a key frame, and its place in the file (samples of both tracks are laid out by `order`, so a copy
/// keeps its original's interleave: the player's sound start depends on it, `re/screens/video.md` §4).
pub struct SampleOut<'a> {
    pub data: &'a [u8],
    pub duration: u32,
    pub sync: bool,
    pub order: u64,
}

fn put_box(out: &mut Vec<u8>, kind: &[u8; 4], body: impl FnOnce(&mut Vec<u8>)) {
    let at = out.len();
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(kind);
    body(out);
    let size = (out.len() - at) as u32;
    out[at..at + 4].copy_from_slice(&size.to_be_bytes());
}

fn full_box(out: &mut Vec<u8>, kind: &[u8; 4], flags: u32, body: impl FnOnce(&mut Vec<u8>)) {
    put_box(out, kind, |o| {
        o.extend_from_slice(&flags.to_be_bytes());
        body(o)
    });
}

fn be(out: &mut Vec<u8>, words: &[u32]) {
    out.extend(words.iter().flat_map(|w| w.to_be_bytes()));
}

/// The identity matrix of `mvhd` and `tkhd`.
const MATRIX: [u32; 9] = [0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x4000_0000];

/// Writes an `.mp4` of the remaster's layout: one AVC track and an optional AAC track, `ftyp`, `mdat` (each sample a
/// chunk, in `order`), then `moov` with 64-bit chunk offsets. What [`parse`] reads back sample for sample; no edit
/// lists, no composition offsets (the samples are in display order).
pub fn write(video: &TrackOut, audio: Option<&TrackOut>) -> Result<Vec<u8>> {
    let tracks: Vec<&TrackOut> = std::iter::once(video).chain(audio).collect();
    let mut out = Vec::new();
    put_box(&mut out, b"ftyp", |o| {
        o.extend_from_slice(b"isom");
        be(o, &[0x200]);
        o.extend_from_slice(b"isomiso2avc1mp41");
    });
    // The media data, both tracks' samples by `order`; each sample's offset kept for its `co64`.
    let mut order: Vec<(u64, usize, usize)> =
        tracks.iter().enumerate().flat_map(|(t, tr)| tr.samples.iter().enumerate().map(move |(i, s)| (s.order, t, i))).collect();
    order.sort_unstable();
    let body: u64 = tracks.iter().flat_map(|t| &t.samples).map(|s| s.data.len() as u64).sum();
    if body + 8 > u32::MAX as u64 {
        return Err(format_err("mp4", "media data past 4 GB"));
    }
    let mut offsets: Vec<Vec<u64>> = tracks.iter().map(|t| vec![0; t.samples.len()]).collect();
    put_box(&mut out, b"mdat", |o| {
        for &(_, t, i) in &order {
            offsets[t][i] = o.len() as u64;
            o.extend_from_slice(tracks[t].samples[i].data);
        }
    });
    let duration = |t: &TrackOut| t.samples.iter().map(|s| s.duration as u64).sum::<u64>();
    // Movie time in milliseconds.
    let movie = |t: &TrackOut| (duration(t) * 1000).checked_div(t.timescale as u64).unwrap_or(0) as u32;
    let mut err = None;
    put_box(&mut out, b"moov", |o| {
        full_box(o, b"mvhd", 0, |o| {
            be(o, &[0, 0, 1000, tracks.iter().map(|t| movie(t)).max().unwrap_or(0), 0x10000]);
            o.extend_from_slice(&[1, 0, 0, 0]); // volume 1.0, reserved
            be(o, &[0, 0]);
            be(o, &MATRIX);
            be(o, &[0; 6]);
            be(o, &[tracks.len() as u32 + 1]);
        });
        for (k, t) in tracks.iter().enumerate() {
            let sound = matches!(t.codec, Codec::Aac { .. });
            let (w, h) = match t.codec {
                Codec::Avc { width, height, .. } => (width as u32, height as u32),
                _ => (0, 0),
            };
            put_box(o, b"trak", |o| {
                full_box(o, b"tkhd", 3, |o| {
                    be(o, &[0, 0, k as u32 + 1, 0, movie(t), 0, 0, 0]);
                    be(o, &[if sound { 0x0100_0000 } else { 0 }]); // alternate group 0, volume, reserved
                    be(o, &MATRIX);
                    be(o, &[w << 16, h << 16]);
                });
                put_box(o, b"mdia", |o| {
                    full_box(o, b"mdhd", 0, |o| {
                        be(o, &[0, 0, t.timescale, duration(t) as u32]);
                        be(o, &[0x55c4_0000]); // "und", pre-defined
                    });
                    full_box(o, b"hdlr", 0, |o| {
                        be(o, &[0]);
                        o.extend_from_slice(if sound { b"soun" } else { b"vide" });
                        be(o, &[0, 0, 0]);
                        o.push(0);
                    });
                    put_box(o, b"minf", |o| {
                        if sound {
                            full_box(o, b"smhd", 0, |o| be(o, &[0]));
                        } else {
                            full_box(o, b"vmhd", 1, |o| be(o, &[0, 0]));
                        }
                        put_box(o, b"dinf", |o| full_box(o, b"dref", 0, |o| {
                            be(o, &[1]);
                            full_box(o, b"url ", 1, |_| ());
                        }));
                        put_box(o, b"stbl", |o| {
                            if let Err(e) = write_stsd(o, &t.codec) {
                                err = Some(e);
                            }
                            full_box(o, b"stts", 0, |o| {
                                let mut runs: Vec<(u32, u32)> = Vec::new();
                                for s in &t.samples {
                                    match runs.last_mut() {
                                        Some((n, d)) if *d == s.duration => *n += 1,
                                        _ => runs.push((1, s.duration)),
                                    }
                                }
                                be(o, &[runs.len() as u32]);
                                runs.iter().for_each(|&(n, d)| be(o, &[n, d]));
                            });
                            if t.samples.iter().any(|s| !s.sync) {
                                full_box(o, b"stss", 0, |o| {
                                    let keys: Vec<u32> = (1..).zip(&t.samples).filter(|(_, s)| s.sync).map(|(i, _)| i).collect();
                                    be(o, &[keys.len() as u32]);
                                    be(o, &keys);
                                });
                            }
                            full_box(o, b"stsz", 0, |o| {
                                be(o, &[0, t.samples.len() as u32]);
                                t.samples.iter().for_each(|s| be(o, &[s.data.len() as u32]));
                            });
                            full_box(o, b"stsc", 0, |o| be(o, &[1, 1, 1, 1]));
                            full_box(o, b"co64", 0, |o| {
                                be(o, &[t.samples.len() as u32]);
                                offsets[k].iter().for_each(|&p| o.extend_from_slice(&p.to_be_bytes()));
                            });
                        });
                    });
                });
            });
        }
    });
    err.map_or(Ok(out), Err)
}

/// The sample description: `avc1` + `avcC` (from the first SPS's profile and level) or `mp4a` + `esds`.
fn write_stsd(o: &mut Vec<u8>, codec: &Codec) -> Result<()> {
    let mut res = Ok(());
    full_box(o, b"stsd", 0, |o| {
        be(o, &[1]);
        match codec {
            Codec::Avc { width, height, nal_length_size, sps, pps } => put_box(o, b"avc1", |o| {
                o.extend_from_slice(&[0; 6]);
                o.extend_from_slice(&1u16.to_be_bytes());
                o.extend_from_slice(&[0; 16]);
                o.extend_from_slice(&width.to_be_bytes());
                o.extend_from_slice(&height.to_be_bytes());
                be(o, &[0x48_0000, 0x48_0000, 0]);
                o.extend_from_slice(&1u16.to_be_bytes());
                o.extend_from_slice(&[0; 32]);
                o.extend_from_slice(&[0, 0x18, 0xff, 0xff]);
                put_box(o, b"avcC", |o| {
                    let Some(first) = sps.first().filter(|s| s.len() >= 4) else {
                        res = Err(format_err("mp4", "no SPS"));
                        return;
                    };
                    o.extend_from_slice(&[1, first[1], first[2], first[3], 0xfc | (nal_length_size - 1), 0xe0 | sps.len() as u8]);
                    for set in sps {
                        o.extend_from_slice(&(set.len() as u16).to_be_bytes());
                        o.extend_from_slice(set);
                    }
                    o.push(pps.len() as u8);
                    for set in pps {
                        o.extend_from_slice(&(set.len() as u16).to_be_bytes());
                        o.extend_from_slice(set);
                    }
                });
            }),
            Codec::Aac { sample_rate, channels, config } => put_box(o, b"mp4a", |o| {
                o.extend_from_slice(&[0; 6]);
                o.extend_from_slice(&1u16.to_be_bytes());
                o.extend_from_slice(&[0; 8]);
                o.extend_from_slice(&channels.to_be_bytes());
                o.extend_from_slice(&[0, 16, 0, 0, 0, 0]);
                be(o, &[sample_rate << 16]);
                full_box(o, b"esds", 0, |o| {
                    // ES (3) ⊃ DecoderConfig (4: AAC, audio stream) ⊃ DecoderSpecificInfo (5), then SLConfig (6).
                    let n = config.len() as u8;
                    o.extend_from_slice(&[3, 23 + n, 0, 1, 0, 4, 15 + n, 0x40, 0x15, 0, 0, 0]);
                    be(o, &[0, 0]);
                    o.extend_from_slice(&[5, n]);
                    o.extend_from_slice(config);
                    o.extend_from_slice(&[6, 1, 2]);
                });
            }),
            Codec::Other(k) => res = Err(format_err("mp4", format!("cannot write {:?}", String::from_utf8_lossy(k)))),
        }
    });
    res
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

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = (8 + body.len() as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    fn words(w: &[u32]) -> Vec<u8> {
        w.iter().flat_map(|x| x.to_be_bytes()).collect()
    }

    #[test]
    fn largesize_past_u64_is_an_error() {
        let mut b = bx(b"free", &[]);
        b.extend_from_slice(&1u32.to_be_bytes());
        b.extend_from_slice(b"free");
        b.extend_from_slice(&u64::MAX.to_be_bytes());
        b.resize(32, 0);
        let mut it = boxes(&b);
        assert!(matches!(it.next(), Some(Ok((k, []))) if &k == b"free"));
        assert!(matches!(it.next(), Some(Err(_))));
    }

    /// Two samples in one chunk; `stts` and `ctts` list more samples than there are, `stss` holds a 0.
    #[test]
    fn sample_tables_out_of_range() {
        let mdhd = words(&[0, 0, 0, 1000, 2]);
        let stsd = [words(&[0, 1]), bx(b"none", &[])].concat();
        let stbl = [
            bx(b"stsd", &stsd),
            bx(b"stsz", &words(&[0, 0, 2, 3, 4])),
            bx(b"stco", &words(&[0, 1, 100])),
            bx(b"stsc", &words(&[0, 1, 1, 2, 1])),
            bx(b"stts", &words(&[0, 2, u32::MAX, u32::MAX, 5, 1])),
            bx(b"ctts", &words(&[0, 2, u32::MAX, 7, 5, 1])),
            bx(b"stss", &words(&[0, 2, 0, 2])),
        ]
        .concat();
        let mdia = [bx(b"mdhd", &mdhd), bx(b"minf", &bx(b"stbl", &stbl))].concat();
        let t = parse_trak(&bx(b"mdia", &mdia), 1000).unwrap();
        let got: Vec<_> = t.samples.iter().map(|s| (s.offset, s.size, s.dts, s.cts, s.sync)).collect();
        assert_eq!(got, [(100, 3, 0, 7, false), (103, 4, u32::MAX as u64, 7, true)]);
        // A fixed sample size whose count can't fit in the file.
        let big = stbl.windows(4).position(|w| w == b"stsz").unwrap() + 8;
        let mut stbl = stbl.clone();
        stbl[big..big + 8].copy_from_slice(&words(&[1, u32::MAX]));
        let mdia = [bx(b"mdhd", &mdhd), bx(b"minf", &bx(b"stbl", &stbl))].concat();
        assert!(parse_trak(&bx(b"mdia", &mdia), 1000).is_err());
    }

    /// What [`write`] lays out, [`parse`] reads back: the codecs, every sample's bytes, decode time and key-frame flag,
    /// both tracks interleaved by `order`.
    #[test]
    fn writer_round_trip() {
        let frames: Vec<Vec<u8>> = (0..7u8).map(|i| vec![i; 10 + i as usize]).collect();
        let sounds: Vec<Vec<u8>> = (0..5u8).map(|i| vec![0x80 | i; 3 + i as usize]).collect();
        let (sps, pps) = (vec![0x67, 66, 0xc0, 31, 1, 2], vec![0x68, 9]);
        let video = TrackOut {
            timescale: 12800,
            codec: Codec::Avc { width: 1280, height: 800, nal_length_size: 4, sps: vec![sps.clone()], pps: vec![pps.clone()] },
            samples: frames.iter().enumerate().map(|(i, d)| SampleOut { data: d, duration: 512, sync: i % 3 == 0, order: i as u64 * 10 }).collect(),
        };
        let audio = TrackOut {
            timescale: 44100,
            codec: Codec::Aac { sample_rate: 44100, channels: 2, config: vec![0x12, 0x10] },
            samples: sounds.iter().enumerate().map(|(i, d)| SampleOut { data: d, duration: 1024, sync: true, order: i as u64 * 10 + 25 }).collect(),
        };
        let file = write(&video, Some(&audio)).unwrap();
        let m = parse(&file).unwrap();
        let (v, a) = (m.video.unwrap(), m.audio.unwrap());
        assert!(matches!(&v.codec, Codec::Avc { width: 1280, height: 800, nal_length_size: 4, sps: s, pps: p } if *s == [sps.clone()] && *p == [pps.clone()]));
        assert!(matches!(&a.codec, Codec::Aac { sample_rate: 44100, channels: 2, config } if config == &[0x12, 0x10]));
        assert_eq!((v.timescale, v.duration, a.timescale, a.duration), (12800, 7 * 512, 44100, 5 * 1024));
        for (i, s) in v.samples.iter().enumerate() {
            assert_eq!((v.data(&file, s).unwrap(), s.dts, s.cts, s.sync), (&frames[i][..], i as u64 * 512, 0, i % 3 == 0));
        }
        for (i, s) in a.samples.iter().enumerate() {
            assert_eq!((a.data(&file, s).unwrap(), s.dts), (&sounds[i][..], i as u64 * 1024));
        }
        // Frames 0–2 come before the first sound, as their `order` says.
        assert_eq!(v.samples.iter().filter(|s| s.offset < a.samples[0].offset).count(), 3);
        // A track of key frames only has no `stss`; every sample still reads as one.
        let keys = TrackOut { samples: video.samples.iter().map(|s| SampleOut { sync: true, ..*s }).collect(), ..video };
        assert!(parse(&write(&keys, None).unwrap()).unwrap().video.unwrap().samples.iter().all(|s| s.sync));
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
