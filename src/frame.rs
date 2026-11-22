//! The window: sidebar, page header, the selection bar and the dialogs over it all.

use crate::app::{Job, Kind, Page, State};
use crate::look::{self, Look, num, ui};
use crate::ops;
use crate::{overview, pages};
use rune::{Color, Cursor, Cx, El, Icon, ScrollSpec, col, el, icon, row, spacer, text};
use rune_kit::{Button, ButtonVariant, Dialog, Kit, Progress};

const SIDEBAR: f32 = 224.0;

fn nav(s: &State, l: &Look, page: Page, glyph: &'static Icon, label: &str, detail: Option<String>) -> El {
    let on = s.page == page;
    let mut r = row()
        .key(format!("nav-{label}").to_lowercase().replace(' ', "-"))
        .role("tab")
        .label(label)
        .checked(on)
        .focusable(true)
        .w_full()
        .h(34.0)
        .items_center()
        .gap(10.0)
        .px(10.0)
        .radius(8.0)
        .fill(if on { l.selected } else { Color::TRANSPARENT })
        .hover(l.hover)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut State| s.go(page))
        .child(icon(glyph, 15.0).color(if on { l.fg } else { l.fg2 }))
        .child(text(label.to_string()).font(ui(13.0, if on { 500.0 } else { 400.0 })).color(if on { l.fg } else { l.fg2 }).grow(1.0));
    if let Some(d) = detail {
        r = r.child(text(d).font(num(12.0, 400.0)).color(l.fg3));
    }
    r
}

fn sidebar(s: &State, l: &Look) -> El {
    let junk = if s.measuring.is_some() { None } else { Some(s.junk_found()).filter(|&b| b > 0).map(look::size) };
    let used = s.used;
    let frac = if s.capacity > 0.0 { (used / s.capacity) as f32 } else { 0.0 };
    let disk = col()
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(10.0)
        .fill(Color::WHITE.alpha(0.03))
        .border(1.0, l.rule)
        .child(row().gap(8.0).items_center().child(icon(&look::DISK, 14.0).color(l.fg2)).child(text(if cfg!(windows) { "Local Disk" } else { "This disk" }).font(ui(12.5, 500.0)).color(l.fg)))
        .child(el().w_full().h(5.0).radius(2.5).fill(Color::WHITE.alpha(0.08)).child(el().w_frac(frac.clamp(0.0, 1.0)).h(5.0).radius(2.5).fill(l.fg2)))
        .child(text(format!("{} free of {}", look::whole_gb((s.capacity - used).max(0.0)), look::whole_gb(s.capacity))).font(num(12.0, 400.0)).color(l.fg3));
    col().key("sidebar")
        .w(SIDEBAR)
        .h_full()
        .fill(l.side)
        .px(12.0)
        .pt(18.0)
        .pb(14.0)
        .gap(2.0)
        .child(
            row()
                .gap(10.0)
                .items_center()
                .px(8.0)
                .pb(18.0)
                .child(look::logo(24.0))
                .child(text("cleanitup").font(ui(15.0, 600.0).tracking(-0.2)).color(l.fg)),
        )
        .child(nav(s, l, Page::Overview, &look::OVERVIEW, "Overview", None))
        .child(nav(s, l, Page::Cleaner, &look::CLEANER, "Cleaner", junk))
        .child(nav(s, l, Page::Files, &look::FILES, "Large files", None))
        .child(nav(s, l, Page::Explore, &look::EXPLORE, "Explore", None))
        .child(spacer())
        .child(disk)
}

fn heading(s: &State, l: &Look, k: &Kit) -> El {
    let (title, sub): (&str, El) = match s.page {
        Page::Overview => ("Overview", text("What fills your disk, by kind of file").font(ui(13.0, 400.0)).color(l.fg2)),
        Page::Cleaner => {
            let sub = if s.measuring.is_some() {
                "Measuring caches and temporary files…".to_string()
            } else {
                format!("{} found · {} selected", look::size(s.junk_found()), look::size(s.junk_chosen()))
            };
            ("Cleaner", text(sub).font(ui(13.0, 400.0)).color(l.fg2))
        }
        Page::Files => ("Large files", text("The biggest files in your user folder").font(ui(13.0, 400.0)).color(l.fg2)),
        Page::Explore => ("Explore", pages::crumbs(s, l)),
    };
    let mut head = row().w_full().items_end().gap(16.0).child(
        col().grow(1.0).min_w(0.0).gap(4.0).child(text(title).font(ui(22.0, 600.0).tracking(-0.3)).color(l.fg)).child(sub),
    );
    if s.page == Page::Cleaner {
        let chosen = s.junk_chosen();
        let ready = chosen > 0 && s.measuring.is_none() && !s.busy();
        let label = if chosen > 0 { format!("Clean {}", look::size(chosen)) } else { "Clean".into() };
        head = head.child(Button::new(label).variant(ButtonVariant::Contrast).disabled(!ready).on_press(State::ask_clean).build(k).key("clean"));
    }
    if s.page == Page::Files {
        head = head.child(pages::filters(s, l).max_w(460.0));
    }
    head
}

