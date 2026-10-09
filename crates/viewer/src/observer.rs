//! The observer's place on Earth and the "tonight" overlay. Once a location is
//! set (O opens the prompt, or the device location on the web), the hologram
//! shader marks the part of the sky facing the observer tonight: a wedge within
//! six hours of the meridian, a bright slice on the meridian that turns with
//! the Earth, and the horizon. Y hides or shows it.

use std::f64::consts::{PI, TAU};
use std::sync::Mutex;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::apply_orbit;
use crate::look::{HoloEffect, Look};
use crate::search::{Search, open_location};
use crate::sky::SkyView;
use crate::{AppState, CHART_RADIUS, GLOBE_RADIUS, Typing, hotkeys_enabled};

pub struct ObserverPlugin;

impl Plugin for ObserverPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Observer { place: load(), show: true, note: None }).add_systems(
            Update,
            (
                observer_keys.run_if(hotkeys_enabled).run_if(in_state(AppState::Ready)),
                receive_device,
                feed_overlay.after(apply_orbit),
            )
                .chain(),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// Degrees, north positive.
    pub lat: f64,
    /// Degrees, east positive.
    pub lon: f64,
}

impl Place {
    /// "34.05N 118.24W"
    pub fn label(&self) -> String {
        let ns = if self.lat < 0.0 { 'S' } else { 'N' };
        let ew = if self.lon < 0.0 { 'W' } else { 'E' };
        format!("{:.2}{ns} {:.2}{ew}", self.lat.abs(), self.lon.abs())
    }
}

#[derive(Resource)]
pub struct Observer {
    pub place: Option<Place>,
    /// Y toggles the overlay without forgetting the place.
    pub show: bool,
    /// One-off message for the FIND box footer (device location errors).
    pub note: Option<String>,
}

impl Observer {
    pub fn set(&mut self, place: Option<Place>, view: &mut SkyView) {
        self.place = place;
        self.show = true;
        save(place);
        // Face the chart toward the pole the observer can see.
        if let Some(p) = place {
            view.south = p.lat < 0.0;
        }
    }
}

/// Parses "34.05 -118.24", "34.05, -118.24" or "34.05N 118.24W" (any case).
pub fn parse_place(text: &str) -> Option<Place> {
    let parts: Vec<&str> =
        text.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
    let [a, b] = parts[..] else { return None };
    let coord = |s: &str, pos: char, neg: char| -> Option<f64> {
        let s = s.to_ascii_uppercase();
        let (num, sign) = match s.chars().last()? {
            c if c == pos => (&s[..s.len() - 1], 1.0),
            c if c == neg => (&s[..s.len() - 1], -1.0),
            c if c.is_ascii_digit() || c == '.' => (&s[..], 1.0),
            _ => return None,
        };
        let v: f64 = num.parse().ok()?;
        (v.is_finite() && (sign > 0.0 || v >= 0.0)).then_some(v * sign)
    };
    let lat = coord(a, 'N', 'S')?;
    let mut lon = coord(b, 'E', 'W')?;
    if lon > 180.0 && lon <= 360.0 {
        lon -= 360.0;
    }
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some(Place { lat, lon })
}

/// Local sidereal time in radians for a Unix time (seconds) and east longitude.
pub fn local_sidereal(unix_secs: f64, lon_deg: f64) -> f64 {
    let days = unix_secs / 86_400.0 + 2_440_587.5 - 2_451_545.0;
    let gmst = 280.460_618_37 + 360.985_647_366_29 * days;
    (gmst + lon_deg).to_radians().rem_euclid(TAU)
}

/// The Sun's right ascension and declination (radians), good to a few arcminutes.
pub fn sun_position(unix_secs: f64) -> (f64, f64) {
    let d = unix_secs / 86_400.0 + 2_440_587.5 - 2_451_545.0;
    let l = (280.460 + 0.985_647_4 * d).to_radians();
    let g = (357.528 + 0.985_600_3 * d).to_radians();
    let lambda = l + (1.915 * g.sin() + 0.020 * (2.0 * g).sin()).to_radians();
    let eps = (23.439 - 0.000_000_4 * d).to_radians();
    let ra = (eps.cos() * lambda.sin()).atan2(lambda.cos()).rem_euclid(TAU);
    (ra, (eps.sin() * lambda.sin()).asin())
}

