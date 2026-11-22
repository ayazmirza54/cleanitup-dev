//! The overview: the scan card with its glowing dot-matrix bar, then junk and largest files.

use crate::app::{MAX_COLS, Page, ROWS, State};
use crate::look::{self, CATEGORIES, Look, color_of, gb, num, ui, whole_gb};
use crate::scan::{N, TRASH};
use rune::{Color, Cursor, Cx, El, Family, LineCap, LoopSpec, Shape, Tx, canvas, col, el, icon, row, spacer, text};
use rune_kit::{Button, ButtonVariant, Kit};

const BAR_H: f32 = 78.0;
const PITCH: f32 = 6.0;

fn hover_handler(i: usize) -> impl Fn(&mut State, bool, &mut Tx) + 'static {
    move |s: &mut State, inside, _| {
        if inside && s.found(i) {
            s.hover = Some(i);
        } else if s.hover == Some(i) {
            s.hover = None;
        }
    }
}

/// Where each segment starts and ends, in points along a bar `w` wide.
fn bounds(s: &State, w: f32) -> [(f32, f32); N] {
    // A scan that counts more than the volume holds (hard links) still has to fit.
    let scale = w as f64 / s.capacity.max(s.total()).max(1.0);
    let mut x = 0.0;
    let mut out = [(0.0, 0.0); N];
    for i in 0..N {
        // Anything found gets a sliver, so a small category still shows.
        let wd = if s.shown[i] > 0.0 { ((s.shown[i] * scale) as f32).max(4.0) } else { 0.0 };
        out[i] = (x, x + wd);
        x += wd;
    }
    out
}

fn dots(s: &State, l: &Look, w: f32, segs: &[(f32, f32); N]) -> Shape {
    let cols = (((w - 8.0) / PITCH) as usize + 1).min(MAX_COLS);
    let ox = (w - (cols - 1) as f32 * PITCH) / 2.0;
    let oy = (BAR_H - (ROWS - 1) as f32 * PITCH) / 2.0;
    let mut shape = Shape::new();
    let mut seg = 0;
    for c in 0..cols {
        let x = ox + c as f32 * PITCH;
        while seg < N && x > segs[seg].1 {
            seg += 1;
        }
        let inside = seg < N && x >= segs[seg].0 + 3.0 && x <= segs[seg].1 - 3.0;
        let covered = seg < N && x >= segs[seg].0;
        let (base, light, dim, edge) = if inside {
            let base = color_of(seg);
            let near = (1.0 - (segs[seg].1 - x) / 26.0).max(0.0);
            (base, base.mix(Color::WHITE, 0.5), s.dim[seg], near * s.growing[seg])
        } else {
            (l.track_dot, l.track_dot, 1.0, 0.0)
        };
        for r in 0..ROWS {
            let y = oy + r as f32 * PITCH;
            if inside {
                // A cheap hash gives the leading edge its per-frame sparkle.
                let h = (c as u32).wrapping_mul(73_856_093) ^ (r as u32).wrapping_mul(19_349_663) ^ s.frame.wrapping_mul(83_492_791);
                let spark = edge * (0.5 + 0.5 * ((h >> 8) & 0xff) as f32 / 255.0);
                let v = s.dots[c * ROWS + r].max(spark);
                let ink = if v > 0.45 { light } else { base };
                shape = shape.line([x - 0.01, y], [x + 0.01, y], 1.6 + 0.9 * v, LineCap::Round, ink.alpha((0.2 + 0.8 * v) * dim));
            } else if !covered {
                shape = shape.line([x - 0.01, y], [x + 0.01, y], 1.6, LineCap::Round, base);
            }
        }
    }
    shape
}

fn bar(s: &State, l: &Look, w: f32) -> El {
    let segs = bounds(s, w);
    let mut bar = el().key("bar").w_full().h(BAR_H).radius(8.0).fill(l.track).border(1.0, l.edge).instant_changes();
    for (i, (x0, x1)) in segs.into_iter().enumerate() {
        let wd = x1 - x0;
        if wd < 3.0 {
            continue;
        }
        let c = color_of(i);
        let hot = s.growing[i];
        bar = bar.child(
            el().key(format!("seg-{i}"))
                .label(CATEGORIES[i].name)
                .absolute(x0 + 0.5, 1.0)
                .size(wd - 1.0, BAR_H - 2.0)
                .radius(7.0f32.min(wd / 2.0))
                .gradient(c.alpha(0.07), c.alpha(0.24))
                .border(1.5, c.mix(Color::WHITE, 0.45 * hot))
                .shadow(0.0, 16.0 + 10.0 * hot, c.alpha(0.7))
                .opacity(s.dim[i])
                .cursor(Cursor::Pointer)
                .on_hover_change(hover_handler(i))
                .on_press(move |s: &mut State| s.show_category(i)),
        );
    }
    bar.child(canvas(dots(s, l, w, &segs)).key("dots").absolute(0.0, 0.0).size(w, BAR_H).inert())
}

