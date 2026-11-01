//! The browser's H.264 decoder (ADR 0047 part 4): WebCodecs' `VideoDecoder` through the page (`ig2_video_*` in
//! web/main.js), fed Annex B access units from `ig2_formats::mp4` as the native decoder is; the frames come back as
//! RGBA drawn through a canvas, in display order, into the [`super::Player`]'s channel. The YUV → RGB conversion is
//! the browser's (the stream's colour space), not the remaster shader's arithmetic of `decode::rgba`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::Arc;

use ig2_formats::mp4::{self, Codec};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::decode::Clip;
use crate::assets::Image;

#[wasm_bindgen]
extern "C" {
    type Decoder;

    /// A configured decoder of `codec` (`avc1.PPCCLL`); every frame comes to `on_frame` as RGBA, `null` on an error.
    #[wasm_bindgen(js_name = ig2_video_open)]
    fn open(codec: &str, width: u32, height: u32, on_frame: &Closure<dyn FnMut(JsValue)>) -> Decoder;
    #[wasm_bindgen(js_name = ig2_video_chunk)]
    fn chunk(d: &Decoder, key: bool, timestamp: f64, data: &[u8]);
    /// The decoder's `decodeQueueSize`.
    #[wasm_bindgen(js_name = ig2_video_queue)]
    fn queue(d: &Decoder) -> u32;
    #[wasm_bindgen(js_name = ig2_video_flush)]
    fn flush(d: &Decoder) -> js_sys::Promise;
    #[wasm_bindgen(js_name = ig2_video_close)]
    fn close(d: &Decoder);
}

/// Frames decoded but not yet in the channel, and chunks queued in the decoder, at most.
const AHEAD: usize = 4;

async fn sleep(ms: i32) {
    let p = js_sys::Promise::new(&mut |res, _| {
        let _ = web_sys::window().expect("a window").set_timeout_with_callback_and_timeout_and_arguments_0(&res, ms);
    });
    let _ = JsFuture::from(p).await;
}

/// Decode `clip` on the page (again from the start while `looping`) into `tx`, until the player is gone.
pub fn spawn(clip: Arc<Clip>, looping: bool, tx: SyncSender<Image>) {
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = run(clip, looping, tx).await {
            eprintln!("ig2-client: video: {e}");
        }
    });
}

type Got = Rc<RefCell<VecDeque<Option<Image>>>>;

/// Moves the decoded frames into the channel while it takes them; `Err` when the player is gone or the decoder
/// failed.
fn forward(got: &Got, tx: &SyncSender<Image>) -> Result<(), String> {
    let mut g = got.borrow_mut();
    while let Some(f) = g.pop_front() {
        let img = f.ok_or("the browser's decoder failed")?;
        match tx.try_send(img) {
            Ok(()) => {}
            Err(TrySendError::Full(img)) => {
                g.push_front(Some(img));
                break;
            }
            Err(TrySendError::Disconnected(_)) => return Err(String::new()),
        }
    }
    Ok(())
}

async fn run(clip: Arc<Clip>, looping: bool, tx: SyncSender<Image>) -> Result<(), String> {
    let v = clip.video();
    let Codec::Avc { width, height, nal_length_size, sps, pps } = &v.codec else { return Err("not AVC".into()) };
    let s = sps.first().filter(|s| s.len() >= 4).ok_or("no SPS")?;
    let codec = format!("avc1.{:02x}{:02x}{:02x}", s[1], s[2], s[3]);
    let (w, h) = (*width as u32, *height as u32);
    let got: Got = Rc::default();
    let g = got.clone();
    let on_frame = Closure::<dyn FnMut(JsValue)>::new(move |rgba: JsValue| {
        let img = (!rgba.is_null()).then(|| Image { width: w, height: h, rgba: js_sys::Uint8Array::new(&rgba).to_vec(), mips: Vec::new(), hires: None });
        g.borrow_mut().push_back(img);
    });
    let dec = open(&codec, w, h, &on_frame);
    let params = mp4::avc_parameter_sets(sps, pps);
    let mut au = Vec::new();
    let result = async {
        loop {
            for s in &v.samples {
                while got.borrow().len() + queue(&dec) as usize > AHEAD {
                    forward(&got, &tx)?;
                    sleep(4).await;
                }
                forward(&got, &tx)?;
                let Ok(bytes) = v.data(&clip.data, s) else { continue };
                au.clear();
                // Every key frame carries the parameter sets (no `description`: Annex B).
                if s.sync {
                    au.extend_from_slice(&params);
                }
                if mp4::avc_to_annexb(bytes, *nal_length_size, &mut au).is_err() {
                    continue;
                }
                let pts = (s.dts as i64 + s.cts as i64) as f64 * 1e6 / v.timescale.max(1) as f64;
                chunk(&dec, s.sync, pts, &au);
            }
            JsFuture::from(flush(&dec)).await.map_err(|_| "the browser's decoder failed")?;
            while !got.borrow().is_empty() {
                forward(&got, &tx)?;
                sleep(4).await;
            }
            if !looping || v.samples.is_empty() {
                return Ok(());
            }
        }
    }
    .await;
    close(&dec);
    drop(on_frame);
    // The player went away: not an error.
    result.or_else(|e: String| if e.is_empty() { Ok(()) } else { Err(e) })
}
