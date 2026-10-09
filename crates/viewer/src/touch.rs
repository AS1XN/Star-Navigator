//! Touch screens: one finger rotates, two fingers pinch to zoom, a quick tap selects
//! a star. Also the on-screen controls that stand in for keyboard shortcuts on touch
//! screens and narrow windows: a slim bottom row (FIND, LOCATE, CHART/GLOBE, RAW/FX,
//! BACK, MENU), a MENU list with the other display toggles, and a strip for the
//! calibration panel.

use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::{Orbit, apply_orbit, over_ui};
use crate::chart::toggle_chart;
use crate::locate::Locate;
use crate::look::{Look, PALETTES, Tinted, TintedBorder, Tuner};
use crate::observer::{Observer, device_available, request_device};
use crate::picking::{Selection, TAP_RADIUS_PX, Targets, pick_at};
use crate::search::{Search, open_box, open_location};
use crate::sky::{MAG_MAX, MAG_MIN, SkyView};
use crate::sound::SoundOn;
use crate::{AppState, HOLO, Typing};

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchState>()
            .init_resource::<Menu>()
            .add_systems(Startup, spawn_controls)
            .add_systems(
                Update,
                (
                    gestures.before(apply_orbit),
                    (
                        show_controls,
                        size_buttons,
                        press_buttons,
                        bar_labels,
                        menu_labels,
                        place_menu,
                    )
                        .chain(),
                ),
            )
            .add_systems(Update, taps.run_if(in_state(AppState::Ready)));
    }
}

/// A tap must end within this distance of where it started, and this quickly.
const TAP_SLOP_PX: f32 = 12.0;
const TAP_SECS: f64 = 0.35;

#[derive(Resource, Default)]
pub struct TouchState {
    /// Set once any touch is seen; switches the HUD to touch mode.
    pub used: bool,
    /// When each finger went down, and whether it may still count as a tap.
    started: Vec<(u64, f64, bool)>,
}

/// What a frame of finger movement means for the camera: a drag in pixels and a
/// zoom factor (below 1 zooms in).
pub fn gesture(fingers: &[(Vec2, Vec2)]) -> (Vec2, f32) {
    match fingers {
        [(now, before)] => (*now - *before, 1.0),
        [(a, a0), (b, b0)] => {
            let (d, d0) = (a.distance(*b), a0.distance(*b0));
            if d > 1.0 && d0 > 1.0 { (Vec2::ZERO, d0 / d) } else { (Vec2::ZERO, 1.0) }
        }
        _ => (Vec2::ZERO, 1.0),
    }
}

fn gestures(
    touches: Res<Touches>,
    ui: Query<&Interaction>,
    mut state: ResMut<TouchState>,
    mut orbit: ResMut<Orbit>,
) {
    let fingers: Vec<(Vec2, Vec2)> =
        touches.iter().map(|t| (t.position(), t.previous_position())).collect();
    if fingers.is_empty() {
        return;
    }
    state.used = true;
    // Fingers that start on a HUD button belong to the button.
    if touches.iter_just_pressed().next().is_some() && over_ui(&ui) {
        return;
    }
    let (drag, zoom) = gesture(&fingers);
    if drag != Vec2::ZERO {
        orbit.rotate_by(drag);
    }
    if zoom != 1.0 {
        orbit.zoom_by(zoom);
    }
}

#[allow(clippy::too_many_arguments)]
fn taps(
    time: Res<Time>,
    touches: Res<Touches>,
    ui: Query<&Interaction>,
    camera: Single<(&Camera, &GlobalTransform)>,
    targets: Res<Targets>,
    view: Res<SkyView>,
    mut state: ResMut<TouchState>,
    mut selection: ResMut<Selection>,
) {
    let now = time.elapsed_secs_f64();
    let multi = touches.iter().count() > 1;
    for t in touches.iter_just_pressed() {
        state.started.push((t.id(), now, !over_ui(&ui)));
    }
    // A second finger turns everything into a pinch, not a tap.
    if multi {
        for s in &mut state.started {
            s.2 = false;
        }
    }
    let (camera, cam_tf) = *camera;
    for t in touches.iter_just_released() {
        let Some(at) = state.started.iter().position(|s| s.0 == t.id()) else { continue };
        let (_, began, tappable) = state.started.swap_remove(at);
        let still = t.position().distance(t.start_position()) < TAP_SLOP_PX;
        if tappable && still && now - began < TAP_SECS {
            selection.hovered = None;
            selection.selected =
                pick_at(t.position(), camera, cam_tf, &targets, &view, TAP_RADIUS_PX);
        }
    }
    state.started.retain(|s| touches.get_pressed(s.0).is_some());
}

