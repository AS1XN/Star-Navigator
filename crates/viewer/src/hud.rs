//! On-screen readouts: status line, key help and the hover tag.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::data::{LoadError, Sky};
use crate::look::Tinted;
use crate::picking::Selection;
use crate::sky::SkyView;
use crate::touch::{TouchState, touch_mode};
use crate::{AppState, HOLO};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, fit_to_window)
            .add_systems(Update, loading_text.run_if(in_state(AppState::Loading)))
            .add_systems(OnEnter(AppState::Ready), |mut q: Query<&mut Text, With<Center>>| {
                for mut t in &mut q {
                    t.0.clear();
                }
            })
            .add_systems(Update, (status_text, hover_tag).run_if(in_state(AppState::Ready)));
    }
}

#[derive(Component)]
struct Center;
#[derive(Component)]
struct Status;
#[derive(Component)]
struct HoverTag;
#[derive(Component)]
struct Help;

/// Shrinks the HUD on narrow (phone) screens. The key help gives way to the touch
/// button bar on touch screens and narrow windows.
fn fit_to_window(
    window: Single<&Window, With<PrimaryWindow>>,
    touch: Res<TouchState>,
    mut help: Single<&mut Visibility, With<Help>>,
    mut scale: ResMut<UiScale>,
) {
    let hidden = touch_mode(&touch, &window);
    help.set_if_neq(if hidden { Visibility::Hidden } else { Visibility::Inherited });
    // Shrink a little for narrow windows, but not below a readable size on phones.
    let s = (window.width() / 1100.0).clamp(0.75, 1.0);
    if scale.0 != s {
        scale.0 = s;
    }
}

fn font(size: f32) -> TextFont {
    TextFont { font_size: FontSize::Px(size), ..default() }
}

fn dim() -> TextColor {
    TextColor(HOLO.with_alpha(0.6))
}

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_child((
            Center,
            Text::new("INITIALIZING STAR CHARTS..."),
            font(18.0),
            TextColor(HOLO),
            Tinted(1.0),
        ));
    commands.spawn((
        Status,
        Text::new("STAR-NAVIGATOR"),
        font(15.0),
        TextColor(HOLO),
        Tinted(1.0),
        Node { position_type: PositionType::Absolute, left: px(24), bottom: px(20), ..default() },
    ));
    commands.spawn((
        Help,
        Text::new(
            "DRAG ROTATE   WHEEL ZOOM   CLICK SELECT\n\
             [ ] MAG LIMIT   G GRID   C FIGURES   SPACE SPIN\n\
             P PALETTE   T TUNE LOOK   H RAW VIEW\n\
             / FIND   L LOCATE SELECTED   ESC BACK",
        ),
        font(12.0),
        dim(),
        Tinted(0.6),
        TextLayout::justify(Justify::Right),
        Node { position_type: PositionType::Absolute, right: px(24), bottom: px(20), ..default() },
    ));
    commands.spawn((
        HoverTag,
        Text::new(""),
        font(13.0),
        TextColor(HOLO),
        Tinted(1.0),
        Node { position_type: PositionType::Absolute, ..default() },
    ));
}

fn loading_text(error: Option<Res<LoadError>>, mut q: Single<&mut Text, With<Center>>) {
    if let Some(e) = error.and_then(|e| e.0.clone()) {
        q.0 = format!("SIGNAL LOST: {e}");
    }
}

fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn status_text(view: Res<SkyView>, sky: Res<Sky>, mut q: Single<&mut Text, With<Status>>) {
    if !view.is_changed() {
        return;
    }
    let shown = sky.catalog.base().partition_point(|s| s.mag <= view.mag_limit) - 1;
    q.0 =
        format!("STAR-NAVIGATOR // {} STARS // LIMIT MAG {:.1}", thousands(shown), view.mag_limit);
}

fn hover_tag(
    selection: Res<Selection>,
    sky: Res<Sky>,
    window: Single<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    mut tag: Single<(&mut Text, &mut Node), With<HoverTag>>,
) {
    let (text, node) = &mut *tag;
    match (selection.hovered, window.cursor_position()) {
        (Some(i), Some(cursor)) => {
            let star = &sky.catalog.stars()[i];
            text.0 = format!("{}  {:+.2}", star.display_name().to_uppercase(), star.mag);
            // UI units are scaled by UiScale; cursor coordinates are not.
            node.left = px((cursor.x + 16.0) / scale.0);
            node.top = px((cursor.y - 22.0) / scale.0);
        }
        _ => text.0.clear(),
    }
}
