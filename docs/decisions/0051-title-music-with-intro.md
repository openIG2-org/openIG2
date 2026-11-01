# ADR 0051: the title video's music starts with the intro

Date: 2026-11-01. Status: accepted. Implemented by `ig2-client::audio` (`mixer::Gate`, `Audio::play_video`'s gate),
`ig2-client::video` (`TITLE_INTRO_MS`), `ig2-client::window` (`video_frame`) and `ig2-client::display` (the "Title
music with intro" row).

## Context
`title.mp4` is one 88 s clip: the Imperium Galactica II logo (with the THQ Nordic overlay), the Digital Reality
logo, then the intro. Its single sound track is continuous music from 0.5 s, so the exe (and the port, bit for bit
in timing, re/screens/video.md §4) plays the intro's music under both logos. The logos then feel like part of the
intro rather than a quiet opening.

## Decision
1. **A "Title music with intro" row** (`TitleMusicWithIntro`, on by default). Off = the exe: the clip's sound from
   its start.
2. **The gate**: the title's sound is silent until 14.56 s of video time, where the Digital Reality logo has faded
   to black, and rises linearly to full at 15.88 s, the intro's first picture (measured on the decoded frames: mean
   luminance 0.9 from 14 560 ms, 3.0 at 15 880 ms). The gate counts the sound's own output frames, which follow the
   picture clock within the exe's 16–17 ms (video.md §4), so no new sync is needed. The music under the intro is
   the clip's own, at the same place as before; only the part under the logos is silent.
3. Only `title.bik`; the credits and every other video keep their sound.

## Rejected
- **Starting the track from its beginning at the intro**: the music is cut to the intro's pictures; shifting it
  would put it out of step with them.
- **Ducking instead of silence**: the request is music only with the intro.