#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum Action {
    Find,
    Locate,
    Back,
    Menu,
    Fainter,
    Brighter,
    Grid,
    Figures,
    Spin,
    Palette,
    Raw,
    Calibrate,
    View,
    Pole,
    Sound,
    Location,
    Tonight,
    CalPrev,
    CalNext,
    CalLess,
    CalMore,
    CalClose,
}

/// Whether the MENU list is open.
#[derive(Resource, Default)]
pub struct Menu {
    pub open: bool,
}

/// The bottom row.
#[derive(Component)]
pub struct Bar;
#[derive(Component)]
struct MenuPanel;
#[derive(Component)]
struct CalStrip;

/// Bottom offset of the controls in logical px; the row sits just above the
/// status line.
const BAR_BOTTOM: f32 = 36.0;
/// Width the page's own FIND button needs at the left of the bar (web only).
const PAGE_FIND_ROOM: f32 = 62.0;

/// Thin palette-coloured outline over a faint dark backing, like a terminal key.
fn button(action: Action, label: &str) -> impl Bundle {
    (
        action,
        Button,
        Node { border: UiRect::all(px(1)), ..default() },
        BorderColor::all(HOLO.with_alpha(0.5)),
        TintedBorder(0.5),
        BackgroundColor(Color::BLACK.with_alpha(0.35)),
        children![(
            Text::new(label),
            TextFont { font_size: FontSize::Px(13.0), ..default() },
            TextColor(HOLO),
            Tinted(1.0),
        )],
    )
}

fn spawn_controls(mut commands: Commands) {
    let row = || Node {
        position_type: PositionType::Absolute,
        right: px(12),
        bottom: px(BAR_BOTTOM),
        column_gap: px(6),
        row_gap: px(6),
        // A very narrow phone wraps onto a second row rather than overflowing.
        flex_wrap: FlexWrap::Wrap,
        justify_content: JustifyContent::FlexEnd,
        ..default()
    };

    // In the browser the page has its own FIND button (only a real text field can
    // raise the on-screen keyboard); the row leaves room for it on the left.
    commands.spawn((Bar, row(), Visibility::Hidden)).with_children(|b| {
        if !cfg!(target_arch = "wasm32") {
            b.spawn(button(Action::Find, "FIND"));
        }
        b.spawn(button(Action::Locate, "LOCATE"));
        b.spawn(button(Action::View, "CHART"));
        b.spawn(button(Action::Raw, "RAW"));
        b.spawn(button(Action::Back, "BACK"));
        b.spawn(button(Action::Menu, "MENU"));
    });

    commands
        .spawn((
            MenuPanel,
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                bottom: px(BAR_BOTTOM + 44.0),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                padding: UiRect::all(px(6)),
                ..default()
            },
            // Nearly opaque: nothing behind the list should show through its text.
            BackgroundColor(Color::BLACK.with_alpha(0.9)),
            ZIndex(15),
            Visibility::Hidden,
        ))
        .with_children(|m| {
            for action in [
                Action::Location,
                Action::Tonight,
                Action::Pole,
                Action::Grid,
                Action::Figures,
                Action::Spin,
                Action::Fainter,
                Action::Brighter,
                Action::Palette,
                Action::Sound,
                Action::Calibrate,
            ] {
                m.spawn(button(action, ""));
            }
        });

    commands.spawn((CalStrip, row(), Visibility::Hidden)).with_children(|s| {
        s.spawn(button(Action::CalPrev, "< PREV"));
        s.spawn(button(Action::CalNext, "NEXT >"));
        s.spawn(button(Action::CalLess, " - "));
        s.spawn(button(Action::CalMore, " + "));
        s.spawn(button(Action::CalClose, "CLOSE"));
    });
}

/// Touch controls replace the keyboard help on touch screens and narrow windows.
pub fn touch_mode(state: &TouchState, window: &Window) -> bool {
    state.used || window.width() < 760.0
}

type Panels<'a> = (Has<Bar>, Has<MenuPanel>, Has<CalStrip>, &'a mut Visibility);
type AnyPanel = Or<(With<Bar>, With<MenuPanel>, With<CalStrip>)>;

