//! The FIND prompt: `/` or Enter opens it, typing filters the catalog, Up/Down or the
//! pointer pick a result, Enter or a tap/click locates it, Esc closes.
//!
//! On touch screens in the browser the canvas can't raise the on-screen keyboard,
//! so the page has a real text field (see the site generator). It mirrors its text
//! into the canvas's `data-query` attribute and signals Enter through `data-submit`;
//! `web_bridge` picks those up here.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use catalog::Hit;

use crate::data::Sky;
use crate::locate::{Locate, locate_input};
use crate::look::{Look, Tinted};
use crate::simbad::Lookup;
use crate::{AppState, HOLO, Typing, hotkeys_enabled};

pub struct SearchPlugin;

impl Plugin for SearchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Search>().add_systems(Startup, spawn_box).add_systems(
            Update,
            (
                open_search.run_if(hotkeys_enabled),
                web_bridge,
                type_search.after(locate_input),
                pick_row,
                close_web,
                draw_box,
            )
                .chain()
                .run_if(in_state(AppState::Ready)),
        );
    }
}

const MAX_RESULTS: usize = 8;
/// The extra row after the results that sends the query to SIMBAD.
const SIMBAD_ROW: usize = MAX_RESULTS;

#[derive(Resource, Default)]
pub struct Search {
    query: String,
    results: Vec<Hit>,
    cursor: usize,
    /// The key that opened the box arrives as text the same frame; skip it.
    just_opened: bool,
    /// Opened from the page's text field rather than the keyboard.
    from_web: bool,
    last_submit: Option<String>,
}

/// Opens an empty FIND box (the `/` key or the FIND button).
pub fn open_box(search: &mut Search, typing: &mut Typing) {
    typing.0 = true;
    *search = Search { just_opened: true, last_submit: search.last_submit.take(), ..default() };
}

impl Search {
    /// Long enough to be worth asking SIMBAD about.
    fn online_ok(&self) -> bool {
        self.query.trim().chars().count() >= 2
    }

    /// Locates the highlighted result, or sends the query to SIMBAD when the
    /// highlight is on the online row (or there are no results at all).
    fn choose(&self, locate: &mut Locate, lookup: &mut Lookup) {
        match self.results.get(self.cursor) {
            Some(hit) => locate.request = Some(hit.index),
            None if self.online_ok() => lookup.request(&self.query),
            None => {}
        }
    }

    fn set_query(&mut self, query: String, sky: &Sky) {
        if query != self.query {
            self.results = sky.catalog.search(&query, MAX_RESULTS);
            self.query = query;
            self.cursor = 0;
        }
    }
}

#[derive(Component)]
struct Outer;
#[derive(Component)]
struct Panel;
#[derive(Component)]
struct Header;
#[derive(Component)]
struct Footer;
#[derive(Component)]
struct ResultRow(usize);

fn spawn_box(mut commands: Commands) {
    let text = |size: f32| {
        (Text::new(""), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(HOLO))
    };
    commands
        .spawn((
            Outer,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                top: px(20),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|outer| {
            outer
                .spawn((
                    Panel,
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(px(10), px(6)),
                        ..default()
                    },
                    BackgroundColor(Color::BLACK.with_alpha(0.6)),
                    ZIndex(20),
                    Visibility::Hidden,
                ))
                .with_children(|panel| {
                    panel.spawn((Header, text(15.0), Tinted(1.0)));
                    for i in 0..=MAX_RESULTS {
                        panel
                            .spawn((
                                ResultRow(i),
                                Button,
                                Node { padding: UiRect::axes(px(2), px(2)), ..default() },
                                BackgroundColor(Color::NONE),
                            ))
                            .with_child((text(15.0), Tinted(1.0)));
                    }
                    panel.spawn((Footer, text(13.0), Tinted(0.7)));
                });
        });
}

fn open_search(
    keys: Res<ButtonInput<KeyCode>>,
    mut typing: ResMut<Typing>,
    mut search: ResMut<Search>,
) {
    if keys.just_pressed(KeyCode::Slash) || keys.just_pressed(KeyCode::Enter) {
        open_box(&mut search, &mut typing);
    }
}

fn type_search(
    mut keys_in: MessageReader<KeyboardInput>,
    mut typing: ResMut<Typing>,
    mut search: ResMut<Search>,
    mut locate: ResMut<Locate>,
    mut lookup: ResMut<Lookup>,
    sky: Res<Sky>,
) {
    if !typing.0 || search.from_web || std::mem::take(&mut search.just_opened) {
        keys_in.clear();
        return;
    }
    let mut query = search.query.clone();
    for ev in keys_in.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Escape => typing.0 = false,
            Key::Enter => {
                search.choose(&mut locate, &mut lookup);
                typing.0 = false;
            }
            Key::Backspace => {
                query.pop();
            }
            Key::ArrowDown => {
                // The online row sits just past the results.
                let last = if search.online_ok() {
                    search.results.len()
                } else {
                    search.results.len().saturating_sub(1)
                };
                search.cursor = (search.cursor + 1).min(last);
            }
            Key::ArrowUp => search.cursor = search.cursor.saturating_sub(1),
            _ => {
                if let Some(text) = &ev.text {
                    let text: String = text.chars().filter(|c| !c.is_control()).collect();
                    if query.len() + text.len() <= 40 {
                        query.push_str(&text);
                    }
                }
            }
        }
    }
    search.set_query(query, &sky);
}

