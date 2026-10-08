//! Online fallback for stars that aren't in the catalog: the name goes to CDS
//! Sesame (SIMBAD's name resolver) for position, parallax and spectral type, then
//! SIMBAD's TAP service for a V (or Gaia G) magnitude. The result becomes a star
//! like any other: it can be located, gets a dossier, and is marked on the map.
//! Both services allow cross-origin requests, so the web build calls them directly.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use catalog::{EXTERNAL_HYG, Star};

use crate::data::Sky;
use crate::locate::Locate;
use crate::picking::{Targets, reticle};
use crate::sky::SkyView;
use crate::{AppState, HOLO, field_position, globe_position, star_world};

pub struct SimbadPlugin;

impl Plugin for SimbadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Lookup>().add_systems(
            Update,
            (dispatch, receive, draw_markers).chain().run_if(in_state(AppState::Ready)),
        );
    }
}

/// What came back from SIMBAD for a name.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub name: String,
    pub otype: String,
    pub ra_deg: f32,
    pub dec_deg: f32,
    pub parallax_mas: Option<f32>,
    pub spectral: Option<String>,
    pub oid: Option<u64>,
    pub mag: Option<f32>,
}

enum Reply {
    Found(String, Found),
    NotFound(String),
    Failed(String, String),
}

#[derive(Resource, Default)]
pub struct Lookup {
    /// Set by search or a deep link; picked up by `dispatch`.
    request: Option<String>,
    /// Other spellings to try if SIMBAD doesn't know the first one.
    fallbacks: Vec<String>,
    busy: bool,
    inbox: Arc<Mutex<Vec<Reply>>>,
    /// Indices of stars added from SIMBAD, for the map markers.
    added: Vec<usize>,
}

impl Lookup {
    pub fn request(&mut self, name: &str) {
        self.request_any(&[name]);
    }

    /// Tries each spelling in turn until SIMBAD recognises one (deep-link slugs are
    /// ambiguous: "barnards-star" wants spaces, "trappist-1" keeps its dash).
    pub fn request_any(&mut self, names: &[&str]) {
        let mut names: Vec<String> =
            names.iter().map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect();
        names.dedup();
        if self.busy || names.is_empty() {
            return;
        }
        self.request = Some(names.remove(0));
        names.reverse();
        self.fallbacks = names;
    }
}

fn dispatch(mut lookup: ResMut<Lookup>, mut locate: ResMut<Locate>) {
    let Some(name) = lookup.request.take() else { return };
    lookup.busy = true;
    locate.set_note(Some(format!("QUERYING SIMBAD FOR \"{}\"...", name.to_uppercase())));
    let inbox = lookup.inbox.clone();
    let query = name.clone();
    fetch(sesame_url(&name), move |reply| {
        let reply = match reply.map(|xml| parse_sesame(&xml)) {
            Err(e) => Some(Reply::Failed(query, e)),
            Ok(None) => Some(Reply::NotFound(query)),
            // Found by name; one more request for the brightness.
            Ok(Some(found)) => match found.oid {
                Some(oid) => {
                    let inbox2 = inbox.clone();
                    fetch(tap_mag_url(oid), move |reply| {
                        let mag = reply.ok().and_then(|json| parse_tap_mag(&json));
                        inbox2.lock().unwrap().push(Reply::Found(query, Found { mag, ..found }));
                    });
                    None
                }
                None => Some(Reply::Found(query, found)),
            },
        };
        if let Some(reply) = reply {
            inbox.lock().unwrap().push(reply);
        }
    });
}

fn receive(
    mut lookup: ResMut<Lookup>,
    mut sky: ResMut<Sky>,
    mut targets: ResMut<Targets>,
    mut locate: ResMut<Locate>,
) {
    let replies: Vec<Reply> = std::mem::take(&mut *lookup.inbox.lock().unwrap());
    for reply in replies {
        lookup.busy = false;
        match reply {
            Reply::Found(query, found) => {
                info!("SIMBAD resolved {query:?} as {} ({})", found.name, found.otype);
                // Already catalogued under another name? Use that one.
                let near = sky.catalog.find_near(found.ra_deg / 15.0, found.dec_deg, 30.0);
                let index = near.unwrap_or_else(|| {
                    let star = star_from(&found, lookup.added.len() as u32);
                    let globe = globe_position(&star);
                    let field = field_position(&star);
                    let mag = star.mag;
                    let index = sky.catalog.push(star);
                    targets.push_external(index, globe, field, mag);
                    lookup.added.push(index);
                    index
                });
                lookup.fallbacks.clear();
                locate.set_note(None);
                locate.request = Some(index);
            }
            Reply::NotFound(query) => match lookup.fallbacks.pop() {
                Some(next) => lookup.request = Some(next),
                None => locate.set_note(Some(format!(
                    "SIMBAD: NO OBJECT NAMED \"{}\"",
                    query.to_uppercase()
                ))),
            },
            Reply::Failed(query, err) => {
                warn!("SIMBAD lookup for {query:?} failed: {err}");
                locate.set_note(Some("SIMBAD UNREACHABLE // CHECK CONNECTION".into()));
            }
        }
    }
}

