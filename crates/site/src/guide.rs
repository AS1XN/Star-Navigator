//! The user guide: what every feature does and how to reach it with a mouse and
//! keyboard or on a touch screen. Keep it in step with the viewer: any new
//! feature, key or button gets a line here.

use stucco::Raw;
use stucco::prelude::*;

use crate::site_page;

fn section(title: &'static str, paragraphs: &[&'static str]) -> Stack<'static> {
    let mut s = Stack::new().space(Space::S2).child(Heading::new(2, title));
    for p in paragraphs {
        s = s.child(Text::new(*p));
    }
    s
}

/// Two columns only, so the table fits a phone screen without scrolling sideways.
fn controls_table(caption: &'static str, input: &'static str, phone: bool) -> Table<'static> {
    let mut table = Table::new(caption).header(Row::new().header("Action").header(input));
    for (action, desktop, touch) in CONTROLS {
        table = table.row(Row::new().cell(*action).cell(if phone { *touch } else { *desktop }));
    }
    table
}

const CONTROLS: &[(&str, &str, &str)] = &[
    ("Rotate the view", "Drag with the mouse", "Drag with one finger"),
    ("Zoom", "Mouse wheel", "Pinch with two fingers"),
    ("Select a star", "Click it", "Tap it"),
    ("Find a star", "/ or Enter", "FIND"),
    ("Fly to the selected star", "L", "LOCATE"),
    ("Return to the whole sky", "Esc", "BACK"),
    ("Flat chart / globe", "V", "CHART / GLOBE"),
    ("Screen effects off / on", "H", "RAW / FX"),
    ("Centre the chart on the other pole", "N", "MENU > CHART CENTER"),
    ("Coordinate grid", "G", "MENU > GRID"),
    ("Constellation figures", "C", "MENU > FIGURES"),
    ("Slow automatic spin", "Space", "MENU > SPIN"),
    ("Show more / fewer stars", "] and [ (or = and -)", "MENU > MORE / FEWER STARS"),
    ("Change colour", "P", "MENU > PALETTE"),
    ("Sound on / off", "M", "MENU > SOUND"),
    ("Tune the screen effects", "T", "MENU > CALIBRATE"),
];