/// What can be done with the selection, along the bottom of Explore and Large files.
fn selection_bar(s: &State, l: &Look, k: &Kit) -> El {
    let n = s.picks.len() as u64;
    let mut bar = row()
        .key("selection-bar")
        .w_full()
        .h(60.0)
        .px(28.0)
        .items_center()
        .gap(10.0)
        .fill(l.side)
        .child(text(format!("{} selected", look::plural(n, "item", "items"))).font(ui(13.0, 500.0)).color(l.fg))
        .child(text(look::size(s.picked_bytes())).font(num(13.0, 400.0)).color(l.fg2))
        .child(spacer())
        .child(Button::new("Clear").variant(ButtonVariant::Ghost).on_press(|s: &mut State| s.picks.clear()).build(k).key("clear"));
    if n == 1 {
        let path = s.picks.keys().next().cloned().unwrap_or_default();
        bar = bar.child(Button::new(format!("Show in {}", ops::FILE_MANAGER)).variant(ButtonVariant::Neutral).on_press(move |_: &mut State| ops::reveal(&path)).build(k).key("reveal-one"));
    }
    bar.child(Button::new("Delete permanently…").variant(ButtonVariant::Danger).disabled(s.busy()).on_press(|s: &mut State| s.ask(Kind::Delete)).build(k).key("delete"))
        .child(Button::new(format!("Move to {}", ops::BIN)).variant(ButtonVariant::Contrast).disabled(s.busy()).on_press(|s: &mut State| s.ask(Kind::Recycle)).build(k).key("recycle"))
}

fn buttons(row_: El) -> El {
    row_.w_full().justify_end().gap(8.0).px(16.0).pb(16.0).pt(8.0)
}

