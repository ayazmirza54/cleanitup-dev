//! The Cleaner, Large files and Explore pages.

use crate::app::{Entry, State, Where, location_name, lock};
use crate::look::{self, CATEGORIES, Look, checkbox, color_of, icon_button, num, ui};
use crate::ops;
use crate::scan::{N, TRASH};
use rune::{Color, Cursor, El, col, el, icon, row, text};

// ---- Cleaner ----

pub fn cleaner(s: &State, l: &Look) -> El {
    let mut list = col().w_full().gap(10.0);
    for (gi, g) in s.junk.iter().enumerate() {
        let usable: Vec<_> = g.targets.iter().filter(|t| !t.denied).collect();
        let on = usable.iter().filter(|t| t.on).count();
        let all = on > 0 && on == usable.len();
        let mixed = on > 0 && !all;
        let size = if s.measuring.is_some() && g.targets.iter().all(|t| t.bytes == 0) { "Measuring…".to_string() } else { look::size(g.bytes()) };
        let chevron = if g.open { &look::CHEVRON_DOWN } else { &look::CHEVRON };
        let head = row()
            .key(format!("group-{gi}"))
            .w_full()
            .items_center()
            .gap(12.0)
            .px(14.0)
            .py(12.0)
            .radius(12.0)
            .cursor(Cursor::Pointer)
            .on_press(move |s: &mut State| s.junk[gi].open ^= true)
            .child(checkbox(all, mixed, g.name, l).key(format!("group-check-{gi}")).on_press(move |s: &mut State| s.toggle_group(gi)))
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .gap(3.0)
                    .child(text(g.name).font(ui(14.0, 500.0)).color(l.fg))
                    .child(text(g.detail).font(ui(12.5, 400.0)).color(l.fg3).wrap()),
            )
            .child(text(size).font(num(13.5, 500.0)).color(if g.chosen() > 0 { l.fg } else { l.fg3 }))
            .child(icon(chevron, 14.0).color(l.fg3));
        let mut card = col().w_full().fill(l.card).border(1.0, l.edge).radius(12.0).child(head);
        if g.open {
            let mut rows = col().w_full().pb(8.0);
            for (ti, t) in g.targets.iter().enumerate() {
                let mut line = row()
                    .key(format!("target-{gi}-{ti}"))
                    .w_full()
                    .items_center()
                    .gap(12.0)
                    .pl(50.0)
                    .pr(40.0)
                    .py(5.0);
                if t.denied {
                    line = line
                        .child(el().size(24.0, 24.0).center().child(icon(&look::LOCK, 13.0).color(l.fg3)))
                        .child(text(t.name.clone()).font(ui(13.0, 400.0)).color(l.fg3).grow(1.0).min_w(0.0))
                        .child(text("Needs administrator").font(ui(12.0, 400.0)).color(l.fg3));
                } else {
                    line = line
                        .child(checkbox(t.on, false, &t.name, l).on_press(move |s: &mut State| {
                            let t = &mut s.junk[gi].targets[ti];
                            t.on = !t.on;
                        }))
                        .child(text(t.name.clone()).font(ui(13.0, 400.0)).color(if t.on { l.fg } else { l.fg2 }).grow(1.0).min_w(0.0).truncate())
                        .child(text(look::size(t.bytes)).font(num(13.0, 400.0)).color(l.fg2));
                }
                rows = rows.child(line);
            }
            card = card.child(el().w_full().h(1.0).fill(l.rule).mb(6.0)).child(rows);
        }
        list = list.child(card);
    }
    if s.junk.is_empty() && s.measuring.is_none() {
        list = list.child(empty(l, "Nothing to clean", "No caches or temporary folders were found on this computer."));
    }
    list.child(
        row()
            .gap(8.0)
            .pt(4.0)
            .items_start()
            .child(icon(&look::WARN, 14.0).color(l.fg3).mt(1.0))
            .child(
                text("Cleaning deletes these files permanently. Apps make caches again as they need them, so some space comes back over time.")
                    .font(ui(12.5, 400.0))
                    .color(l.fg3)
                    .wrap()
                    .grow(1.0)
                    .min_w(0.0),
            ),
    )
}

// ---- shared rows ----

fn empty(l: &Look, title: &str, body: &str) -> El {
    col().w_full()
        .py(48.0)
        .gap(6.0)
        .items_center()
        .child(text(title.to_string()).font(ui(15.0, 500.0)).color(l.fg2))
        .child(text(body.to_string()).font(ui(13.0, 400.0)).color(l.fg3))
}

pub fn scanning(l: &Look) -> El {
    col().w_full()
        .py(64.0)
        .gap(12.0)
        .items_center()
        .child(look::spinner(l, 20.0))
        .child(text("Scanning your disk").font(ui(15.0, 500.0)).color(l.fg2))
        .child(text("This page fills in when the scan finishes.").font(ui(13.0, 400.0)).color(l.fg3))
}

