//! Stock PC precipitation in the renderer's Z-up world. Spec: `rain-pc.md`.
use super::{drops::DropPool, lens::LensRain};
use blackbox_hash::bstring_hash;
use blackbox_paramap::Layer;
use glam::Vec3;

#[derive(Debug, Clone, Copy)]
enum RainOverride {
    Regional,
    Fixed(f32),
}

impl RainOverride {
    fn from_signed(value: f32) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        if value == 0.0 {
            return Some(Self::Regional);
        }
        Some(Self::Fixed(value.clamp(0.0, 1.0)))
    }
}

#[derive(Debug, Clone)]
pub struct Rain {
    zones: Option<blackbox_roads::TrackZones>,
    curtain: super::curtain::Curtain,
    layer: Option<Layer>,
    intensity: f32,
    pool: DropPool,
    lens: LensRain,
    always: bool,
    amount: f32,
    story_override: RainOverride,
    nis_override: Option<RainOverride>,
    exposed: bool,
    previous: Option<Vec3>,
}
impl Default for Rain {
    fn default() -> Self {
        Self::new()
    }
}
impl Rain {
    pub fn new() -> Self {
        Self {
            zones: None,
            curtain: Default::default(),
            layer: None,
            intensity: 0.0,
            pool: DropPool::default(),
            lens: LensRain::default(),
            always: false,
            amount: 1.0,
            story_override: RainOverride::Regional,
            nis_override: None,
            exposed: true,
            previous: None,
        }
    }
    pub fn read(metadata: &[u8]) -> anyhow::Result<Self> {
        let layer = Layer::read_all(metadata)?.into_iter().find(|l| l.name_hash == bstring_hash("Rain"));
        Ok(Self { layer, zones: blackbox_roads::TrackZones::read(metadata)?, ..Self::new() })
    }
    pub fn configure(&mut self, always: bool, amount: f32) {
        self.always = always;
        if amount.is_finite() {
            self.amount = amount.clamp(0.0, 1.0);
        }
    }
    /// What [`Self::configure`] set: rain on every track, and the amount from 0 to 1.
    pub fn setting(&self) -> (bool, f32) {
        (self.always, self.amount)
    }
    /// Sets the race's rain policy. Negative selects dry, zero regional, and positive explicit rain.
    pub fn set_story_override(&mut self, value: f32) {
        if let Some(policy) = RainOverride::from_signed(value) {
            self.story_override = policy;
        }
    }
    /// Returns the normalized signed race or roaming rain policy.
    #[cfg(test)]
    pub fn story_override(&self) -> f32 {
        match self.story_override {
            RainOverride::Regional => 0.0,
            RainOverride::Fixed(0.0) => -1.0,
            RainOverride::Fixed(amount) => amount,
        }
    }
    /// Applies a scene rain key using the same signed policy, until the scene ends.
    pub fn set_nis_override(&mut self, value: f32) {
        if let Some(policy) = RainOverride::from_signed(value) {
            self.nis_override = Some(policy);
        }
    }
    /// Gives weather control back to the current race or roaming policy.
    pub fn clear_nis_override(&mut self) {
        self.nis_override = None;
    }
    pub fn target_at(&self, camera: Vec3) -> f32 {
        if self.amount == 0.0 {
            return 0.0;
        }
        if self.always {
            return 1.0;
        }
        if let RainOverride::Fixed(amount) = self.nis_override.unwrap_or(self.story_override) {
            return amount;
        }
        let Some(layer) = &self.layer else { return 0.0 };
        let Some(set) = layer.set_at(camera.x, camera.y) else { return 0.0 };
        let (Some(value), Some(threshold)) = (layer.float(set, 0), layer.float(set, 1)) else { return 0.0 };
        if !value.is_finite() || !threshold.is_finite() || super::regional_gate(1.0, camera.x) > threshold {
            return 0.0;
        }
        value.clamp(0.0, 1.0)
    }
    pub fn target(&self) -> f32 {
        self.target_at(self.previous.unwrap_or(Vec3::ZERO))
    }
    pub fn intensity(&self) -> f32 {
        self.intensity
    }
    /// Visible precipitation after the user's rain-amount control.
    pub fn visible_intensity(&self) -> f32 {
        self.intensity * self.amount
    }
    pub fn lens(&self) -> &[super::lens::Droplet] {
        self.lens.visible(self.exposed && self.intensity > 0.0 && self.amount > 0.0)
    }
    #[cfg(test)]
    pub fn update(&mut self, dt: f32, camera: Vec3) {
        self.update_view(dt, camera, Vec3::X, true);
    }
    pub fn update_view(&mut self, dt: f32, camera: Vec3, forward: Vec3, exposed: bool) {
        if !dt.is_finite() || dt <= 0.0 || !camera.is_finite() || !forward.is_finite() {
            return;
        }
        self.exposed = exposed;
        let target = self.target_at(camera);
        let alpha = 1.0 - (59.0_f32 / 60.0).powf(60.0 * dt);
        self.intensity += (target - self.intensity) * alpha;
        if target == 0.0 && self.intensity < 0.01 {
            self.intensity = 0.0;
        }
        let displacement = self.previous.map_or(Vec3::ZERO, |old| camera - old);
        self.previous = Some(camera);
        self.pool.update(dt, camera, forward, self.intensity * self.amount, exposed);
        let zone = self.zones.as_ref().and_then(|zones| {
            zones.zones.iter().find(|zone| {
                matches!(zone.kind, 3 | 4 | 5 | 7) && zone.contains(camera.truncate()) && zone.elevation > camera.z
            })
        });
        self.curtain.update(dt, camera, forward, zone.filter(|_| !exposed), self.intensity * self.amount);
        self.lens.update(dt, (displacement / dt).length(), exposed && self.intensity > 0.0 && self.amount > 0.0);
    }
    pub fn falling(&self) -> usize {
        self.pool.falling()
    }
    pub fn quads(&self, camera: Vec3, out: &mut Vec<([Vec3; 4], usize)>) {
        self.pool.quads(camera, out);
        self.curtain.quads(camera, out);
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn regional_rain() -> Rain {
        fn chunk(id: u32, payload: &[u8]) -> Vec<u8> {
            let mut bytes = id.to_le_bytes().to_vec();
            bytes.extend((payload.len() as u32).to_le_bytes());
            bytes.extend(payload);
            bytes
        }
        let header = [bstring_hash("Rain"), 0, 0, 0, 0, 0, 1, 8, 2].map(u32::to_le_bytes).concat();
        let mut layer = chunk(0x3B602, &header);
        layer.extend(chunk(0x3B603, &[0; 8]));
        layer.extend(chunk(0x3B604, &[0u32, 4].map(u32::to_le_bytes).concat()));
        layer.extend(chunk(0x3B605, &[0.5f32, 1.0].map(f32::to_le_bytes).concat()));
        Rain::read(&chunk(0x8003_B600, &chunk(0x8003_B601, &layer))).unwrap()
    }

    #[test]
    fn scripted_rain_layers_over_regional_rain_and_respects_manual_settings() {
        let mut rain = regional_rain();
        assert_eq!(rain.target(), 0.5, "roaming samples the regional map");
        rain.set_story_override(-1.0);
        assert_eq!(rain.story_override(), -1.0);
        assert_eq!(rain.target(), 0.0, "the race forces a dry target");
        rain.set_nis_override(0.0);
        assert_eq!(rain.target(), 0.5, "the scene can request regional weather");
        rain.set_nis_override(0.75);
        rain.set_nis_override(f32::NAN);
        assert_eq!(rain.target(), 0.75, "invalid keys leave the current scene target");
        rain.configure(false, 0.5);
        rain.update(30.0, Vec3::ZERO);
        assert!((rain.visible_intensity() - 0.375).abs() < 1e-6, "manual amount scales scene rain");
        rain.configure(true, 0.0);
        assert_eq!(rain.target(), 0.0, "zero amount overrides forced and scripted rain");
        rain.configure(true, 1.0);
        assert_eq!(rain.target(), 1.0, "manual forced rain overrides a scene");
        rain.configure(false, 1.0);
        rain.clear_nis_override();
        assert_eq!(rain.target(), 0.0, "releasing the scene restores the race target");
        rain.set_story_override(0.25);
        assert_eq!(rain.target(), 0.25, "positive race override bypasses the map");
        rain.set_story_override(0.0);
        assert_eq!(rain.target(), 0.5, "the regional map resumes after the race");
    }

    #[test]
    #[ignore = "requires the original global texture and track packs"]
    fn installed_rain_textures_and_regional_map_are_present() {
        let dir = game_install::GameDir::open(std::env::var_os("NFSMW_GAME_DIR").unwrap()).unwrap();
        let globals = nfsmw_data::world::load_global_textures(&dir).unwrap();
        for name in ["RAINDROP", "RAINDROPPC"] {
            let texture = globals.textures.iter().find(|t| t.name_hash == bstring_hash(name)).expect(name);
            assert!(texture.width > 0 && texture.height > 0);
        }
        let bytes = nfsmw_data::read_unwrapped(&dir, "TRACKS/L2RA.BUN").unwrap();
        let rain = Rain::read(&bytes).unwrap();
        let layer = rain.layer.expect("installed Rain parameter map");
        assert!(layer.set_count() > 0);
        assert!((0..layer.set_count()).any(|set| layer.float(set, 0).is_some_and(|v| v > 0.0)));
    }
    #[test]
    fn shelter_blocks_lens_water_without_drying_the_weather() {
        let mut rain = Rain::new();
        rain.configure(true, 1.0);
        rain.update_view(3.0, Vec3::ZERO, Vec3::X, false);
        assert!(rain.intensity() > 0.9);
        assert!(rain.lens().is_empty());
        assert_eq!(rain.falling(), 0);
        rain.update_view(1.0 / 60.0, Vec3::ZERO, Vec3::X, true);
        assert!(!rain.lens().is_empty());
        assert!(rain.falling() > 300);
    }
    #[test]
    fn pausing_holds_weather_and_droplets() {
        let mut rain = Rain::new();
        rain.configure(true, 1.0);
        rain.update(1.0, Vec3::ZERO);
        let before = rain.clone();
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            rain.update(dt, Vec3::splat(100.0));
        }
        assert_eq!(rain.intensity(), before.intensity());
        assert_eq!(rain.lens(), before.lens());
        assert_eq!(rain.pool, before.pool);
    }
    #[test]
    fn zero_amount_removes_precipitation() {
        let mut rain = Rain::new();
        rain.configure(true, 0.0);
        rain.update(5.0, Vec3::ZERO);
        assert_eq!(rain.falling(), 0);
        assert_eq!(rain.intensity(), 0.0);
        assert!(rain.lens().is_empty());
    }
}