pub fn guide(bundle: &Bundle) -> String {
    let body = Stack::new()
        .space(Space::S5)
        .child(Heading::new(1, "User guide"))
        .child(Text::new(
            "Star-Navigator is a map of the real night sky. Every dot is a real star in its real \
             direction, and for most of them at their real distance from the Sun. You can spin \
             the sky, look any star up, fly out to it in 3D and read what kind of star it is.",
        ))
        .child(Text::new(
            "It works best on a computer with a mouse and keyboard and a reasonably recent \
             browser. Phones and tablets work too, with on-screen buttons in place of the keys, \
             but the first load is large (about 11 MB) and small screens show less detail.",
        ))
        .child(section(
            "Quick start",
            &[
                "1. Drag to turn the globe and scroll (or pinch) to zoom. Each dot is a star; \
                 brighter stars are bigger.",
                "2. Click or tap a star to select it. A data plate with its details appears at \
                 the top left.",
                "3. Press L or the LOCATE button. The globe unfolds into a 3D field of stars and \
                 the camera flies out to the one you picked. Press Esc or BACK to come home.",
                "4. To find a particular star, press / (or FIND on a phone) and start typing \
                 its name, for example Sirius, Vega or Betelgeuse.",
            ],
        ))
        .child(section(
            "The globe",
            &[
                "The starting view is the whole sky drawn on a sphere, seen from outside, as if \
                 Earth were at its centre. The bright ring is the celestial equator, the faint \
                 lines are the coordinate grid (right ascension and declination), and the stick \
                 figures are the constellations. Stars on the far side of the globe are dimmed \
                 so the near side reads clearly.",
                "By default the map shows stars down to magnitude 6.5, roughly what the naked \
                 eye sees on a dark night (about 8,900 stars). The status line at the bottom \
                 shows the current count and limit. Show more stars to go fainter, up to the \
                 full catalog of almost 120,000.",
                "Hovering over a star with the mouse shows its name and brightness. When left \
                 alone for a few seconds the globe starts a slow spin; turn that off with Space \
                 or MENU > SPIN.",
            ],
        ))
        .child(section(
            "Finding a star",
            &[
                "Open FIND with / or Enter on a computer, or the FIND button on a phone, and \
                 type. Up to eight matches appear as you type; use the arrow keys and Enter, or \
                 click or tap a result, to fly to that star.",
                "You can search by proper name (Vega, Polaris), by Bayer designation (Alpha \
                 Lyrae or alf Lyr), by Flamsteed number (61 Cyg), or by catalog number (HIP \
                 91262, HD 172167, HR 7001, Gliese 699). Partial names work, and accents are \
                 optional.",
                "If a star is not in the built-in catalog, choose QUERY SIMBAD ONLINE (or press \
                 Enter when there are no matches). Star-Navigator then asks the SIMBAD \
                 astronomical database, adds the object to the map with a diamond marker and \
                 flies to it. This needs an internet connection and works for objects such as \
                 TRAPPIST-1 or Kepler-452.",
            ],
        ))
        .child(section(
            "Locating a star",
            &[
                "LOCATE (or L) takes you to the selected star. The globe unfolds: every star \
                 moves from its spot on the sphere to its true position in space, so the sky \
                 becomes a 3D field around the Sun. The camera follows the target and shows \
                 TARGET LOCKED when it arrives.",
                "In this view, rings mark distances from the Sun (5 to 100 parsecs; one parsec \
                 is about 3.26 light years), a reticle marks the Sun itself, and a line shows \
                 the bearing from the Sun to the target. The nearest neighbours get name tags. \
                 You can still rotate and zoom around the target.",
                "Some stars have no measured distance. For those, the globe simply turns to \
                 face the star instead of unfolding.",
                "Esc or BACK returns to the whole-sky view.",
            ],
        ))
        .child(section(
            "The data plate",
            &[
                "Selecting a star opens a plate with its name, constellation, position (right \
                 ascension and declination), distance in light years and parsecs, apparent \
                 magnitude (how bright it looks from Earth) and absolute magnitude (how bright \
                 it really is), and its spectral class with a plain description such as RED \
                 SUPERGIANT.",
                "Where the data allows, it also estimates the surface temperature, how many \
                 times brighter and wider than the Sun the star is, and lists the three \
                 nearest stars. On larger screens a rotating wireframe model of the star sits \
                 below the plate, sized against a dashed outline of the Sun drawn in a \
                 contrasting colour (yellow on the red palette). The model is scaled by the \
                 fourth root of the radius so giants stay on screen; the plate gives the true \
                 figure.",
            ],
        ))
        .child(section(
            "Chart view",
            &["V or the CHART button flattens the globe onto a round briefing table: a flat \
                 sky chart with a celestial pole at the centre, declination rings, hour marks \
                 around the rim and a sector wedge pointing at the selected star. N (MENU > \
                 CHART CENTER on a phone) switches between the north and south pole. Press V \
                 or GLOBE to lift it back into a globe."],
        ))
        .child(section(
            "Look and screen effects",
            &[
                "The display imitates an old analog hologram: scanlines, flicker, grain, glow \
                 and colour fringing. RAW (H) turns all of that off for a clean view in plain \
                 blue-white, and FX (H again) turns it back on.",
                "P or MENU > PALETTE cycles the colour scheme: tactical red, holo blue, \
                 targeting amber and wireframe green.",
                "T or MENU > CALIBRATE opens the calibration panel, where each effect can be \
                 adjusted. On a computer use the arrow keys (Shift for bigger steps), R to \
                 reset; on a phone use PREV, NEXT, - and +.",
            ],
        ))
        .child(section(
            "Sound",
            &["Sound is on by default: soft console beeps when you select, locate and lock \
                 onto a star, a sweep when the chart folds and unfolds, and a quiet projector \
                 hum. Browsers only allow sound after your first click, tap or key press, so it \
                 starts then. M or MENU > SOUND turns it off. On an iPhone, the silent switch \
                 also mutes it."],
        ))
        .child(section(
            "Sharing and star pages",
            &[
                "Every named star has its own page that opens the map already flying to it, \
                 for example star/vega/. While you are locked onto a star the address bar \
                 updates to match, so you can copy the link and send it to someone.",
                "The CATALOG lists all 88 constellations and, for each, its named stars and \
                 everything brighter than magnitude 5.5, with links that open them on the map.",
            ],
        ))
        .child(Heading::new(2, "All controls"))
        // Let long control names wrap so the tables fit a phone without sideways scrolling.
        .child(Raw::trusted("<style>.st-table td, .st-table th { white-space: normal; }</style>"))
        .child(controls_table("On a computer", "Mouse or key", false))
        .child(controls_table("On a phone or tablet", "Gesture or button", true))
        .child(section(
            "If something goes wrong",
            &[
                "Stuck on INITIALIZING STAR CHARTS: the first visit downloads about 11 MB, \
                 which can take a while on a slow connection. The map needs WebGL2, which every \
                 current desktop and mobile browser supports; an old browser may show a black \
                 screen.",
                "No sound: click or tap the map once, check that SOUND is ON in the menu, and \
                 check the device volume or silent switch.",
                "A search finds nothing: try a different spelling or a catalog number, or use \
                 QUERY SIMBAD ONLINE for objects outside the built-in catalog.",
            ],
        ));
    site_page(
        bundle,
        "User guide",
        "How to use Star-Navigator: moving around, finding and locating stars, the data plate, \
         chart view, display options and every control.",
        "",
        body,
    )
}
