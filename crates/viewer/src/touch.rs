//! Touch screens: one finger rotates, two fingers pinch to zoom, a quick tap selects
//! a star. Also the on-screen button bar that stands in for keyboard shortcuts on
//! touch and narrow screens.

use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::{Orbit, apply_orbit, over_ui};
use crate::locate::Locate;
use crate::look::{Look, PALETTES, Tinted};
use crate::picking::{Selection, TAP_RADIUS_PX, Targets, pick_at};
use crate::search::{Search, open_box};
use crate::sky::{MAG_MAX, MAG_MIN, SkyView};
use crate::{AppState, HOLO, Typing};

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchState>()
            .add_systems(Startup, spawn_bar)
            .add_systems(
                Update,
                (gestures.before(apply_orbit), show_bar, size_buttons, press_buttons),
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
    Fainter,
    Brighter,
    Palette,
}

#[derive(Component)]
struct Bar;

fn spawn_bar(mut commands: Commands) {
    let mut actions = vec![
        (Action::Locate, "LOCATE"),
        (Action::Back, "BACK"),
        (Action::Brighter, "MAG -"),
        (Action::Fainter, "MAG +"),
        (Action::Palette, "PALETTE"),
    ];
    // In the browser the page has its own FIND button, which can raise the
    // on-screen keyboard; the canvas can't.
    if !cfg!(target_arch = "wasm32") {
        actions.insert(0, (Action::Find, "FIND"));
    }
    commands
        .spawn((
            Bar,
            // Full width (a definite width lets the rows wrap and size correctly),
            // right-aligned, sitting above the status line.
            Node {
                position_type: PositionType::Absolute,
                left: px(12),
                right: px(12),
                bottom: px(48),
                column_gap: px(6),
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::FlexEnd,
                align_content: AlignContent::FlexEnd,
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|bar| {
            for (action, label) in actions {
                bar.spawn((
                    action,
                    Button,
                    Node {
                        padding: UiRect::axes(px(12), px(8)),
                        margin: UiRect::top(px(6)),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BorderColor::all(Color::WHITE.with_alpha(0.35)),
                    BackgroundColor(Color::BLACK.with_alpha(0.45)),
                ))
                .with_child((
                    Text::new(label),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                    TextColor(HOLO),
                    Tinted(1.0),
                ));
            }
        });
}

/// The bar replaces the keyboard help on touch screens and narrow windows.
pub fn touch_mode(state: &TouchState, window: &Window) -> bool {
    state.used || window.width() < 760.0
}

fn show_bar(
    state: Res<TouchState>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut bar: Single<&mut Visibility, With<Bar>>,
) {
    let vis = if touch_mode(&state, &window) { Visibility::Inherited } else { Visibility::Hidden };
    bar.set_if_neq(vis);
}

/// The rest of the HUD shrinks with the window, but buttons are for fingers, so
/// they keep the same real size (about 15 px text, 40 px tall) on any screen.
fn size_buttons(
    scale: Res<UiScale>,
    mut bar: Single<&mut Node, (With<Bar>, Without<Action>)>,
    mut buttons: Query<(&mut Node, &Children), With<Action>>,
    mut fonts: Query<&mut TextFont>,
) {
    if !scale.is_changed() {
        return;
    }
    let s = scale.0.max(0.1);
    // In the browser the page's own FIND button sits at the left end of the bottom
    // row (about 100 px wide), so leave it room.
    bar.left = px(if cfg!(target_arch = "wasm32") { 104.0 / s } else { 12.0 });
    for (mut node, children) in &mut buttons {
        node.padding = UiRect::axes(px(11.0 / s), px(8.0 / s));
        node.margin = UiRect::top(px(6.0 / s));
        for child in children {
            if let Ok(mut font) = fonts.get_mut(*child) {
                font.font_size = FontSize::Px(14.0 / s);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn press_buttons(
    buttons: Query<(&Interaction, &Action, &mut BackgroundColor), Changed<Interaction>>,
    selection: Res<Selection>,
    mut typing: ResMut<Typing>,
    mut search: ResMut<Search>,
    mut locate: ResMut<Locate>,
    mut orbit: ResMut<Orbit>,
    mut view: ResMut<SkyView>,
    mut look: ResMut<Look>,
) {
    for (interaction, action, mut bg) in buttons {
        bg.0 = match interaction {
            Interaction::Pressed => HOLO.with_alpha(0.3),
            Interaction::Hovered => HOLO.with_alpha(0.12),
            Interaction::None => Color::BLACK.with_alpha(0.45),
        };
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            Action::Find => open_box(&mut search, &mut typing),
            Action::Locate => locate.request = selection.selected,
            Action::Back => locate.back_to_chart(&mut orbit),
            Action::Fainter => view.mag_limit = (view.mag_limit + 0.5).min(MAG_MAX),
            Action::Brighter => view.mag_limit = (view.mag_limit - 0.5).max(MAG_MIN),
            Action::Palette => look.palette = (look.palette + 1) % PALETTES.len(),
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
