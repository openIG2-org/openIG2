//! The mixer behind the sound engine (re/screens/audio.md §2): SDL_mixer's model as the exe's `SoundEngine` uses it:
//! chunks played once on numbered channels with a volume 0..=128, one music stream with its own volume (the exe
//! fades by volume steps, `FRISSIT2_SoundSystem`; its `SetPan` does nothing). Device-independent: `mix` fills
//! stereo f32 frames at the output rate, so tests run without a sound card.

use std::sync::Arc;

/// SDL_mixer's `MIX_MAX_VOLUME`.
pub const MAX_VOLUME: i32 = 128;

/// A decoded sound: interleaved stereo at the mixer's rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub frames: Arc<[[f32; 2]]>,
}

impl Chunk {
    pub fn new(frames: Vec<[f32; 2]>) -> Self {
        Self { frames: frames.into() }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }
}

/// A music stream: fills stereo frames at the mixer's rate, returns how many it wrote (fewer at the end).
pub trait Stream: Send {
    fn read(&mut self, out: &mut [[f32; 2]]) -> usize;
}

#[derive(Debug, Clone)]
struct Channel {
    chunk: Chunk,
    pos: usize,
    /// Plays left after this one (−1 forever), as `Mix_PlayChannel`'s `loops` (the exe always passes 0).
    loops: i32,
    volume: i32,
}

struct Music {
    stream: Box<dyn Stream>,
}

pub struct Mixer {
    pub rate: u32,
    channels: Vec<Option<Channel>>,
    music: Option<Music>,
    music_volume: i32,
    /// Set when the music stream ran out (`Mix_HookMusicFinished`): the owner picks the next track.
    pub music_finished: bool,
    /// The channels [`Self::alloc_free`] made.
    voices: Vec<usize>,
    /// A video's sound (re/screens/video.md §4): a stream of its own beside the music, with its own volume.
    video: Option<(Box<dyn Stream>, i32)>,
}

impl Mixer {
    pub fn new(rate: u32, channels: usize) -> Self {
        Self { rate, channels: vec![None; channels], music: None, music_volume: MAX_VOLUME, music_finished: false, voices: Vec::new(), video: None }
    }

    /// A new channel (sgsnd allocates as it goes; nothing is reserved).
    pub fn alloc(&mut self) -> usize {
        self.channels.push(None);
        self.channels.len() - 1
    }

    /// A channel for a voice of its own (`SE_Wave` auto-release, freed by `Poll` when done): the first idle one past
    /// the effects' fixed voices that no one holds, else a new one. Voices only reuse voice channels.
    pub fn alloc_free(&mut self) -> usize {
        if let Some(i) = self.voices.iter().copied().find(|&i| !self.playing(i)) {
            return i;
        }
        let i = self.alloc();
        self.voices.push(i);
        i
    }

    /// `Mix_PlayChannel(channel, chunk, loops)`: restarts the channel with the chunk; keeps its volume and panning.
    pub fn play(&mut self, channel: usize, chunk: &Chunk, loops: i32) {
        let volume = self.channels[channel].as_ref().map_or(MAX_VOLUME, |c| c.volume);
        self.channels[channel] = Some(Channel { chunk: chunk.clone(), pos: 0, loops, volume });
    }

    /// `Mix_HaltChannel`.
    pub fn halt(&mut self, channel: usize) {
        if let Some(c) = &mut self.channels[channel] {
            c.pos = c.chunk.len();
            c.loops = 0;
        }
    }

    /// `Mix_Playing`.
    pub fn playing(&self, channel: usize) -> bool {
        self.channels[channel].as_ref().is_some_and(|c| c.pos < c.chunk.len())
    }

    /// `Mix_Volume`: 0..=128, clamped as SDL_mixer does.
    pub fn set_volume(&mut self, channel: usize, volume: i32) {
        let volume = volume.clamp(0, MAX_VOLUME);
        match &mut self.channels[channel] {
            Some(c) => c.volume = volume,
            None => self.channels[channel] = Some(Channel { chunk: Chunk::new(Vec::new()), pos: 0, loops: 0, volume }),
        }
    }

    /// `Mix_PlayMusic(music, 0)`: replaces the music.
    pub fn play_music(&mut self, stream: Box<dyn Stream>) {
        self.music = Some(Music { stream });
        self.music_finished = false;
    }

    /// `sgsnd::stop_channels("music")`: the music stops (its end is not reported as finished).
    pub fn stop_music(&mut self) {
        self.music = None;
    }

    pub fn music_playing(&self) -> bool {
        self.music.is_some()
    }