fn legend_cell(s: &State, l: &Look, i: usize) -> El {
    let c = &CATEGORIES[i];
    let found = s.found(i);
    let dimmed = !found || s.hover.is_some_and(|h| h != i);
    let cell = row()
        .key(format!("legend-{i}"))
        .grow(1.0)
        .min_w(0.0)
        .items_center()
        .gap(9.0)
        .px(6.0)
        .py(5.0)
        .radius(7.0)
        .fill(if s.hover == Some(i) { Color::WHITE.alpha(0.06) } else { Color::TRANSPARENT })
        .opacity(if dimmed { 0.4 } else { 1.0 })
        .on_hover_change(hover_handler(i))
        .child(icon(&c.icon, 15.0).color(color_of(i)))
        .child(text(c.name).font(ui(13.0, 400.0)).color(l.fg))
        .child(spacer())
        .child(text(gb(s.shown[i])).font(num(13.0, 400.0)).color(l.fg2));
    if s.ready() {
        cell.role("button").label(format!("{} files", c.name)).cursor(Cursor::Pointer).on_press(move |s: &mut State| s.show_category(i))
    } else {
        cell
    }
}

fn header(s: &State, l: &Look) -> El {
    let scanning = !s.ready();
    let status = if scanning {
        row()
            .gap(8.0)
            .items_center()
            .child(text("Calculating").font(ui(13.0, 400.0)).color(l.fg))
            .child(text(s.path.clone()).font(ui(12.0, 400.0).family(Family::SystemMono)).color(l.fg3).truncate_middle().grow(1.0).min_w(0.0))
    } else {
        let line = if s.demo {
            format!("Scanned {} files just now", look::thousands(s.files))
        } else {
            format!("Scanned {} files in {:.1} s", look::thousands(s.files), s.took)
        };
        row().child(text(line).font(ui(13.0, 400.0)).color(l.fg))
    };
    let ink = if scanning || s.busy() { l.fg3 } else { l.fg };
    let mut glyph = icon(&look::RESCAN, 14.0).color(ink);
    let mut rescan = row().key("rescan").role("button").label("Rescan").items_center().gap(7.0).px(8.0).py(6.0).radius(8.0);
    if scanning {
        glyph = glyph.looping(LoopSpec::rotation(1.1));
        rescan = rescan.disabled(true);
    } else {
        rescan = rescan.focusable(true).hover(Color::WHITE.alpha(0.07)).cursor(Cursor::Pointer).on_press(State::rescan);
    }
    row()
        .w_full()
        .items_start()
        .gap(12.0)
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(5.0)
                .child(
                    row()
                        .items_end()
                        .gap(7.0)
                        .child(text(gb(s.total())).key("total").font(num(26.0, 600.0).tracking(-0.4)).color(l.fg))
                        .child(text(format!("of {} used", whole_gb(s.capacity))).font(ui(14.0, 400.0)).color(l.fg2).pb(4.0)),
                )
                .child(status.w_full().h(18.0)),
        )
        .child(rescan.child(glyph).child(text("Rescan").font(ui(13.0, 500.0)).color(ink)))
}

fn footer(s: &State, l: &Look, k: &Kit) -> El {
    let line = row().w_full().px(22.0).h(52.0).items_center().gap(10.0);
    if !s.ready() {
        return line.key("looking").child(look::spinner(l, 14.0)).child(text("Looking for things you can clear").font(ui(13.0, 400.0)).color(l.fg3));
    }
    let junk = s.junk_found();
    let line = line.key("found").child(icon(&look::CHECK_CIRCLE, 15.0).color(l.good));
    if junk < 50_000_000 && s.measuring.is_none() {
        return line.child(text("Nothing obvious to clear").font(ui(13.0, 400.0)).color(l.fg2));
    }
    let lead = if s.measuring.is_some() { "Measuring junk…".to_string() } else { look::size(junk) };
    line.child(text(lead).font(num(13.0, 500.0)).color(l.fg))
        .child(text("of caches, temporary files and deleted files can be cleared").font(ui(13.0, 400.0)).color(l.fg2).ml(-5.0).truncate().min_w(0.0).shrink())
        .child(spacer())
        .child(Button::new("Review").variant(ButtonVariant::Neutral).on_press(|s: &mut State| s.go(Page::Cleaner)).build(k).key("review"))
}

