//! Scene-clock event dispatch, including the ICE camera's cut tags.

use super::{NisEvent, Playing, camera};

impl Playing {
    pub(super) fn advance_events(&mut self, step: f32) -> Vec<NisEvent> {
        let Some(events) = self.events.as_mut() else { return Vec::new() };
        let key = self
            .camera
            .as_ref()
            .and_then(|track| track.key_index(camera::track_fraction(track, self.run.scene_time())));
        if key.is_some() && self.camera_key.is_none() {
            events.camera_cut(0);
            self.camera_key = Some(0);
        }
        let mut fired = events.advance(step);
        if let Some(key) = key.filter(|&index| Some(index) != self.camera_key) {
            events.camera_cut(key);
            fired.extend(events.advance(0.0));
            self.camera_key = Some(key);
        }
        fired
    }
}
