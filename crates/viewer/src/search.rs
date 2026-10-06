//! The FIND prompt: `/` or Enter opens it, typing filters the catalog, Up/Down pick
//! a result, Enter locates it, Esc closes.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use catalog::Hit;

use crate::data::Sky;
use crate::locate::{Locate, locate_input};
use crate::look::Tinted;
use crate::{AppState, HOLO, Typing, hotkeys_enabled};

pub struct SearchPlugin;

impl Plugin for SearchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Search>().add_systems(Startup, spawn_box).add_systems(
            Update,
            (open_search.run_if(hotkeys_enabled), type_search.after(locate_input), draw_box)
                .chain()
                .run_if(in_state(AppState::Ready)),
        );
    }
}

const MAX_RESULTS: usize = 8;

#[derive(Resource, Default)]
struct Search {
    query: String,
    results: Vec<Hit>,
    cursor: usize,
    /// The key that opened the box arrives as text the same frame; skip it.
    just_opened: bool,
}

#[derive(Component)]
struct SearchBox;

fn spawn_box(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            top: px(20),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_child((
            SearchBox,
            Text::new(""),
            TextFont { font_size: FontSize::Px(15.0), ..default() },
            TextColor(HOLO),
            Tinted(1.0),
        ));
}

fn open_search(
    keys: Res<ButtonInput<KeyCode>>,
    mut typing: ResMut<Typing>,
    mut search: ResMut<Search>,
) {
    if keys.just_pressed(KeyCode::Slash) || keys.just_pressed(KeyCode::Enter) {
        typing.0 = true;
        *search = Search { just_opened: true, ..default() };
    }
}

fn type_search(
    mut keys_in: MessageReader<KeyboardInput>,
    mut typing: ResMut<Typing>,
    mut search: ResMut<Search>,
    mut locate: ResMut<Locate>,
    sky: Res<Sky>,
) {
    if !typing.0 || std::mem::take(&mut search.just_opened) {
        keys_in.clear();
        return;
    }
    let before = search.query.clone();
    for ev in keys_in.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Escape => typing.0 = false,
            Key::Enter => {
                if let Some(hit) = search.results.get(search.cursor) {
                    locate.request = Some(hit.index);
                }
                typing.0 = false;
            }
            Key::Backspace => {
                search.query.pop();
            }
            Key::ArrowDown => {
                search.cursor = (search.cursor + 1).min(search.results.len().saturating_sub(1));
            }
            Key::ArrowUp => search.cursor = search.cursor.saturating_sub(1),
            _ => {
                if let Some(text) = &ev.text {
                    let text: String = text.chars().filter(|c| !c.is_control()).collect();
                    if search.query.len() + text.len() <= 40 {
                        search.query.push_str(&text);
                    }
                }
            }
        }
    }
    if search.query != before {
        search.results = sky.catalog.search(&search.query, MAX_RESULTS);
        search.cursor = 0;
    }
}

fn draw_box(
    typing: Res<Typing>,
    search: Res<Search>,
    sky: Res<Sky>,
    time: Res<Time>,
    mut text: Single<&mut Text, With<SearchBox>>,
) {
    if !typing.0 {
        if !text.0.is_empty() {
            text.0.clear();
        }
        return;
    }
    let caret = if time.elapsed_secs().fract() < 0.5 { "_" } else { " " };
    let mut lines = vec![format!("FIND > {}{caret}", search.query.to_uppercase())];
    for (i, hit) in search.results.iter().enumerate() {
        let star = &sky.catalog.stars()[hit.index];
        let marker = if i == search.cursor { ">" } else { " " };
        let dist =
            star.dist_ly().map(|ly| format!("{ly:>8.1} LY")).unwrap_or_else(|| "    -- LY".into());
        lines.push(format!("{marker} {:<34} {:+6.2} {dist}", hit.label.to_uppercase(), star.mag));
    }
    if search.results.is_empty() && !search.query.trim().is_empty() {
        lines.push("  NO MATCH IN CATALOG".into());
    }
    lines.push("ENTER LOCATE   UP/DN SELECT   ESC CANCEL".into());
    text.0 = lines.join("\n");
}