/// One file or folder: select, open (folders), show in the file manager.
fn item_row(s: &State, l: &Look, e: &Entry, max: u64, key: String, detail: Option<String>, when: Option<String>) -> El {
    let picked = s.picks.contains_key(&e.path);
    let lead = match e.lock {
        Some(why) => el().size(24.0, 24.0).center().help(why).child(icon(&look::LOCK, 13.0).color(l.fg3)),
        None => {
            let entry = e.clone();
            checkbox(picked, false, &e.name, l).key("pick").on_press(move |s: &mut State| s.toggle(&entry))
        }
    };
    let glyph = if e.is_dir() { &look::FOLDER } else { &CATEGORIES[e.category].icon };
    let mut names = col().grow(1.0).min_w(0.0).gap(2.0).child(text(e.name.clone()).font(ui(13.0, 500.0)).color(l.fg).truncate());
    if let Some(detail) = detail {
        names = names.child(text(detail).font(ui(12.0, 400.0)).color(l.fg3).truncate_middle());
    }
    let frac = if max > 0 { (e.bytes as f64 / max as f64) as f32 } else { 0.0 };
    let meter = el().w(96.0).h(5.0).radius(2.5).fill(Color::WHITE.alpha(0.06)).child(el().w_frac(frac.clamp(0.0, 1.0)).h(5.0).radius(2.5).fill(color_of(e.category)));
    let path = e.path.clone();
    let reveal = icon_button(&look::REVEAL, &format!("Show in {}", ops::FILE_MANAGER), l).on_press(move |_: &mut State| ops::reveal(&path));
    let mut line = row()
        .key(key)
        .w_full()
        .h(48.0)
        .items_center()
        .gap(12.0)
        .px(10.0)
        .radius(9.0)
        .fill(if picked { l.selected } else { Color::TRANSPARENT })
        .hover(l.hover)
        .child(lead)
        .child(icon(glyph, 17.0).color(if e.is_dir() { color_of(e.category).mix(l.fg, 0.25) } else { color_of(e.category) }))
        .child(names);
    if let Some(when) = when {
        line = line.child(text(when).font(ui(12.5, 400.0)).color(l.fg3).w(104.0));
    }
    line = line
        .child(meter)
        .child(row().w(84.0).justify_end().child(text(look::size(e.bytes)).font(num(13.0, 500.0)).color(l.fg2)))
        .child(reveal);
    if let Where::Node(node) = e.at {
        line = line
            .cursor(Cursor::Pointer)
            .on_press(move |s: &mut State| s.open(node))
            .child(el().size(16.0, 16.0).center().child(icon(&look::CHEVRON, 13.0).color(l.fg3)));
    } else {
        line = line.child(el().size(16.0, 16.0));
        if e.lock.is_none() {
            let entry = e.clone();
            line = line.cursor(Cursor::Pointer).on_press(move |s: &mut State| s.toggle(&entry));
        }
    }
    line
}

fn select_all(s: &State, l: &Look, entries: &[Entry], label: &str) -> El {
    let open: Vec<&Entry> = entries.iter().filter(|e| e.lock.is_none()).collect();
    let on = open.iter().filter(|e| s.picks.contains_key(&e.path)).count();
    let list: Vec<Entry> = entries.to_vec();
    if open.is_empty() {
        return row().w_full().px(10.0).pb(4.0).h(28.0).items_center().child(text(label.to_string()).font(ui(12.5, 500.0)).color(l.fg3));
    }
    row()
        .w_full()
        .items_center()
        .gap(12.0)
        .px(10.0)
        .pb(4.0)
        .child(checkbox(on > 0 && on == open.len(), on > 0 && on < open.len(), "Select all", l).key("select-all").on_press(move |s: &mut State| s.toggle_all(&list)))
        .child(text(label.to_string()).font(ui(12.5, 500.0)).color(l.fg3))
}

// ---- Large files ----

pub fn files_entries(s: &State) -> Vec<Entry> {
    let Some(tree) = &s.tree else { return Vec::new() };
    tree.top
        .iter()
        // What's already in the bin is the Cleaner's to empty.
        .filter(|f| f.category as usize != TRASH && s.filter.is_none_or(|c| f.category as usize == c))
        .take(200)
        .map(|f| {
            let at = Where::File { dir: f.dir, name: f.name.clone() };
            Entry {
                name: f.name.to_string(),
                path: tree.path(f.dir).join(&*f.name),
                bytes: f.bytes,
                files: 0,
                category: f.category as usize,
                modified: f.modified,
                lock: lock(tree, &at),
                at,
                detail: Some(tree.display(f.dir)),
            }
        })
        .collect()
}

