use std::time::{Duration, Instant};

use gpui::Window;

const FADE_IN: Duration = Duration::from_millis(600);

#[derive(Default)]
pub(crate) struct Fade {
    start: Option<Instant>,
    last: Option<Instant>,
    armed: bool,
}

impl Fade {
    pub(crate) fn opacity(&mut self, window: &mut Window) -> f32 {
        let now = Instant::now();
        match self.last {
            None => self.start = Some(now),
            Some(last) if !self.armed => {
                if now.duration_since(last) > Duration::from_millis(60) {
                    self.start = Some(now);
                }
                self.armed = true;
            }
            Some(_) => {}
        }
        self.last = Some(now);

        let elapsed = now.duration_since(self.start.unwrap());
        let t = (elapsed.as_secs_f32() / FADE_IN.as_secs_f32()).clamp(0.0, 1.0);
        if t < 1.0 {
            window.request_animation_frame();
        }
        let inv = 1.0 - t;
        1.0 - inv * inv * inv * inv * inv
    }
}
