//! Deep links. On the web the map opens focused on a star given as `?star=vega` or
//! by a star page's `data-focus` attribute, and the address bar follows whatever
//! star is located so it can be shared. On desktop, `--star vega` does the same.

use bevy::prelude::*;

use crate::AppState;
use crate::data::Sky;
use crate::locate::Locate;

pub struct DeepLinkPlugin;

impl Plugin for DeepLinkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Focus { requested: read_focus(), shown: None })
            .add_systems(OnEnter(AppState::Ready), apply_focus)
            .add_systems(Update, follow_locate.run_if(in_state(AppState::Ready)));
    }
}

#[derive(Resource)]
struct Focus {
    requested: Option<String>,
    shown: Option<usize>,
}

fn apply_focus(mut focus: ResMut<Focus>, sky: Res<Sky>, mut locate: ResMut<Locate>) {
    let Some(slug) = focus.requested.take() else { return };
    match sky.catalog.resolve_slug(&slug) {
        Some(index) => {
            locate.request = Some(index);
            // Already in the address bar; don't rewrite it.
            focus.shown = Some(index);
        }
        None => warn!("deep link: no star matches {slug:?}"),
    }
}

fn follow_locate(locate: Res<Locate>, sky: Res<Sky>, mut focus: ResMut<Focus>) {
    let current = locate.current();
    if current == focus.shown {
        return;
    }
    focus.shown = current;
    let slug = current.map(|i| {
        let page = sky.catalog.named_slugs().into_iter().find(|(j, _)| *j == i);
        page.map(|(_, s)| s).unwrap_or_else(|| sky.catalog.stars()[i].slug())
    });
    set_address(slug.as_deref());
}

/// Decodes the `%XX` escapes and `+` of a URL query value.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(
                    std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                    16,
                ) {
                    Ok(b) => {
                        out.push(b);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Pulls `star` out of a query string like `?star=vega&x=1`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn star_param(query: &str) -> Option<String> {
    query
        .trim_start_matches('?')
        .split('&')
        .find_map(|pair| pair.strip_prefix("star="))
        .map(percent_decode)
        .filter(|s| !s.trim().is_empty())
}

#[cfg(target_arch = "wasm32")]
fn read_focus() -> Option<String> {
    let window = web_sys::window()?;
    let from_query = window.location().search().ok().as_deref().and_then(star_param);
    from_query
        .or_else(|| window.document()?.get_element_by_id("viewer")?.get_attribute("data-focus"))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_focus() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().enumerate().find_map(|(i, a)| {
        a.strip_prefix("--star=")
            .map(str::to_owned)
            .or_else(|| (a == "--star").then(|| args.get(i + 1).cloned()).flatten())
    })
}

/// Points the address bar at the located star (relative to the page's base, so star
/// pages fall back to the root map URL), or back to the plain map.
#[cfg(target_arch = "wasm32")]
fn set_address(slug: Option<&str>) {
    let Some(history) = web_sys::window().and_then(|w| w.history().ok()) else { return };
    let url = match slug {
        Some(slug) => format!("./?star={slug}"),
        None => "./".to_string(),
    };
    let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&url));
}

#[cfg(not(target_arch = "wasm32"))]
fn set_address(_slug: Option<&str>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_parsing() {
        assert_eq!(star_param("?star=vega").as_deref(), Some("vega"));
        assert_eq!(star_param("?x=1&star=rigil%20kentaurus").as_deref(), Some("rigil kentaurus"));
        assert_eq!(star_param("?star=alpha+centauri").as_deref(), Some("alpha centauri"));
        assert_eq!(star_param("?star=").as_deref(), None);
        assert_eq!(star_param("").as_deref(), None);
        assert_eq!(percent_decode("b%C3%A9l%C3%A9nos"), "b\u{e9}l\u{e9}nos");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
