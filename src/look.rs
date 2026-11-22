//! Colours, icons, type and small shared pieces of the UI.

use crate::scan::N;
use rune::{Color, Cursor, El, Icon, TextStyle, el, icon};
use rune_kit::{Kit, PaletteSeed};

pub fn rgb(hex: u32) -> Color {
    let lin = |v: u32| {
        let c = (v & 0xff) as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    Color { r: lin(hex >> 16), g: lin(hex >> 8), b: lin(hex), a: 1.0 }
}

pub struct Look {
    pub side: Color,
    pub bg: Color,
    pub card: Color,
    pub edge: Color,
    pub track: Color,
    pub track_dot: Color,
    pub fg: Color,
    pub fg2: Color,
    pub fg3: Color,
    pub rule: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub good: Color,
    pub warn: Color,
    pub hover: Color,
    pub selected: Color,
}

pub fn look() -> Look {
    Look {
        side: rgb(0x0f0f11),
        bg: rgb(0x151517),
        card: rgb(0x1b1b1e),
        edge: rgb(0x2a2a2f),
        track: rgb(0x141416),
        track_dot: rgb(0x2b2b31),
        fg: rgb(0xf4f4f6),
        fg2: rgb(0xa2a2ab),
        fg3: rgb(0x6c6c76),
        rule: rgb(0x26262b),
        accent: rgb(0x4c8dff),
        on_accent: rgb(0xffffff),
        good: rgb(0x2fd158),
        warn: rgb(0xf5b524),
        hover: Color::WHITE.alpha(0.05),
        selected: Color::WHITE.alpha(0.08),
    }
}

pub fn kit(l: &Look) -> Kit {
    Kit::from_palette(PaletteSeed { bg: l.bg, fg: l.fg, accent: l.accent })
}

// ---- type ----

pub const NUM: &str = "'tnum' 1";
pub fn ui(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight)
}
pub fn num(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).features(NUM)
}

// ---- categories ----

pub struct Category {
    pub name: &'static str,
    pub color: u32,
    pub icon: Icon,
}

const fn glyph(name: &'static str, body: &'static str) -> Icon {
    Icon { name, view: 16.0, body }
}

