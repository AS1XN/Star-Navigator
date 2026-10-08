//! The locate sequence: the globe unfolds into the stars' true 3D positions around
//! Sol while the camera flies to the target, then locks on. L locates the selected
//! star, Esc returns to the chart.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::{Orbit, OrbitGoal, heading_toward};
use crate::data::Sky;
use crate::look::Tinted;
use crate::picking::{Selection, reticle};
use crate::sky::SkyView;
use crate::touch::{Bar, Menu, TouchState, controls_top, touch_mode};
use crate::{
    AppState, FIELD_SCALE, HOLO, field_position, globe_position, hotkeys_enabled, star_world,
};

pub struct LocatePlugin;

impl Plugin for LocatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Locate>().add_systems(Startup, spawn_status).add_systems(
            Update,
            (
                locate_input.run_if(hotkeys_enabled),
                start_locate,
                animate,
                draw_field,
                status_text,
                place_status,
            )
                .chain()
                .run_if(in_state(AppState::Ready)),
        );
    }
}

/// Seconds for the globe to unfold into the field (or fold back).
const UNFOLD_SECS: f32 = 2.4;

#[derive(Resource, Default)]
pub struct Locate {
    /// Set by search or the L key; consumed by `start_locate`.
    pub request: Option<usize>,
    target: Option<usize>,
    progress: f32,
    into_field: bool,
    /// Shown when a star can't be placed in 3D.
    note: Option<String>,
}

impl Locate {
    /// Shows (or clears) a one-line message in the lock readout, e.g. lookup status.
    pub fn set_note(&mut self, note: Option<String>) {
        self.note = note;
    }

    /// The star currently being shown by a locate, if any.
    pub fn current(&self) -> Option<usize> {
        self.target.filter(|_| self.into_field || self.note.is_some())
    }
}

pub fn locate_input(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    mut locate: ResMut<Locate>,
    mut orbit: ResMut<Orbit>,
) {
    if keys.just_pressed(KeyCode::KeyL) {
        locate.request = selection.selected;
    }
    if keys.just_pressed(KeyCode::Escape) {
        locate.back_to_chart(&mut orbit);
    }
}

impl Locate {
    /// Folds the field back into the globe (Esc or the BACK button).
    pub fn back_to_chart(&mut self, orbit: &mut Orbit) {
        if self.into_field || self.note.is_some() {
            self.into_field = false;
            self.target = None;
            self.note = None;
            orbit.goal = OrbitGoal::globe();
        }
    }
}