/// A small diamond marks stars that came from SIMBAD (they aren't in the star mesh).
fn draw_markers(
    lookup: Res<Lookup>,
    sky: Res<Sky>,
    view: Res<SkyView>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut gizmos: Gizmos,
) {
    let eye = camera.translation();
    for &i in &lookup.added {
        if let Some(pos) = star_world(&sky.catalog.stars()[i], &view) {
            let color = (LinearRgba::from(HOLO) * 2.0).into();
            reticle(&mut gizmos, eye, pos, 0.006, color, std::f32::consts::FRAC_PI_4);
        }
    }
}

/// Turns a SIMBAD result into a catalog star. `n` keeps the ids unique.
pub fn star_from(found: &Found, n: u32) -> Star {
    let dist_pc = found.parallax_mas.filter(|p| *p > 0.0).map(|p| 1000.0 / p);
    let mag = found.mag.unwrap_or(99.0);
    let absmag = match dist_pc {
        Some(d) if mag < 50.0 => mag - 5.0 * (d.log10() - 1.0),
        _ => 99.0,
    };
    Star {
        hyg: EXTERNAL_HYG + n,
        hip: None,
        hd: None,
        hr: None,
        gliese: None,
        proper: Some(found.name.clone()),
        bayer: None,
        flamsteed: None,
        con: None,
        ra: found.ra_deg / 15.0,
        dec: found.dec_deg,
        dist_pc,
        mag,
        absmag,
        ci: None,
        spectral: found.spectral.clone().unwrap_or_default(),
    }
}

/// Percent-encodes everything but unreserved URL characters.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn sesame_url(name: &str) -> String {
    format!("https://cds.unistra.fr/cgi-bin/nph-sesame/-oxp/S?{}", encode(name))
}

fn tap_mag_url(oid: u64) -> String {
    let query =
        format!("SELECT filter, flux FROM flux WHERE oidref = {oid} AND filter IN ('V','G')");
    format!(
        "https://simbad.cds.unistra.fr/simbad/sim-tap/sync?request=doQuery&lang=adql&format=json&query={}",
        encode(&query)
    )
}

/// Text of the first `<tag>...</tag>` in `xml`.
fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{name}>"))? + start;
    Some(xml[start..end].trim())
}

fn unescape(s: &str) -> String {
    s.replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// SIMBAD main ids carry catalogue prefixes ("NAME Barnard's star", "* alf Lyr",
/// "V* V2134 Oph"); drop them for display.
fn clean_name(oname: &str) -> String {
    let name = unescape(oname);
    for prefix in ["NAME ", "** ", "* ", "V* "] {
        if let Some(rest) = name.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    name.trim().to_string()
}

/// Parses Sesame's `-oxp` XML; `None` when nothing was found.
pub fn parse_sesame(xml: &str) -> Option<Found> {
    let resolver = &xml[xml.find("<Resolver")?..];
    let ra_deg = tag(resolver, "jradeg")?.parse().ok()?;
    let dec_deg = tag(resolver, "jdedeg")?.parse().ok()?;
    let name = tag(resolver, "oname").map(clean_name)?;
    Some(Found {
        name,
        otype: tag(resolver, "otype").unwrap_or_default().to_string(),
        ra_deg,
        dec_deg,
        parallax_mas: tag(resolver, "plx").and_then(|p| tag(p, "v")).and_then(|v| v.parse().ok()),
        spectral: tag(resolver, "spType").map(str::to_string).filter(|s| !s.is_empty()),
        oid: tag(resolver, "oid").and_then(|v| v.parse().ok()),
        mag: None,
    })
}

/// V magnitude from the TAP JSON reply, or Gaia G if there's no V.
pub fn parse_tap_mag(json: &str) -> Option<f32> {
    let band = |name: &str| {
        let key = format!("[\"{name}\",");
        let start = json.find(&key)? + key.len();
        let end = json[start..].find(']')? + start;
        json[start..end].trim().parse::<f32>().ok()
    };
    band("V").or_else(|| band("G"))
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch(url: String, done: impl FnOnce(Result<String, String>) + Send + 'static) {
    std::thread::spawn(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(20)))
            .build()
            .into();
        let reply = agent
            .get(&url)
            .call()
            .map_err(|e| e.to_string())
            .and_then(|mut r| r.body_mut().read_to_string().map_err(|e| e.to_string()));
        done(reply);
    });
}

