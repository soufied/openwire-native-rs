use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub struct PlaybackView {
    pub progress: f32,
    pub elapsed: Duration,
    pub length: Duration,
    pub paused: bool,
}

pub(crate) struct Playback {
    length: Duration,
    accumulated: Duration,
    running_since: Option<Instant>,
    looping: bool,
}

impl Playback {
    pub(crate) fn new(length: Duration, looping: bool) -> Self {
        Self {
            length,
            accumulated: Duration::ZERO,
            running_since: Some(Instant::now()),
            looping,
        }
    }

    pub(crate) fn elapsed(&self) -> Duration {
        self.accumulated + self.running_since.map(|t| t.elapsed()).unwrap_or(Duration::ZERO)
    }

    pub(crate) fn finished(&self) -> bool {
        !self.looping && self.elapsed() >= self.length
    }

    pub(crate) fn paused(&self) -> bool {
        self.running_since.is_none()
    }

    pub(crate) fn pause(&mut self) {
        if let Some(since) = self.running_since.take() {
            self.accumulated += since.elapsed();
        }
    }

    pub(crate) fn resume(&mut self) {
        if self.running_since.is_none() {
            self.running_since = Some(Instant::now());
        }
    }

    pub(crate) fn seek(&mut self, progress: f32) {
        self.accumulated = self.length.mul_f32(progress.clamp(0.0, 1.0));
        if self.running_since.is_some() {
            self.running_since = Some(Instant::now());
        }
    }

    pub(crate) fn view(&self) -> PlaybackView {
        let len = self.length.as_secs_f32().max(1e-3);
        let elapsed = self.elapsed().as_secs_f32();
        let (progress, shown) = if self.looping {
            let position = elapsed % len;
            (position / len, Duration::from_secs_f32(position))
        } else {
            ((elapsed / len).min(1.0), self.elapsed().min(self.length))
        };
        PlaybackView {
            progress,
            elapsed: shown,
            length: self.length,
            paused: self.paused(),
        }
    }
}
