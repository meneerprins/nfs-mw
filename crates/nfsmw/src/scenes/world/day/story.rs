//! The daylight orbit selected by career events and by cinematic lighting keys.
use super::{DayClock, WorldScene, sun_phase_limit};
use crate::scenes::world::nis::SceneLighting;

impl DayClock {
    /// A loaded career first opens its safehouse before a driving world exists.
    /// Offline PC boots near phase 0.45 and initially moves toward dawn.
    pub(crate) fn initial_safehouse_clock() -> Self {
        let mut clock = Self::new(Default::default());
        clock.set_story_phase(0.45);
        clock.direction = -1.0;
        clock
    }

    pub(crate) fn advance_in_menu(&mut self, dt: f32, frozen: bool) {
        self.update(dt, self.held || frozen);
    }

    pub(crate) fn set_story_phase(&mut self, phase: f32) {
        if phase.is_finite() && phase >= 0.0 {
            self.rate = 1.0;
            self.azimuth = self.cfg.sun_azimuth;
        }
        self.apply_orbit_phase(phase);
    }

    fn apply_orbit_phase(&mut self, phase: f32) {
        if self.manual_clock || !phase.is_finite() || phase < 0.0 {
            return;
        }
        let lo = sun_phase_limit(0);
        self.story_orbit = true;
        self.sun_mode = crate::viewer::SunMode::OriginalCycle;
        self.phase = f64::from(phase.clamp(lo, 1.0 - lo));
        if phase <= lo {
            self.direction = 1.0;
        }
        if phase >= 1.0 - lo {
            self.direction = -1.0;
        }
    }

    pub(crate) fn clear_story_overcast(&mut self) {
        self.overcast = None;
        self.rate = 1.0;
        self.azimuth = self.cfg.sun_azimuth;
    }
}
impl WorldScene {
    pub(in crate::scenes::world) fn set_story_phase(&mut self, phase: f32) {
        self.day.set_story_phase(phase);
        self.update_day(0.0, true);
        self.reflection_last = None;
    }

    pub(in crate::scenes::world) fn nis_lighting(&mut self, light: SceneLighting) {
        self.day.overcast = (light.overcast >= 0.0).then(|| light.overcast.clamp(0.0, 1.0));
        if !self.day.manual_clock {
            self.day.rate = light.rate;
            self.day.azimuth = light.azimuth;
            self.day.apply_orbit_phase(light.phase);
            self.update_day(0.0, true);
            self.reflection_last = None;
        }
    }

    pub(in crate::scenes::world) fn story_environment_sample(&self) -> serde_json::Value {
        serde_json::json!({"sun_phase": self.day.sun_phase(),"rate":self.day.rate,
            "azimuth":self.day.azimuth,"overcast":self.day.overcast,"cloud_intensity":self.visible_overcast(),
            "rain_target":self.rain.target(),"rain_intensity":self.rain.intensity()})
    }

    pub(in crate::scenes::world) fn visible_overcast(&self) -> f32 {
        if self.atmosphere.override_amount().is_some() {
            return self.atmosphere.intensity();
        }
        self.day.overcast.unwrap_or_else(|| self.atmosphere.intensity())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewer::{Environment, SunMode};
    #[test]
    fn cold_safehouse_clock_moves_before_first_world_load() {
        let mut clock = DayClock::initial_safehouse_clock();
        clock.advance_in_menu(12.0, true);
        assert_eq!(clock.sun_phase(), 0.45);
        clock.advance_in_menu(12.0, false);
        assert!((clock.sun_phase() - 0.44).abs() < 1e-6);
    }

    #[test]
    fn story_orbit_survives_weather_edits_but_yields_to_manual_time() {
        let mut day = DayClock::new(Default::default());
        day.set_story_phase(1.0);
        day.advance_in_menu(12.0, false);
        assert!((day.sun_phase() - (1.0 - sun_phase_limit(0) - 0.01)).abs() < 1e-5);
        let before = day.sun_phase();
        day.configure(day.environment());
        day.set_story_phase(-1.0);
        assert_eq!(day.sun_phase(), before);
        day.set_story_phase(0.5);
        assert_eq!(day.hour(), 12.0);
        let manual = Environment { sun_mode: SunMode::FixedPhase, sun_phase: 0.25, ..day.environment() };
        day.configure(manual);
        day.set_story_phase(1.0);
        assert_eq!(day.sun_phase(), 0.25);
    }
}