fn show_controls(
    state: Res<TouchState>,
    tuner: Res<Tuner>,
    mut menu: ResMut<Menu>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut panels: Query<Panels, AnyPanel>,
) {
    let on = touch_mode(&state, &window);
    // The calibration strip takes over the bottom while the panel is open.
    let calibrating = on && tuner.open;
    if (!on || calibrating) && menu.open {
        menu.open = false;
    }
    for (bar, menu_panel, strip, mut vis) in &mut panels {
        let shown =
            (bar && on && !calibrating) || (menu_panel && menu.open) || (strip && calibrating);
        vis.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
    }
}

type Rows<'a> = (&'a mut Node, Has<Bar>);
type RowFilter = (Or<(With<Bar>, With<CalStrip>)>, Without<Action>);

/// The HUD shrinks with narrow windows, but buttons are for fingers, so they keep
/// the same real size on any screen (sizes here are logical px divided by UiScale).
fn size_buttons(
    scale: Res<UiScale>,
    mut rows: Query<Rows, RowFilter>,
    mut buttons: Query<(&mut Node, &Children), With<Action>>,
    mut fonts: Query<&mut TextFont>,
) {
    if !scale.is_changed() {
        return;
    }
    let s = scale.0.max(0.1);
    for (mut node, is_bar) in &mut rows {
        node.right = px(12.0 / s);
        node.bottom = px(BAR_BOTTOM / s);
        node.column_gap = px(6.0 / s);
        node.row_gap = px(6.0 / s);
        // Leave room for the page's FIND button at the left of the bar.
        node.left = if is_bar && cfg!(target_arch = "wasm32") {
            px((12.0 + PAGE_FIND_ROOM) / s)
        } else {
            Val::Auto
        };
    }
    for (mut node, children) in &mut buttons {
        node.padding = UiRect::axes(px(8.0 / s), px(7.0 / s));
        for child in children {
            if let Ok(mut font) = fonts.get_mut(*child) {
                font.font_size = FontSize::Px(13.0 / s);
            }
        }
    }
}

/// Top edge of the bottom controls in logical px, for things that must sit above
/// them (the MENU list, the lock readout).
pub fn controls_top(bar: &ComputedNode) -> f32 {
    BAR_BOTTOM + bar.size().y * bar.inverse_scale_factor()
}

/// Keeps the MENU list just above the bottom row, whatever its real height.
fn place_menu(
    scale: Res<UiScale>,
    bar: Single<&ComputedNode, With<Bar>>,
    mut panel: Single<&mut Node, (With<MenuPanel>, Without<Action>)>,
) {
    let s = scale.0.max(0.1);
    let bottom = px((controls_top(&bar) + 8.0) / s);
    if panel.bottom != bottom {
        panel.bottom = bottom;
        panel.right = px(12.0 / s);
        panel.padding = UiRect::all(px(6.0 / s));
        panel.row_gap = px(4.0 / s);
    }
}

fn on_off(on: bool) -> String {
    if on { "ON".into() } else { "OFF".into() }
}

/// The bar's toggles name what they switch to.
fn bar_labels(
    view: Res<SkyView>,
    look: Res<Look>,
    rows: Query<(&Action, &Children)>,
    mut texts: Query<&mut Text>,
) {
    if !view.is_changed() && !look.is_changed() {
        return;
    }
    for (action, children) in &rows {
        let label = match action {
            Action::View => {
                if view.chart_on {
                    "GLOBE"
                } else {
                    "CHART"
                }
            }
            Action::Raw => {
                if look.bypassed() {
                    "FX"
                } else {
                    "RAW"
                }
            }
            _ => continue,
        };
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child)
                && text.0 != label
            {
                text.0 = label.into();
            }
        }
    }
}