fn scan_card(s: &State, l: &Look, k: &Kit, w: f32) -> El {
    let mut legend = col().gap(4.0).mx(-6.0);
    for r in 0..3 {
        let mut line = row().gap(14.0);
        for c in 0..3 {
            line = line.child(legend_cell(s, l, r * 3 + c));
        }
        legend = legend.child(line);
    }
    col().key("scan-card")
        .w_full()
        .fill(l.card)
        .border(1.0, l.edge)
        .radius(14.0)
        .child(col().w_full().padding(22.0).gap(18.0).child(header(s, l)).child(bar(s, l, w)).child(legend))
        .child(el().w_full().h(1.0).fill(l.rule))
        .child(footer(s, l, k))
}

fn largest(s: &State, l: &Look, k: &Kit) -> El {
    let mut card = col().grow(1.0).basis_card(l).gap(4.0).child(text("Largest files").font(ui(13.0, 500.0)).color(l.fg2).pb(6.0));
    let Some(tree) = &s.tree else {
        return card.child(text("Shows up when the scan is done").font(ui(13.0, 400.0)).color(l.fg3));
    };
    for f in tree.top.iter().filter(|f| f.category as usize != TRASH).take(4) {
        card = card.child(
            row()
                .w_full()
                .items_center()
                .gap(10.0)
                .py(5.0)
                .child(icon(&CATEGORIES[f.category as usize].icon, 14.0).color(color_of(f.category as usize)))
                .child(text(f.name.to_string()).font(ui(13.0, 400.0)).color(l.fg).truncate().grow(1.0).min_w(0.0))
                .child(text(look::size(f.bytes)).font(num(13.0, 400.0)).color(l.fg2)),
        );
    }
    card.child(spacer()).child(row().pt(10.0).child(Button::new("See all").variant(ButtonVariant::Neutral).on_press(|s: &mut State| {
        s.filter = None;
        s.go(Page::Files);
    }).build(k).key("see-all")))
}

fn junk_card(s: &State, l: &Look, k: &Kit) -> El {
    let found = s.junk_found();
    let mut card = col().grow(1.0).basis_card(l).gap(6.0).child(text("Junk you can clean").font(ui(13.0, 500.0)).color(l.fg2));
    if s.measuring.is_some() {
        card = card.child(row().gap(10.0).items_center().py(8.0).child(look::spinner(l, 16.0)).child(text("Measuring caches and temporary files").font(ui(13.0, 400.0)).color(l.fg3)));
    } else {
        card = card.child(text(look::size(found)).font(num(30.0, 600.0).tracking(-0.5)).color(l.fg));
    }
    let mut groups = col().gap(3.0).pt(4.0);
    let mut sorted: Vec<_> = s.junk.iter().filter(|g| g.bytes() > 0).collect();
    sorted.sort_by_key(|g| std::cmp::Reverse(g.bytes()));
    for g in sorted.into_iter().take(3) {
        groups = groups.child(
            row()
                .w_full()
                .child(text(g.name).font(ui(13.0, 400.0)).color(l.fg2).grow(1.0))
                .child(text(look::size(g.bytes())).font(num(13.0, 400.0)).color(l.fg3)),
        );
    }
    card.child(groups)
        .child(spacer())
        .child(row().pt(10.0).child(Button::new("Open Cleaner").variant(ButtonVariant::Contrast).on_press(|s: &mut State| s.go(Page::Cleaner)).build(k).key("open-cleaner")))
}

trait CardExt {
    fn basis_card(self, l: &Look) -> El;
}

impl CardExt for El {
    fn basis_card(self, l: &Look) -> El {
        self.min_w(0.0).padding(20.0).fill(l.card).border(1.0, l.edge).radius(14.0)
    }
}

pub fn page(s: &State, l: &Look, k: &Kit, cx: &mut Cx<State>) -> El {
    let w = cx.layout_size("bar").map_or(600.0, |size| size[0]);
    col().w_full()
        .gap(16.0)
        .child(scan_card(s, l, k, w))
        .child(row().w_full().gap(16.0).items_stretch().child(junk_card(s, l, k)).child(largest(s, l, k)))
}