fn dialogs(s: &State, l: &Look, k: &Kit, layer: &mut Vec<El>) {
    match &s.job {
        Job::Idle => {}
        Job::Confirm(plan) => {
            let n = plan.items.len() as u64;
            let (title, description, verb, variant) = match plan.kind {
                Kind::Clean => (
                    format!("Clean {}?", look::size(plan.bytes)),
                    "These files are deleted permanently. Files that apps have open are skipped.".to_string(),
                    "Clean".to_string(),
                    ButtonVariant::Contrast,
                ),
                Kind::Recycle => (
                    format!("Move {} to the {}?", look::plural(n, "item", "items"), ops::BIN),
                    format!("{} in all. You can restore them from the {} until you empty it.", look::size(plan.bytes), ops::BIN),
                    format!("Move to {}", ops::BIN),
                    ButtonVariant::Contrast,
                ),
                Kind::Delete => (
                    format!("Delete {} permanently?", look::plural(n, "item", "items")),
                    format!("{} in all. This skips the {} and can't be undone.", look::size(plan.bytes), ops::BIN),
                    "Delete permanently".to_string(),
                    ButtonVariant::Danger,
                ),
            };
            let names: Vec<(String, u64)> = match plan.kind {
                Kind::Clean => plan.cleans.iter().map(|&(g, t)| {
                    let t = &s.junk[g].targets[t];
                    (t.name.clone(), t.bytes)
                }).collect(),
                _ => plan.items.iter().map(|(_, p)| (p.name.clone(), p.bytes)).collect(),
            };
            let mut list = col().w_full().px(16.0).gap(2.0);
            for (name, bytes) in names.iter().take(6) {
                list = list.child(
                    row()
                        .w_full()
                        .gap(12.0)
                        .py(3.0)
                        .child(text(name.clone()).font(ui(13.0, 400.0)).color(l.fg).truncate().grow(1.0).min_w(0.0))
                        .child(text(look::size(*bytes)).font(num(13.0, 400.0)).color(l.fg2)),
                );
            }
            if names.len() > 6 {
                list = list.child(text(format!("and {} more", names.len() - 6)).font(ui(13.0, 400.0)).color(l.fg3).py(3.0));
            }
            let mut body = col().w_full().gap(10.0).child(list);
            if plan.app_data {
                body = body.child(
                    row()
                        .mx(16.0)
                        .gap(8.0)
                        .padding(10.0)
                        .radius(8.0)
                        .fill(l.warn.alpha(0.1))
                        .child(icon(&look::WARN, 14.0).color(l.warn))
                        .child(text("Some of these hold app data or settings. Apps that use them may lose what they saved.").font(ui(12.5, 400.0)).color(l.fg).wrap().grow(1.0).min_w(0.0)),
                );
            }
            let actions = buttons(row())
                .child(Button::new("Cancel").variant(ButtonVariant::Neutral).on_press(State::cancel).build(k).key("cancel"))
                .child(Button::new(verb).variant(variant).on_press(State::confirm).build(k).key("confirm"));
            Dialog::new("confirm", title).description(description).open(true).fit().width(460.0).on_close(State::cancel).child(body).child(actions).build(k, layer);
        }
        Job::Running(op, plan) => {
            let title = match plan.kind {
                Kind::Clean => "Cleaning…".to_string(),
                Kind::Recycle => format!("Moving to the {}…", ops::BIN),
                Kind::Delete => "Deleting…".to_string(),
            };
            let current = op.current.lock().unwrap().clone();
            // No close button: the work can't be stopped halfway.
            let panel = col()
                .key("running-panel")
                .role("dialog")
                .label(title.clone())
                .w_full()
                .max_w(420.0)
                .padding(18.0)
                .gap(12.0)
                .radius(10.0)
                .fill(k.c.layer_01)
                .border(1.0, l.edge)
                .shadow(16.0, 48.0, Color::BLACK.alpha(0.5))
                .modal(false, |_: &mut State| {})
                .child(row().gap(10.0).items_center().child(look::spinner(l, 14.0)).child(text(title).font(ui(14.0, 500.0)).color(l.fg)))
                .child(Progress::new(Some(op.fraction())).build(k).w_full())
                .child(text(current).font(ui(12.5, 400.0)).color(l.fg3).truncate_middle());
            layer.push(el().key("running-scrim").absolute(0.0, 0.0).w_full().h_full().fill(k.c.scrim));
            layer.push(el().key("running-layer").absolute(0.0, 0.0).w_full().h_full().padding(16.0).center().child(panel));
        }
        Job::Report(report) => {
            let mut body = col().w_full().px(16.0).gap(6.0);
            for line in &report.lines {
                body = body.child(text(line.clone()).font(ui(13.0, 400.0)).color(l.fg2).wrap());
            }
            let actions = buttons(row()).child(Button::new("Done").variant(ButtonVariant::Contrast).on_press(State::cancel).build(k).key("done"));
            Dialog::new("report", report.title.clone()).open(true).fit().width(440.0).on_close(State::cancel).child(body).child(actions).build(k, layer);
        }
    }
}

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let l = look::look();
    let k = look::kit(&l);
    let content = match s.page {
        Page::Overview => overview::page(s, &l, &k, cx),
        Page::Cleaner => pages::cleaner(s, &l),
        Page::Files => pages::files(s, &l),
        Page::Explore => pages::explore(s, &l),
    };
    let scroller = col()
        .key(format!("scroll-{:?}", s.page))
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(col().w_full().px(28.0).pt(8.0).pb(28.0).child(content));
    let mut main = col()
        .key("main")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .fill(l.bg)
        .child(col().w_full().px(28.0).pt(24.0).pb(16.0).child(heading(s, &l, &k)))
        .child(scroller);
    if !s.picks.is_empty() && matches!(s.page, Page::Files | Page::Explore) {
        main = main.child(el().w_full().h(1.0).fill(l.rule)).child(selection_bar(s, &l, &k));
    }
    let base = row().key("root").w_full().h_full().fill(l.bg).child(sidebar(s, &l)).child(el().w(1.0).h_full().fill(l.rule)).child(main);
    let mut layer = Vec::new();
    dialogs(s, &l, &k, &mut layer);
    rune_kit::root(base, layer)
}