/// Menu rows show their current state, terminal style: "GRID ........ ON".
#[allow(clippy::too_many_arguments)]
fn menu_labels(
    menu: Res<Menu>,
    view: Res<SkyView>,
    orbit: Res<Orbit>,
    look: Res<Look>,
    sound: Res<SoundOn>,
    observer: Res<Observer>,
    rows: Query<(&Action, &Children)>,
    mut texts: Query<&mut Text>,
) {
    if !menu.open {
        return;
    }
    for (action, children) in &rows {
        let (name, value) = match action {
            Action::Grid => ("GRID", on_off(view.grid)),
            Action::Figures => ("FIGURES", on_off(view.figures)),
            Action::Spin => ("SPIN", on_off(orbit.auto_spin)),
            Action::Fainter => ("MORE STARS", format!("MAG {:.1}", view.mag_limit)),
            Action::Brighter => ("FEWER STARS", format!("MAG {:.1}", view.mag_limit)),
            Action::Palette => ("PALETTE", PALETTES[look.palette].label.to_string()),
            Action::Calibrate => ("CALIBRATE", ">".into()),
            Action::Pole => ("CHART CENTER", if view.south { "SOUTH" } else { "NORTH" }.into()),
            Action::Sound => ("SOUND", on_off(sound.0)),
            Action::Location => ("LOCATION", observer.place.map_or("SET >".into(), |p| p.label())),
            Action::Tonight => ("TONIGHT", on_off(observer.show && observer.place.is_some())),
            _ => continue,
        };
        let dots = ".".repeat(30usize.saturating_sub(name.len() + value.len()));
        let line = format!("{name} {dots} {value}");
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child)
                && text.0 != line
            {
                text.0 = line.clone();
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn press_buttons(
    buttons: Query<(&Interaction, &Action, &mut BackgroundColor), Changed<Interaction>>,
    selection: Res<Selection>,
    mut menu: ResMut<Menu>,
    mut tuner: ResMut<Tuner>,
    mut typing: ResMut<Typing>,
    mut search: ResMut<Search>,
    mut locate: ResMut<Locate>,
    mut orbit: ResMut<Orbit>,
    mut view: ResMut<SkyView>,
    mut look: ResMut<Look>,
    mut sound: ResMut<SoundOn>,
    mut observer: ResMut<Observer>,
) {
    for (interaction, action, mut bg) in buttons {
        bg.0 = match interaction {
            Interaction::Pressed => look.color().with_alpha(0.3),
            Interaction::Hovered => look.color().with_alpha(0.12),
            Interaction::None => Color::BLACK.with_alpha(0.35),
        };
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            Action::Find => open_box(&mut search, &mut typing),
            Action::Locate => locate.request = selection.selected,
            Action::Back => locate.back_to_chart(&mut orbit),
            Action::Menu => menu.open = !menu.open,
            Action::Fainter => view.mag_limit = (view.mag_limit + 0.5).min(MAG_MAX),
            Action::Brighter => view.mag_limit = (view.mag_limit - 0.5).max(MAG_MIN),
            Action::Grid => view.grid = !view.grid,
            Action::Figures => view.figures = !view.figures,
            Action::Spin => orbit.auto_spin = !orbit.auto_spin,
            Action::Palette => look.next_palette(),
            Action::Raw => look.toggle_bypass(),
            Action::View => toggle_chart(&mut view, &mut locate, &mut orbit),
            Action::Pole => view.south = !view.south,
            Action::Sound => sound.0 = !sound.0,
            Action::Location => {
                menu.open = false;
                if device_available() {
                    request_device();
                } else {
                    open_location(&mut search, &mut typing);
                }
            }
            Action::Tonight => observer.show = !observer.show,
            Action::Calibrate => {
                tuner.open = true;
                menu.open = false;
            }
            Action::CalPrev => tuner.select(-1),
            Action::CalNext => tuner.select(1),
            Action::CalLess => tuner.adjust(&mut look, -1.0),
            Action::CalMore => tuner.adjust(&mut look, 1.0),
            Action::CalClose => tuner.open = false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_finger_drags() {
        let (drag, zoom) = gesture(&[(Vec2::new(110.0, 50.0), Vec2::new(100.0, 52.0))]);
        assert_eq!(drag, Vec2::new(10.0, -2.0));
        assert_eq!(zoom, 1.0);
    }

    #[test]
    fn pinch_zooms() {
        // Fingers spreading apart: 100 px -> 200 px, so the camera moves in by half.
        let spread = [
            (Vec2::new(0.0, 0.0), Vec2::new(50.0, 0.0)),
            (Vec2::new(200.0, 0.0), Vec2::new(150.0, 0.0)),
        ];
        let (drag, zoom) = gesture(&spread);
        assert_eq!(drag, Vec2::ZERO);
        assert!((zoom - 0.5).abs() < 1e-6);

        let pinch = [
            (Vec2::new(50.0, 0.0), Vec2::new(0.0, 0.0)),
            (Vec2::new(150.0, 0.0), Vec2::new(200.0, 0.0)),
        ];
        assert!((gesture(&pinch).1 - 2.0).abs() < 1e-6);
    }

    #[test]
    fn degenerate_inputs_do_nothing() {
        assert_eq!(gesture(&[]), (Vec2::ZERO, 1.0));
        let same = [(Vec2::ONE, Vec2::ONE), (Vec2::ONE, Vec2::ONE)];
        assert_eq!(gesture(&same), (Vec2::ZERO, 1.0));
        let three = [(Vec2::ONE, Vec2::ZERO); 3];
        assert_eq!(gesture(&three), (Vec2::ZERO, 1.0));
    }
}