pub fn filters(s: &State, l: &Look) -> El {
    let Some(tree) = &s.tree else { return el() };
    let mut counts = [0usize; N];
    for f in tree.top.iter().filter(|f| f.category as usize != TRASH) {
        counts[f.category as usize] += 1;
    }
    let chip = |label: String, on: bool, key: String| {
        row()
            .key(key)
            .role("button")
            .label(label.clone())
            .focusable(true)
            .px(11.0)
            .py(5.0)
            .radius(99.0)
            .fill(if on { Color::WHITE.alpha(0.12) } else { Color::TRANSPARENT })
            .border(1.0, if on { Color::TRANSPARENT } else { l.edge })
            .hover(l.hover)
            .cursor(Cursor::Pointer)
            .child(text(label).font(ui(12.5, 500.0)).color(if on { l.fg } else { l.fg2 }))
    };
    let mut chips = row().gap(6.0).flex_wrap().child(chip("All".into(), s.filter.is_none(), "filter-all".into()).on_press(|s: &mut State| {
        s.filter = None;
        s.picks.clear();
    }));
    for (c, &n) in counts.iter().enumerate() {
        if n == 0 {
            continue;
        }
        chips = chips.child(chip(CATEGORIES[c].name.into(), s.filter == Some(c), format!("filter-{c}")).on_press(move |s: &mut State| {
            s.filter = Some(c);
            s.picks.clear();
        }));
    }
    chips
}

pub fn files(s: &State, l: &Look) -> El {
    if !s.ready() {
        return scanning(l);
    }
    let entries = files_entries(s);
    if entries.is_empty() {
        let what = s.filter.map_or("files", |c| CATEGORIES[c].name);
        return empty(l, &format!("No large {} in your user folder", what.to_lowercase()), "Try another category, or look around in Explore.");
    }
    let now = if s.demo { crate::demo::NOW } else { look::now() };
    let max = entries.first().map_or(0, |e| e.bytes);
    let mut list = col().w_full().gap(2.0).child(select_all(s, l, &entries, &format!("{} largest", entries.len())));
    for (i, e) in entries.iter().enumerate() {
        list = list.child(item_row(s, l, e, max, format!("file-{i}"), e.detail.clone(), Some(look::ago(now, e.modified))));
    }
    list
}

// ---- Explore ----

pub fn crumbs(s: &State, l: &Look) -> El {
    let Some(tree) = &s.tree else { return el() };
    let mut chain = Vec::new();
    let mut at = s.at;
    while at != 0 {
        chain.push(at);
        at = tree.node(at).parent;
    }
    chain.reverse();
    let crumb = |label: String, node: u32, last: bool| {
        let t = text(label).font(ui(13.0, if last { 500.0 } else { 400.0 })).color(if last { l.fg } else { l.fg2 }).truncate();
        let c = row().key(format!("crumb-{node}")).px(6.0).py(3.0).radius(6.0).min_w(0.0).child(t);
        if last { c } else { c.role("link").focusable(true).hover(l.hover).cursor(Cursor::Pointer).on_press(move |s: &mut State| s.open(node)) }
    };
    let mut out = row().items_center().gap(2.0).ml(-6.0).min_w(0.0).child(crumb("All locations".into(), 0, chain.is_empty()));
    for (k, &node) in chain.iter().enumerate() {
        let name = if tree.node(node).parent == 0 { location_name(tree, node) } else { tree.node(node).name.to_string() };
        out = out.child(icon(&look::CHEVRON, 11.0).color(l.fg3)).child(crumb(name, node, k + 1 == chain.len()));
    }
    out
}

pub fn explore(s: &State, l: &Look) -> El {
    if !s.ready() {
        return scanning(l);
    }
    let Some(tree) = &s.tree else { return scanning(l) };
    if s.listing.is_empty() {
        return empty(l, "This folder is empty", "Nothing in it takes up space.");
    }
    let max = s.listing.first().map_or(0, |e| e.bytes);
    let label = if s.at == 0 {
        "Scanned locations".to_string()
    } else {
        format!("{} · {}", look::plural(s.listing.len() as u64 + s.more as u64, "item", "items"), look::size(tree.node(s.at).total))
    };
    let mut list = col().w_full().gap(2.0).child(select_all(s, l, &s.listing, &label));
    for (i, e) in s.listing.iter().enumerate() {
        let detail = match (&e.detail, e.is_dir()) {
            (Some(d), _) => Some(d.clone()),
            (None, true) => Some(look::plural(e.files, "file", "files")),
            (None, false) => None,
        };
        list = list.child(item_row(s, l, e, max, format!("item-{i}"), detail, None));
    }
    if s.more > 0 {
        list = list.child(text(format!("{} smaller items not shown", look::thousands(s.more as u64))).font(ui(12.5, 400.0)).color(l.fg3).px(10.0).py(10.0));
    }
    list
}