/// In bar order, which is also the legend's reading order.
pub static CATEGORIES: [Category; N] = [
    Category { name: "Apps", color: 0x2f8fff, icon: glyph("st-apps", r#"<rect x="2" y="2" width="5" height="5" rx="1.2" fill="currentColor"/><rect x="9" y="2" width="5" height="5" rx="1.2" fill="currentColor"/><rect x="2" y="9" width="5" height="5" rx="1.2" fill="currentColor"/><rect x="9" y="9" width="5" height="5" rx="1.2" fill="currentColor"/>"#) },
    Category { name: "Developer", color: 0x2fd158, icon: glyph("st-dev", r#"<rect x="1.8" y="2.5" width="12.4" height="11" rx="2.2" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M4.8 6.5l2 1.7-2 1.7M8.6 10h2.6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#) },
    Category { name: "Photos", color: 0xff4f9a, icon: glyph("st-photos", r#"<rect x="1.8" y="2.8" width="12.4" height="10.4" rx="2.2" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M2.5 11.5l3.3-3.4 2.6 2.4 2-1.8 3.2 3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><circle cx="10.6" cy="6" r="1" fill="currentColor"/>"#) },
    Category { name: "Movies", color: 0xa45cff, icon: glyph("st-movies", r#"<rect x="1.8" y="2.5" width="12.4" height="11" rx="2.2" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M5.2 2.5v11M10.8 2.5v11M1.8 8h3.4M10.8 8h3.4" fill="none" stroke="currentColor" stroke-width="1.5"/>"#) },
    Category { name: "Documents", color: 0x2ec4d6, icon: glyph("st-docs", r#"<path d="M4.3 1.8h4.9l3 3v8.2a1.2 1.2 0 0 1-1.2 1.2H4.3A1.2 1.2 0 0 1 3.1 13V3a1.2 1.2 0 0 1 1.2-1.2z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M5.8 8h4.4M5.8 10.8h4.4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#) },
    Category { name: "Music", color: 0xf5c518, icon: glyph("st-music", r#"<path d="M6 12V3.6l6.5-1.4V10.6" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><circle cx="4.4" cy="12" r="1.7" fill="currentColor"/><circle cx="10.9" cy="10.6" r="1.7" fill="currentColor"/>"#) },
    Category { name: "System data", color: 0xbec1ca, icon: glyph("st-system", r#"<circle cx="8" cy="8" r="2.2" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M8 1.6v2M8 12.4v2M1.6 8h2M12.4 8h2M3.5 3.5l1.4 1.4M11.1 11.1l1.4 1.4M3.5 12.5l1.4-1.4M11.1 4.9l1.4-1.4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>"#) },
    Category { name: "Caches", color: 0xff8a1f, icon: glyph("st-caches", r#"<path d="M8 1.8l5.8 3-5.8 3-5.8-3z" fill="currentColor"/><path d="M2.2 8.2L8 11.2l5.8-3M2.2 11.2l5.8 3 5.8-3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#) },
    Category { name: "Trash", color: 0xff4545, icon: glyph("st-trash", r#"<path d="M2.5 4.2h11M6.2 4.2V2.6h3.6v1.6M3.8 4.2l.7 8.6a1.2 1.2 0 0 0 1.2 1.1h4.6a1.2 1.2 0 0 0 1.2-1.1l.7-8.6M6.6 7v4M9.4 7v4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#) },
];

pub fn color_of(category: usize) -> Color {
    rgb(CATEGORIES[category].color)
}

// ---- icons ----

macro_rules! stroked {
    ($name:literal, $body:literal) => {
        Icon { name: $name, view: 16.0, body: concat!("<g ", r#"fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round""#, ">", $body, "</g>") }
    };
}

pub static RESCAN: Icon = stroked!("st-rescan", r#"<path d="M13.5 8a5.5 5.5 0 0 1-9.6 3.7M2.5 8a5.5 5.5 0 0 1 9.6-3.7M12.4 1.8v2.7H9.7M3.6 14.2v-2.7h2.7"/>"#);
pub static RING: Icon = glyph("st-ring", r#"<circle cx="8" cy="8" r="6" stroke="currentColor" stroke-width="1.8" fill="none"/>"#);
pub static ARC: Icon = glyph("st-arc", r#"<path d="M8 2A6 6 0 0 1 13.258 10.89" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" fill="none"/>"#);
pub static CHECK_CIRCLE: Icon = stroked!("st-check-circle", r#"<circle cx="8" cy="8" r="6.3"/><path d="M5.3 8.2l1.9 1.9 3.6-4"/>"#);
pub static CHECK: Icon = Icon { name: "st-check", view: 16.0, body: r#"<path d="M3.6 8.4l2.9 2.9 5.9-6.4" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>"# };
pub static OVERVIEW: Icon = stroked!("st-nav-overview", r#"<circle cx="8" cy="8" r="6.2"/><path d="M8 1.8V8l4.4 4.4"/>"#);
pub static CLEANER: Icon = stroked!("st-nav-cleaner", r#"<path d="M8 1.8l1.3 3.4 3.4 1.3-3.4 1.3L8 11.2 6.7 7.8 3.3 6.5l3.4-1.3z"/><path d="M12.6 10.4l.6 1.5 1.5.6-1.5.6-.6 1.5-.6-1.5-1.5-.6 1.5-.6z"/>"#);
pub static FILES: Icon = stroked!("st-nav-files", r#"<path d="M5.5 2h4.2l2.8 2.8v7.4a1 1 0 0 1-1 1H5.5a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1z"/><path d="M2.5 5v8.6a1.4 1.4 0 0 0 1.4 1.4h5.6"/>"#);
pub static EXPLORE: Icon = stroked!("st-nav-explore", r#"<path d="M1.8 4.2a1.2 1.2 0 0 1 1.2-1.2h3.2l1.5 1.6H13a1.2 1.2 0 0 1 1.2 1.2v6.4a1.2 1.2 0 0 1-1.2 1.2H3a1.2 1.2 0 0 1-1.2-1.2z"/>"#);
pub static FOLDER: Icon = Icon { name: "st-folder", view: 16.0, body: r#"<path d="M1.6 4.2A1.3 1.3 0 0 1 2.9 2.9h3.4l1.6 1.7h5.2a1.3 1.3 0 0 1 1.3 1.3v6.3a1.3 1.3 0 0 1-1.3 1.3H2.9a1.3 1.3 0 0 1-1.3-1.3z" fill="currentColor" fill-opacity=".22" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>"# };
pub static CHEVRON: Icon = stroked!("st-chevron", r#"<path d="M6 3.5l4.5 4.5L6 12.5"/>"#);
pub static CHEVRON_DOWN: Icon = stroked!("st-chevron-down", r#"<path d="M3.5 6l4.5 4.5L12.5 6"/>"#);
pub static REVEAL: Icon = stroked!("st-reveal", r#"<path d="M9.5 2.5h4v4M13.5 2.5L7.5 8.5"/><path d="M12 9.5v3a1 1 0 0 1-1 1H3.5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1h3"/>"#);
pub static LOCK: Icon = stroked!("st-lock", r#"<rect x="3.2" y="7" width="9.6" height="7" rx="1.6"/><path d="M5.3 7V5.1a2.7 2.7 0 0 1 5.4 0V7"/>"#);
pub static WARN: Icon = stroked!("st-warn", r#"<path d="M8 2.2l6.2 11H1.8z"/><path d="M8 6.6v3M8 11.6v.1"/>"#);
pub static DISK: Icon = stroked!("st-disk", r#"<rect x="1.8" y="4" width="12.4" height="8" rx="1.8"/><path d="M4.5 9.5h.1M7 9.5h4.5"/>"#);
pub static SPARKLE: Icon = Icon { name: "st-sparkle", view: 16.0, body: r#"<path d="M8 0.5l1.9 5.6 5.6 1.9-5.6 1.9L8 15.5 6.1 9.9 0.5 8l5.6-1.9z" fill="currentColor"/>"# };

// ---- controls ----

/// A 16 pt box in a 24 pt hit area. The caller adds `on_press`.
pub fn checkbox(on: bool, mixed: bool, name: &str, l: &Look) -> El {
    let filled = on || mixed;
    let mut b = el()
        .size(16.0, 16.0)
        .radius(4.5)
        .center()
        .fill(if filled { l.accent } else { Color::TRANSPARENT })
        .border(1.5, if filled { l.accent } else { l.fg3 });
    if on {
        b = b.child(icon(&CHECK, 12.0).color(l.on_accent));
    } else if mixed {
        b = b.child(el().size(8.0, 2.0).radius(1.0).fill(l.on_accent));
    }
    let hit = el().role("checkbox").label(name).focusable(true).size(24.0, 24.0).center().radius(6.0).hover(l.hover).cursor(Cursor::Pointer);
    let hit = if mixed { hit.checked_mixed() } else { hit.checked(on) };
    hit.child(b)
}

/// A small square button holding an icon.
pub fn icon_button(i: &'static Icon, name: &str, l: &Look) -> El {
    el().role("button")
        .label(name)
        .help(name)
        .focusable(true)
        .size(28.0, 28.0)
        .radius(7.0)
        .center()
        .hover(Color::WHITE.alpha(0.08))
        .cursor(Cursor::Pointer)
        .child(icon(i, 15.0).color(l.fg3))
}

/// The app's mark, drawn like its icon: a dark tile, a three-colour capacity bar and a sparkle.
pub fn logo(size: f32) -> El {
    let k = size / 24.0;
    let seg = |x: f32, w: f32, c: u32| el().absolute(x * k, 12.0 * k).size(w * k, 7.0 * k).radius(1.6 * k).fill(rgb(c));
    el().size(size, size)
        .radius(6.5 * k)
        .gradient(rgb(0x182c38), rgb(0x0a1016))
        .border(1.0, Color::WHITE.alpha(0.12))
        .child(el().absolute(3.5 * k, 12.0 * k).size(17.0 * k, 7.0 * k).radius(1.8 * k).fill(Color::WHITE.alpha(0.09)))
        .child(seg(3.5, 5.0, 0x2f8fff))
        .child(seg(9.0, 4.2, 0xff4f9a))
        .child(seg(13.7, 3.0, 0xff8a1f))
        .child(icon(&SPARKLE, 9.0 * k).absolute(13.0 * k, 2.5 * k).color(rgb(0x7de8c8)))
}

pub fn spinner(l: &Look, size: f32) -> El {
    el().size(size, size)
        .child(icon(&RING, size).absolute(0.0, 0.0).color(l.fg3).opacity(0.25))
        .child(icon(&ARC, size).absolute(0.0, 0.0).color(l.fg3).looping(rune::LoopSpec::rotation(0.9)))
}

// ---- numbers ----

fn unit() -> f64 {
    // Windows counts a GB as 2^30 bytes; macOS and most Linux file managers as 10^9.
    if cfg!(windows) { 1024.0 } else { 1000.0 }
}

/// Always in GB with one decimal, for the overview's totals; MB below 0.1 GB.
pub fn gb(bytes: f64) -> String {
    let g = unit().powi(3);
    if bytes > 0.0 && bytes < g * 0.1 { format!("{:.0} MB", bytes / g * unit()) } else { format!("{:.1} GB", bytes / g) }
}

pub fn whole_gb(bytes: f64) -> String {
    format!("{:.0} GB", bytes / unit().powi(3))
}

/// The unit that reads best, for rows.
pub fn size(bytes: u64) -> String {
    let b = bytes as f64;
    let k = unit();
    if b >= k * k * k * 0.95 {
        format!("{:.1} GB", b / (k * k * k))
    } else if b >= k * k * 10.0 {
        format!("{:.0} MB", b / (k * k))
    } else if b >= k * k * 0.95 {
        format!("{:.1} MB", b / (k * k))
    } else if b >= k {
        format!("{:.0} KB", b / k)
    } else if b > 0.0 {
        "1 KB".into()
    } else {
        "Zero KB".into()
    }
}

pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{} {}", thousands(n), if n == 1 { one } else { many })
}

pub fn ago(now: u64, then: u64) -> String {
    if then == 0 {
        return String::new();
    }
    let days = now.saturating_sub(then) / 86_400;
    match days {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        2..=29 => format!("{days} days ago"),
        30..=364 => {
            let m = (days / 30).max(1);
            if m == 1 { "1 month ago".into() } else { format!("{m} months ago") }
        }
        _ => {
            let y = days / 365;
            if y == 1 { "1 year ago".into() } else { format!("{y} years ago") }
        }
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}