/// The Sun's altitude that counts as dark: nautical twilight, when the stars
/// are out.
const DARK_ALT_DEG: f64 = -12.0;

/// Tonight's wedge: the band of right ascension that crosses the meridian while
/// it is dark, as (centre, half width) in radians. The centre is opposite the
/// Sun (local midnight). Zero width when it never gets dark, a full circle when
/// it never gets light.
pub fn night_wedge(unix_secs: f64, lat_deg: f64) -> (f64, f64) {
    let (ra, dec) = sun_position(unix_secs);
    let lat = lat_deg.to_radians();
    // Hour angle at which the Sun sinks to the dark altitude.
    let cos_h = (DARK_ALT_DEG.to_radians().sin() - lat.sin() * dec.sin())
        / (lat.cos() * dec.cos()).max(1e-9);
    let half = PI - cos_h.clamp(-1.0, 1.0).acos();
    ((ra + PI).rem_euclid(TAU), half)
}

fn now_unix() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() / 1000.0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64())
    }
}

fn observer_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut observer: ResMut<Observer>,
    mut search: ResMut<Search>,
    mut typing: ResMut<Typing>,
) {
    if keys.just_pressed(KeyCode::KeyO) {
        observer.note = None;
        open_location(&mut search, &mut typing);
    }
    if keys.just_pressed(KeyCode::KeyY) {
        observer.show = !observer.show;
    }
}

/// Fills the overlay part of the hologram uniforms: the camera's ray basis (taken
/// after the orbit update so it matches this frame's view) and the sky state.
pub fn feed_overlay(
    observer: Res<Observer>,
    view: Res<SkyView>,
    look: Res<Look>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut camera: Single<(&mut HoloEffect, &Transform, &Projection)>,
) {
    let (effect, tf, projection) = &mut *camera;
    let place = observer.place.filter(|_| observer.show);
    let Some(place) = place else {
        effect.sky.z = 0.0;
        return;
    };
    let Projection::Perspective(p) = projection else { return };
    let half_h = (p.fov * 0.5).tan();
    let half_w = half_h * window.width() / window.height().max(1.0);
    effect.cam_pos = tf.translation.extend(1.0);
    effect.cam_right = (tf.right() * half_w).extend(0.0);
    effect.cam_up = (tf.up() * half_h).extend(0.0);
    effect.cam_fwd = tf.forward().extend(0.0);
    let now = now_unix();
    let lst = local_sidereal(now, place.lon);
    effect.sky = Vec4::new(place.lat.to_radians() as f32, lst as f32, 1.0, view.chart);
    let (centre, half) = night_wedge(now, place.lat);
    effect.sky3 = Vec4::new(centre as f32, half as f32, 0.0, 0.0);
    let pole = if view.south { -1.0 } else { 1.0 };
    effect.sky2 = Vec4::new(view.unfold, pole, CHART_RADIUS, GLOBE_RADIUS);
    let (wedge, now) = look.overlay_colors();
    effect.wedge_color = LinearRgba::from(wedge).to_vec4();
    effect.now_color = LinearRgba::from(now).to_vec4();
}

/// Result of a device location request, filled in by the browser callback.
static DEVICE: Mutex<Option<Result<Place, String>>> = Mutex::new(None);