#[cfg(target_arch = "wasm32")]
fn fetch(url: String, done: impl FnOnce(Result<String, String>) + Send + 'static) {
    wasm_bindgen_futures::spawn_local(async move { done(fetch_text(&url).await) });
}

#[cfg(target_arch = "wasm32")]
async fn fetch_text(url: &str) -> Result<String, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response =
        JsFuture::from(window.fetch_with_str(url)).await.map_err(err)?.dyn_into().map_err(err)?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let text = JsFuture::from(response.text().map_err(err)?).await.map_err(err)?;
    text.as_string().ok_or_else(|| "response was not text".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRAPPIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Sesame><Target option="S">
  <name>TRAPPIST-1</name>
  <Resolver name="Sca=Simbad (CfA, via client/server)">
    <oid>1353525</oid>
    <otype>LM*</otype>
    <jradeg>346.62236873</jradeg>
    <jdedeg>-5.04139925</jdedeg>
    <pm><v>1046.826</v><e>0.112</e></pm>
    <plx><v>80.2123</v><e>0.0716</e><q>A</q></plx>
    <spType>M7.5e</spType>
    <oname>TRAPPIST-1</oname>
  </Resolver>
</Target></Sesame>"#;

    #[test]
    fn parses_a_found_object() {
        let f = parse_sesame(TRAPPIST).unwrap();
        assert_eq!(f.name, "TRAPPIST-1");
        assert_eq!(f.otype, "LM*");
        assert_eq!(f.oid, Some(1353525));
        assert!((f.ra_deg - 346.62237).abs() < 1e-4);
        assert!((f.dec_deg + 5.0414).abs() < 1e-4);
        // The parallax, not the proper motion's <v>.
        assert_eq!(f.parallax_mas, Some(80.2123));
        assert_eq!(f.spectral.as_deref(), Some("M7.5e"));
    }

    #[test]
    fn nothing_found() {
        let xml = r#"<Sesame><Target option="S"><name>zz</name>
            <INFO> *** NNNothing found *** </INFO></Target></Sesame>"#;
        assert_eq!(parse_sesame(xml), None);
    }

    #[test]
    fn names_lose_catalogue_prefixes() {
        assert_eq!(clean_name("NAME Barnard&apos;s star"), "Barnard's star");
        assert_eq!(clean_name("* alf Lyr"), "alf Lyr");
        assert_eq!(clean_name("V* V2134 Oph"), "V2134 Oph");
        assert_eq!(clean_name("TRAPPIST-1"), "TRAPPIST-1");
    }

    #[test]
    fn magnitudes_prefer_v() {
        let json = r#"{"metadata":[],"data":[["G",15.622554],["V",18.798]]}"#;
        assert_eq!(parse_tap_mag(json), Some(18.798));
        assert_eq!(parse_tap_mag(r#"{"data":[["G",12.5]]}"#), Some(12.5));
        assert_eq!(parse_tap_mag(r#"{"data":[]}"#), None);
    }

    #[test]
    fn stars_from_results() {
        let mut f = parse_sesame(TRAPPIST).unwrap();
        f.mag = Some(18.8);
        let s = star_from(&f, 3);
        assert!(s.is_external());
        assert!((s.dist_ly().unwrap() - 40.66).abs() < 0.05);
        assert!((s.ra - 23.1082).abs() < 1e-3);
        // M = m - 5 (log10 d - 1)
        assert!((s.absmag - (18.8 - 5.0 * (12.467f32.log10() - 1.0))).abs() < 0.01);

        f.parallax_mas = None;
        f.mag = None;
        let s = star_from(&f, 4);
        assert_eq!(s.dist_pc, None);
        assert!(s.luminosity_and_radius().is_none());
    }

    #[test]
    fn urls_are_encoded() {
        assert_eq!(encode("Barnard's star"), "Barnard%27s%20star");
        assert!(tap_mag_url(42).contains("oidref%20%3D%2042"));
    }
}
