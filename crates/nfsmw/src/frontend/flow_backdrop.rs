//! Putting a scene in the window: swapping the scene, and showing the menu backdrop behind the menus.

use super::*;

impl Frontend {
    pub(in crate::frontend) fn advance_parked_career_clock(&mut self, dt: f32, frozen: bool) {
        if let Some(clock) = self.parked_career_clock.as_mut() {
            clock.advance_in_menu(dt, frozen);
        }
    }

    /// Takes the current clock with its direction and rate before the world is released.
    fn clock_for_menu(&self, host: &Host) -> Option<crate::scenes::world::DayClock> {
        if !self.career.is_active() || self.parked_career_clock.is_some() {
            return None;
        }
        let mut clock = host.scene.career_clock()?;
        clock.clear_story_overcast();
        Some(clock)
    }

    /// The scripted first car lot and first safe house movie set the sun to the start of daylight.
    pub(super) fn seek_career_day_start(&mut self, host: &Host) {
        let mut clock = self
            .parked_career_clock
            .take()
            .or_else(|| host.scene.career_clock())
            .unwrap_or_else(|| crate::scenes::world::DayClock::new(Default::default()));
        clock.clear_story_overcast();
        clock.set_story_phase(0.0);
        self.parked_career_clock = Some(clock);
    }

    /// Runs the car select's loading LEDs while the menu backdrop is still building its car (R2).
    pub(in crate::frontend) fn sync_car_loading(&mut self, host: &Host) {
        let loading = self.stage == Stage::Menus && self.menu_in_window && !host.scene.ready();
        self.screens.set_car_loading(loading);
    }

    /// Shows a browsed screen over the scene already in the window.
    pub(in crate::frontend) fn show_browsed(&mut self, name: &str, host: &mut Host, env: &mut Env) {
        self.latch = true;
        self.screens.clear();
        let commands = self.screens.open(name, Args::default(), true, env);
        self.stage = Stage::Menus;
        self.apply(commands, host, env);
    }

    /// Puts `scene` in the window. With `park`, a menu backdrop being replaced is kept for the menus that come back
    /// (`Host::replace_scene_parking_old`); any other scene is released.
    pub(super) fn replace_scene(&mut self, host: &mut Host, scene: Box<dyn crate::viewer::Scene>, park: bool) -> bool {
        let parks_menu = park && self.menu_in_window;
        let clock = self.clock_for_menu(host);
        let result = match parks_menu {
            true => host.replace_scene_parking_old(scene),
            false => host.replace_scene(scene),
        };
        match result {
            Ok(()) => {
                if let Some(clock) = clock {
                    self.parked_career_clock = Some(clock);
                }
                self.menu_in_window = false;
                true
            }
            Err(e) => {
                log::error!("cannot change the scene: {e:#}");
                false
            }
        }
    }

    /// Puts the menu backdrop in the window. A backdrop already there is kept, and so is one parked while free roam
    /// had the window: building one reads the car and the room from the install on a worker thread and uploads them over
    /// the next frames (`MenuScene::update`, ~150-190 ms of work in all), which the menus that follow would only repeat.
    pub(super) fn show_backdrop(&mut self, host: &mut Host) {
        if self.menu_in_window {
            return;
        }
        let clock = self.clock_for_menu(host);
        if host.restore_parked_menu() {
            if let Some(clock) = clock {
                self.parked_career_clock = Some(clock);
            }
            self.menu_in_window = true;
            return;
        }
        // A backdrop built from nothing plays its camera intro, whatever the camera did last.
        self.camera.reset();
        let scene = MenuScene::with_look(Some(self.dir.clone()), self.look.clone()).with_rig(self.camera.clone());
        if !self.replace_scene(host, Box::new(scene), false) {
            return;
        }
        self.menu_in_window = true;
    }
}