/// Asks the browser for the device's location (it shows its own permission
/// prompt); the answer arrives through `receive_device`.
#[cfg(target_arch = "wasm32")]
pub fn request_device() {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    let Some(geo) = web_sys::window().and_then(|w| w.navigator().geolocation().ok()) else {
        *DEVICE.lock().unwrap() = Some(Err("NO LOCATION SERVICE".into()));
        return;
    };
    let ok = Closure::once_into_js(|pos: web_sys::Position| {
        let c = pos.coords();
        *DEVICE.lock().unwrap() = Some(Ok(Place { lat: c.latitude(), lon: c.longitude() }));
    });
    let err = Closure::once_into_js(|_: wasm_bindgen::JsValue| {
        *DEVICE.lock().unwrap() = Some(Err("LOCATION DENIED OR UNAVAILABLE".into()));
    });
    let _ =
        geo.get_current_position_with_error_callback(ok.unchecked_ref(), Some(err.unchecked_ref()));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn request_device() {
    *DEVICE.lock().unwrap() = Some(Err("TYPE LAT LON, E.G. 34.05 -118.24".into()));
}

/// Device location is only offered where the browser has it.
pub fn device_available() -> bool {
    cfg!(target_arch = "wasm32")
}

fn receive_device(mut observer: ResMut<Observer>, mut view: ResMut<SkyView>) {
    let Some(result) = DEVICE.lock().unwrap().take() else { return };
    match result {
        Ok(place) => {
            observer.set(Some(place), &mut view);
            observer.note = None;
        }
        Err(e) => observer.note = Some(e),
    }
}

const STORE_KEY: &str = "star-navigator.observer";

fn encode(place: Option<Place>) -> String {
    place.map(|p| format!("{} {}", p.lat, p.lon)).unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
fn load() -> Option<Place> {
    parse_place(&storage()?.get_item(STORE_KEY).ok()??)
}

#[cfg(target_arch = "wasm32")]
fn save(place: Option<Place>) {
    if let Some(s) = storage() {
        let _ = s.set_item(STORE_KEY, &encode(place));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn store_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME"))?;
    Some(std::path::PathBuf::from(base).join("Star-Navigator").join(STORE_KEY))
}

#[cfg(not(target_arch = "wasm32"))]
fn load() -> Option<Place> {
    parse_place(&std::fs::read_to_string(store_path()?).ok()?)
}

#[cfg(not(target_arch = "wasm32"))]
fn save(place: Option<Place>) {
    let Some(path) = store_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, encode(place));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_forms() {
        let la = Place { lat: 34.05, lon: -118.24 };
        assert_eq!(parse_place("34.05 -118.24"), Some(la));
        assert_eq!(parse_place("34.05, -118.24"), Some(la));
        assert_eq!(parse_place("34.05n 118.24W"), Some(la));
        assert_eq!(parse_place("33.9S 151.2E"), Some(Place { lat: -33.9, lon: 151.2 }));
        assert_eq!(parse_place("10 350").map(|p| p.lon), Some(-10.0));
        assert_eq!(parse_place("95 10"), None);
        assert_eq!(parse_place("-34S 10"), None);
        assert_eq!(parse_place("vega"), None);
        assert_eq!(parse_place("1 2 3"), None);
    }

    #[test]
    fn sidereal_time_matches_reference() {
        // 2000-01-01 12:00 UT: GMST is 18h 41m 50.5s (280.46 degrees).
        let gmst = local_sidereal(946_728_000.0, 0.0).to_degrees();
        assert!((gmst - 280.4606).abs() < 1e-3);
        // East longitude adds directly.
        let lst = local_sidereal(946_728_000.0, 90.0).to_degrees();
        assert!((lst - 10.4606).abs() < 1e-3);
    }

    #[test]
    fn sun_follows_the_seasons() {
        // 2026-03-20 15:00 UT (equinox): RA near 0h, dec near 0.
        let (ra, dec) = sun_position(1_774_018_800.0);
        assert!(ra.to_degrees().min(360.0 - ra.to_degrees()) < 1.0);
        assert!(dec.to_degrees().abs() < 0.5);
        // 2026-06-21 08:24 UT (solstice): RA 6h, dec +23.44.
        let (ra, dec) = sun_position(1_782_030_240.0);
        assert!((ra.to_degrees() - 90.0).abs() < 1.0);
        assert!((dec.to_degrees() - 23.44).abs() < 0.1);
    }

    #[test]
    fn nights_have_the_right_length() {
        // Equinox at the equator: dark from Sun altitude -12 is 12h - 2 * 48 min.
        let (_, half) = night_wedge(1_774_018_800.0, 0.0);
        assert!((half.to_degrees() - 78.0).abs() < 0.5);
        // Midsummer far north never gets dark; midwinter there never gets light.
        assert_eq!(night_wedge(1_782_030_240.0, 70.0).1, 0.0);
        assert!((night_wedge(1_782_030_240.0, -80.0).1 - PI).abs() < 1e-9);
    }

    #[test]
    fn labels_read_naturally() {
        assert_eq!(Place { lat: 34.05, lon: -118.24 }.label(), "34.05N 118.24W");
    }
}