    /// `Mix_VolumeMusic`.
    pub fn set_music_volume(&mut self, volume: i32) {
        self.music_volume = volume.clamp(0, MAX_VOLUME);
    }

    /// Starts a video's sound at `volume` 0..=128 (replacing one still playing).
    pub fn play_video(&mut self, stream: Box<dyn Stream>, volume: i32) {
        self.video = Some((stream, volume.clamp(0, MAX_VOLUME)));
    }

    pub fn stop_video(&mut self) {
        self.video = None;
    }

    /// Mixes the next `out.len()` frames over silence.
    pub fn mix(&mut self, out: &mut [[f32; 2]]) {
        out.fill([0.0; 2]);
        for c in self.channels.iter_mut().flatten() {
            let gain = c.volume as f32 / MAX_VOLUME as f32;
            let mut i = 0;
            while i < out.len() && c.pos < c.chunk.len() {
                let s = c.chunk.frames[c.pos];
                out[i][0] += s[0] * gain;
                out[i][1] += s[1] * gain;
                i += 1;
                c.pos += 1;
                if c.pos == c.chunk.len() && c.loops != 0 {
                    c.pos = 0;
                    if c.loops > 0 {
                        c.loops -= 1;
                    }
                }
            }
        }
        self.mix_music(out);
        if let Some((stream, volume)) = &mut self.video {
            let mut buf = vec![[0.0f32; 2]; out.len()];
            let n = stream.read(&mut buf);
            let gain = *volume as f32 / MAX_VOLUME as f32;
            for (o, s) in out.iter_mut().zip(&buf[..n]) {
                o[0] += s[0] * gain;
                o[1] += s[1] * gain;
            }
            if n < out.len() {
                self.video = None;
            }
        }
        for f in out.iter_mut() {
            f[0] = f[0].clamp(-1.0, 1.0);
            f[1] = f[1].clamp(-1.0, 1.0);
        }
    }

    fn mix_music(&mut self, out: &mut [[f32; 2]]) {
        let Some(m) = &mut self.music else { return };
        let mut buf = vec![[0.0f32; 2]; out.len()];
        let n = m.stream.read(&mut buf);
        let volume = self.music_volume as f32 / MAX_VOLUME as f32;
        for (o, s) in out.iter_mut().zip(&buf[..n]) {
            o[0] += s[0] * volume;
            o[1] += s[1] * volume;
        }
        if n < out.len() {
            self.music = None;
            self.music_finished = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ramp(usize);
    impl Stream for Ramp {
        fn read(&mut self, out: &mut [[f32; 2]]) -> usize {
            let n = out.len().min(self.0);
            out[..n].fill([0.5, 0.5]);
            self.0 -= n;
            n
        }
    }

    #[test]
    fn video_sound_mixes_and_ends() {
        let mut m = Mixer::new(1000, 0);
        m.play_video(Box::new(Ramp(5)), 64);
        let mut out = [[0.0; 2]; 4];
        m.mix(&mut out);
        assert_eq!(out[3], [0.25, 0.25]);
        assert!(m.video.is_some());
        m.mix(&mut out);
        assert_eq!(out[1], [0.0, 0.0]);
        assert!(m.video.is_none());
    }

    #[test]
    fn volume_scales_a_chunk() {
        let mut m = Mixer::new(1000, 2);
        m.set_volume(0, 64);
        m.play(0, &Chunk::new(vec![[1.0, 1.0]; 4]), 0);
        let mut out = [[0.0; 2]; 6];
        m.mix(&mut out);
        assert_eq!(out[0], [0.5, 0.5]);
        assert_eq!(out[4], [0.0, 0.0]);
        assert!(!m.playing(0));
    }

    #[test]
    fn loops_repeat_and_forever_never_ends() {
        let mut m = Mixer::new(1000, 1);
        m.play(0, &Chunk::new(vec![[1.0, 1.0]; 3]), 1);
        let mut out = [[0.0; 2]; 8];
        m.mix(&mut out);
        assert_eq!(out.iter().filter(|f| f[0] > 0.0).count(), 6);
        m.play(0, &Chunk::new(vec![[1.0, 1.0]; 3]), -1);
        m.mix(&mut out);
        assert!(m.playing(0));
    }

    #[test]
    fn music_reports_its_end() {
        let mut m = Mixer::new(1000, 0);
        m.set_music_volume(64);
        m.play_music(Box::new(Ramp(25)));
        let mut out = [[0.0; 2]; 20];
        m.mix(&mut out);
        assert!((out[0][0] - 0.25).abs() < 1e-6 && m.music_playing());
        m.mix(&mut out);
        assert!(!m.music_playing() && m.music_finished && out[10][0] == 0.0);
    }
}