/// Pointer or finger on a result row: hover highlights it, a press locates it.
fn pick_row(
    rows: Query<(&Interaction, &ResultRow), Changed<Interaction>>,
    mut typing: ResMut<Typing>,
    mut search: ResMut<Search>,
    mut locate: ResMut<Locate>,
    mut lookup: ResMut<Lookup>,
) {
    if !typing.0 {
        return;
    }
    for (interaction, row) in &rows {
        // The online row's slot is just past the results.
        let slot = if row.0 == SIMBAD_ROW { search.results.len() } else { row.0 };
        if row.0 != SIMBAD_ROW && search.results.get(slot).is_none() {
            continue;
        }
        match interaction {
            Interaction::Pressed => {
                search.cursor = slot;
                search.choose(&mut locate, &mut lookup);
                typing.0 = false;
            }
            Interaction::Hovered => search.cursor = slot,
            Interaction::None => {}
        }
    }
}

type RowParts<'a> = (&'a ResultRow, &'a Children, &'a mut Node, &'a mut BackgroundColor);

#[allow(clippy::too_many_arguments)]
fn draw_box(
    typing: Res<Typing>,
    look: Res<Look>,
    search: Res<Search>,
    sky: Res<Sky>,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut outer: Single<&mut Node, (With<Outer>, Without<ResultRow>)>,
    mut panel: Single<&mut Visibility, (With<Panel>, Without<ResultRow>)>,
    mut header: Single<&mut Text, (With<Header>, Without<Footer>)>,
    mut footer: Single<&mut Text, (With<Footer>, Without<Header>)>,
    mut rows: Query<RowParts>,
    mut row_text: Query<&mut Text, (Without<Header>, Without<Footer>)>,
) {
    panel.set_if_neq(if typing.0 { Visibility::Inherited } else { Visibility::Hidden });
    if !typing.0 {
        return;
    }
    // The page's own text field sits at the top while it's in use.
    let top = px(if search.from_web { 72.0 } else { 20.0 });
    if outer.top != top {
        outer.top = top;
    }
    let narrow = window.width() < 600.0;
    let caret = if time.elapsed_secs().fract() < 0.5 { "_" } else { " " };
    header.0 = format!("FIND > {}{caret}", search.query.to_uppercase());

    for (row, children, mut node, mut bg) in &mut rows {
        let online = row.0 == SIMBAD_ROW;
        let hit = search.results.get(row.0);
        // Unused rows take no space.
        let shown = if online { search.online_ok() } else { hit.is_some() };
        let display = if shown { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        if !shown {
            continue;
        }
        let slot = if online { search.results.len() } else { row.0 };
        let selected = slot == search.cursor;
        let marker = if selected { ">" } else { " " };
        let line = if let (false, Some(hit)) = (online, hit) {
            let star = &sky.catalog.stars()[hit.index];
            let dist = star
                .dist_ly()
                .map(|ly| format!("{ly:>8.1} LY"))
                .unwrap_or_else(|| "    -- LY".into());
            // Phones get a narrower row without the magnitude column.
            if narrow {
                let label: String = hit.label.to_uppercase().chars().take(22).collect();
                format!("{marker} {label:<22} {dist}")
            } else {
                let label: String = hit.label.to_uppercase().chars().take(34).collect();
                format!("{marker} {label:<34} {:+6.2} {dist}", star.mag)
            }
        } else {
            let q: String = search.query.trim().to_uppercase().chars().take(20).collect();
            format!("{marker} QUERY SIMBAD ONLINE: \"{q}\"")
        };
        let text = children.first().and_then(|c| row_text.get_mut(*c).ok());
        if let Some(mut text) = text.filter(|t| t.0 != line) {
            text.0 = line;
        }
        bg.0 = if selected { look.color().with_alpha(0.12) } else { Color::NONE };
    }

    footer.0 = if search.results.is_empty() && !search.query.trim().is_empty() {
        "NO MATCH IN CATALOG // ENTER TO ASK SIMBAD".into()
    } else {
        if narrow {
            "TAP A STAR TO LOCATE".into()
        } else {
            "ENTER OR TAP TO LOCATE   UP/DN SELECT   ESC CANCEL".into()
        }
    };
}

#[cfg(target_arch = "wasm32")]
fn canvas() -> Option<web_sys::Element> {
    web_sys::window()?.document()?.get_element_by_id("viewer")
}

/// Mirrors the page's FIND text field (touch screens) into the search box.
#[cfg(target_arch = "wasm32")]
fn web_bridge(
    mut search: ResMut<Search>,
    mut typing: ResMut<Typing>,
    mut locate: ResMut<Locate>,
    mut lookup: ResMut<Lookup>,
    sky: Res<Sky>,
) {
    let Some(canvas) = canvas() else { return };
    let submit = canvas.get_attribute("data-submit");
    if submit.is_some() && submit != search.last_submit {
        search.last_submit = submit;
        if search.from_web {
            search.choose(&mut locate, &mut lookup);
            typing.0 = false;
        }
        return;
    }
    match canvas.get_attribute("data-query") {
        Some(query) => {
            if !search.from_web {
                open_box(&mut search, &mut typing);
                search.from_web = true;
            }
            search.set_query(query, &sky);
        }
        None if search.from_web => typing.0 = false,
        None => {}
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn web_bridge() {}

/// Once the box closes, put the page's text field away too.
fn close_web(typing: Res<Typing>, mut search: ResMut<Search>) {
    if typing.0 || !search.from_web {
        return;
    }
    search.from_web = false;
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        if let Some(canvas) = canvas() {
            let _ = canvas.remove_attribute("data-query");
        }
        let field = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("find"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
        if let Some(field) = field {
            let _ = field.blur();
        }
    }
}
