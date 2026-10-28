//! Generated native source adapted from media-lab/src/video_lab.rs.
#[cfg(target_os = "macos")]
use anyhow::Result;
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceFormat {
    #[default]
    Bgra,
    Full601,
    Video601,
    Full709,
    Video709,
}
#[cfg(target_os = "macos")]
impl SourceFormat {
    const ALL: [Self; 5] = [Self::Bgra, Self::Full601, Self::Video601, Self::Full709, Self::Video709];
    pub fn next(self) -> Self {
        Self::ALL[(Self::ALL.iter().position(|f| *f == self).unwrap() + 1) % Self::ALL.len()]
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Bgra => "BGRA8",
            Self::Full601 => "NV12 full · BT.601",
            Self::Video601 => "NV12 video · BT.601",
            Self::Full709 => "NV12 full · BT.709",
            Self::Video709 => "NV12 video · BT.709",
        }
    }
    fn frame(self, index: u64) -> Result<xgpui_media::CoreVideoFrame> {
        use gpui_apple::custom_draw::core_video::{Nv12Matrix, test_frame, test_nv12_frame};
        match self {
            Self::Bgra => test_frame(320, 180, index),
            Self::Full601 => test_nv12_frame(320, 180, index, Nv12Matrix::Bt601, false),
            Self::Video601 => test_nv12_frame(320, 180, index, Nv12Matrix::Bt601, true),
            Self::Full709 => test_nv12_frame(320, 180, index, Nv12Matrix::Bt709, false),
            Self::Video709 => test_nv12_frame(320, 180, index, Nv12Matrix::Bt709, true),
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use std::{
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::{Duration, Instant},
    };
    use xgpui_media::{VideoFrame, VideoStream};
    pub struct Demo {
        pub stream: VideoStream,
        stop: Arc<AtomicBool>,
        burst: mpsc::SyncSender<()>,
        error: Arc<Mutex<Option<String>>>,
    }
    impl Demo {
        pub fn new(format: SourceFormat) -> Result<Self> {
            let (sender, stream) = VideoStream::channel();
            sender.publish(VideoFrame::core_video(format.frame(0)?, Duration::ZERO))?;
            let stop = Arc::new(AtomicBool::new(false));
            let done = stop.clone();
            let error = Arc::new(Mutex::new(None));
            let report = error.clone();
            let (burst, requests) = mpsc::sync_channel(1);
            std::thread::Builder::new().name("media-lab-video".into()).spawn(move || {
                let mut index = 1;
                while !done.load(Ordering::Relaxed) {
                    let start = Instant::now();
                    let count = if requests.try_recv().is_ok() { 120 } else { 1 };
                    for _ in 0..count {
                        if done.load(Ordering::Relaxed) {
                            break;
                        }
                        let result = format
                            .frame(index)
                            .map(|f| VideoFrame::core_video(f, Duration::from_micros(index * 33_333)))
                            .and_then(|f| sender.publish(f));
                        if let Err(e) = result {
                            *report.lock().unwrap() = Some(e.to_string());
                            return;
                        }
                        index += 1;
                    }
                    std::thread::sleep(Duration::from_millis(33).saturating_sub(start.elapsed()));
                }
            })?;
            Ok(Self { stream, stop, burst, error })
        }
    }
    impl Drop for Demo {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }
    impl Demo {
        pub fn stop(&self) {
            self.stop.store(true, Ordering::Relaxed);
        }
        pub fn burst(&self) {
            let _ = self.burst.try_send(());
        }
        pub fn error(&self) -> Option<String> {
            self.error.lock().ok().and_then(|error| error.clone())
        }
    }
}
#[cfg(target_os = "macos")]
pub use native::Demo;