fn start_locate(
    mut locate: ResMut<Locate>,
    mut orbit: ResMut<Orbit>,
    mut selection: ResMut<Selection>,
    mut view: ResMut<SkyView>,
    sky: Res<Sky>,
) {
    let Some(index) = locate.request.take() else { return };
    // Locating always lifts the chart back into the globe first.
    view.chart_on = false;
    let star = &sky.catalog.stars()[index];
    selection.selected = Some(index);
    locate.target = Some(index);
    match field_position(star) {
        Some(pos) => {
            locate.into_field = true;
            locate.note = None;
            orbit.goal = OrbitGoal {
                focus: orbit.focus,
                // Far stars get a wider view so their neighbourhood fits on screen.
                distance: (0.3 + 0.25 * pos.length()).min(10.0),
                heading: None,
                min_distance: 0.02,
                max_distance: 150.0,
            };
        }
        None => {
            // No parallax: stay on the chart and turn the globe to face the star.
            locate.into_field = false;
            locate.note = Some(format!(
                "{} // NO DISTANCE ON FILE // CHART POSITION ONLY",
                star.display_name().to_uppercase()
            ));
            orbit.goal = OrbitGoal {
                heading: Some(heading_toward(globe_position(star))),
                ..OrbitGoal::globe()
            };
        }
    }
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn animate(
    time: Res<Time>,
    sky: Res<Sky>,
    mut locate: ResMut<Locate>,
    mut view: ResMut<SkyView>,
    mut orbit: ResMut<Orbit>,
) {
    let step = time.delta_secs() / UNFOLD_SECS;
    let goal = if locate.into_field { 1.0 } else { 0.0 };
    if locate.progress != goal {
        locate.progress = if goal > locate.progress {
            (locate.progress + step).min(1.0)
        } else {
            (locate.progress - step).max(0.0)
        };
        view.unfold = smoothstep(locate.progress);
    }
    // While the field unfolds the target is still moving, so keep chasing it.
    let chasing = locate.target.filter(|_| locate.into_field);
    if let Some(pos) = chasing.and_then(|i| star_world(&sky.catalog.stars()[i], &view)) {
        orbit.goal.focus = pos;
    }
}

/// Distance rings (5 to 100 pc) around Sol and a bearing line to the target.
fn draw_field(
    view: Res<SkyView>,
    locate: Res<Locate>,
    sky: Res<Sky>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut gizmos: Gizmos,
) {
    if view.unfold < 0.01 {
        return;
    }
    let holo = LinearRgba::from(HOLO);
    let a = view.unfold;
    let flat = Isometry3d::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    for pc in [5.0, 10.0, 25.0, 50.0, 100.0] {
        gizmos.circle(flat, pc * FIELD_SCALE, holo * 0.18 * a);
    }
    let eye = camera.translation();
    reticle(
        &mut gizmos,
        eye,
        Vec3::ZERO,
        0.01,
        (holo * 0.8 * a).into(),
        std::f32::consts::FRAC_PI_4,
    );
    if let Some(pos) = locate.target.and_then(|i| star_world(&sky.catalog.stars()[i], &view)) {
        gizmos.line(Vec3::ZERO, pos, holo * 0.35 * a);
    }
}

#[derive(Component)]
struct Status;

#[derive(Component)]
struct StatusRow;

fn spawn_status(mut commands: Commands) {
    commands
        .spawn((
            StatusRow,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                bottom: px(56),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_child((
            Status,
            crate::look::backed(5),
            Node { padding: UiRect::axes(px(10), px(4)), ..default() },
            Text::new(""),
            TextFont { font_size: FontSize::Px(15.0), ..default() },
            TextColor(HOLO),
            Tinted(1.0),
            TextLayout::justify(Justify::Center),
        ));
}

fn status_text(
    locate: Res<Locate>,
    view: Res<SkyView>,
    orbit: Res<Orbit>,
    sky: Res<Sky>,
    time: Res<Time>,
    mut text: Single<&mut Text, With<Status>>,
) {
    let line = if let Some(note) = &locate.note {
        format!("{note}\nESC RETURN")
    } else if let (true, Some(i)) = (locate.into_field, locate.target) {
        let star = &sky.catalog.stars()[i];
        let name = star.display_name().to_uppercase();
        let ly = star.dist_ly().unwrap_or_default();
        let settled = view.unfold > 0.999 && orbit.focus.distance(orbit.goal.focus) < 0.002;
        if settled {
            format!("TARGET LOCKED // {name} // {ly:.1} LY FROM SOL\nESC RETURN TO CHART")
        } else {
            let dots = ".".repeat(1 + (time.elapsed_secs() * 3.0) as usize % 3);
            format!("LOCATING {name}{dots}")
        }
    } else {
        String::new()
    };
    if text.0 != line {
        text.0 = line;
    }
}

/// Keeps the lock readout above the touch button bar when that's showing.
fn place_status(
    touch: Res<TouchState>,
    scale: Res<UiScale>,
    window: Single<&Window, With<PrimaryWindow>>,
    menu: Res<Menu>,
    bar: Single<&ComputedNode, With<Bar>>,
    mut row: Single<&mut Node, (With<StatusRow>, Without<Bar>)>,
) {
    // Measured from the bar's real height, so it clears the controls on any screen.
    let bottom = if touch_mode(&touch, &window) {
        px((controls_top(&bar) + 10.0) / scale.0.max(0.1))
    } else {
        px(56)
    };
    if row.bottom != bottom {
        row.bottom = bottom;
    }
    // The MENU list opens over this spot; step aside until it closes.
    let display = if menu.open { Display::None } else { Display::Flex };
    if row.display != display {
        row.display = display;
    }
}
