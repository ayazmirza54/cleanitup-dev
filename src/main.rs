//! cleanitup: a disk cleaner. Scans the disk, shows what fills it, cleans junk and removes what
//! you pick.
//!
//! `cargo run --release` opens the window and scans for real.
//! `cargo run --release -- --snap shot.png --scene demo --at 12` renders the demo headless;
//! jobs in the demo touch nothing on disk.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod demo;
mod frame;
mod junk;
mod look;
mod ops;
mod overview;
mod pages;
mod scan;
#[cfg(test)]
mod tests;

use app::{Job, Page, Scan, State};
use rune::{AppSpec, Tx};
use scan::N;
use std::sync::atomic::Ordering::Relaxed;

fn poll(s: &mut State, clock: f64, _: &mut Tx) -> bool {
    let dt = (clock - s.clock).clamp(0.0, 0.05) as f32;
    s.clock = clock;
    s.frame = s.frame.wrapping_add(1);
    if !s.started {
        s.start(clock);
    }

    let mut changed = false;
    let before = s.target;
    let scanning = !s.ready();

    // The scan.
    let mut arrived = None;
    match &mut s.scan {
        Scan::Running(p) => {
            let p = p.clone();
            for i in 0..N {
                s.target[i] = p.bytes[i].load(Relaxed) as f64;
            }
            s.files = p.files.load(Relaxed);
            s.path.clone_from(&p.path.lock().unwrap());
            if p.done.load(Relaxed) {
                arrived = p.tree.lock().unwrap().take();
            }
        }
        Scan::Demo(_) => {
            if demo::step(s, clock - s.began)
                && let Scan::Demo(tree) = &mut s.scan
            {
                arrived = tree.take();
            }
        }
        Scan::NotStarted | Scan::Ready => {}
    }
    if let Some(tree) = arrived {
        if !s.demo {
            s.files = tree.nodes.iter().map(|n| u64::from(n.own_files)).sum();
        }
        s.take_tree(tree);
        s.done_at = clock;
        s.took = clock - s.began;
        changed = true;
    }

    // Junk sizes, as they're measured.
    if let Some(m) = s.measuring.clone() {
        let sizes = m.sizes.lock().unwrap().clone();
        let mut k = 0;
        for g in &mut s.junk {
            for t in &mut g.targets {
                if let Some(stats) = sizes.get(k).copied().flatten()
                    && !t.measured
                {
                    t.bytes = stats.bytes;
                    t.files = stats.files;
                    t.denied = stats.denied;
                    t.measured = true;
                    changed = true;
                }
                k += 1;
            }
        }
        if m.done.load(Relaxed) {
            s.measuring = None;
            for g in &mut s.junk {
                for t in &mut g.targets {
                    t.measured = false;
                }
            }
            changed = true;
        }
    }

    // A job that finished.
    if let Job::Running(op, _) = &s.job {
        if s.demo {
            // The demo's dry run plays a step every fifth of a second.
            let k = ((clock - s.job_began) / 0.2) as usize;
            op.step.store(k.min(op.steps), Relaxed);
            if let Some(name) = op.names.get(k.min(op.steps.saturating_sub(1))) {
                op.current.lock().unwrap().clone_from(name);
            }
            if k > op.steps {
                op.done.store(true, Relaxed);
            }
        }
        if op.done.load(Relaxed)
            && let Job::Running(op, plan) = std::mem::replace(&mut s.job, Job::Idle) {
                s.finish(&op, plan);
            }
        changed = true;
    }

    // Ease the bar toward what was found, and note which categories are still taking bytes.
    let ease = 1.0 - (-dt * 7.0).exp();
    let unit = s.capacity.max(1.0);
    let mut moving = false;
    for i in 0..N {
        let gap = s.target[i] - s.shown[i];
        s.shown[i] += gap * ease as f64;
        if gap.abs() < unit * 2e-5 {
            s.shown[i] = s.target[i];
        } else {
            moving = true;
        }
        let taking = scanning && (s.target[i] > before[i] || gap > unit * 2e-3);
        s.growing[i] += ((if taking { 1.0 } else { 0.0 }) - s.growing[i]) * (1.0 - (-dt * 5.0).exp());
        let to = if s.hover.is_none() || s.hover == Some(i) { 1.0 } else { 0.28 };
        s.dim[i] += (to - s.dim[i]) * (1.0 - (-dt * 12.0).exp());
        if (to - s.dim[i]).abs() < 0.004 {
            s.dim[i] = to;
        } else {
            moving = true;
        }
    }

    // The dots twinkle while there is something to watch, then rest: an idle window draws nothing.
    let awake = s.page == Page::Overview && (scanning || moving || clock - s.done_at < 2.5);
    if awake {
        s.owed += s.dots.len() as f32 * 1.1 * dt;
        while s.owed >= 1.0 {
            s.owed -= 1.0;
            let i = (s.random() * s.dots.len() as f32) as usize % s.dots.len();
            s.dots_to[i] = if s.random() < app::lit_chance(i % app::ROWS) { 1.0 } else { 0.0 };
        }
        let k = 1.0 - (-dt * 6.5).exp();
        for (dot, to) in s.dots.iter_mut().zip(&s.dots_to) {
            *dot += (to - *dot) * k;
        }
    }
    awake || moving || changed || scanning || s.measuring.is_some()
}

fn main() {
    // The icon in the title bar, taskbar and Alt+Tab (and the Dock on macOS).
    #[cfg(windows)]
    rune::set_app_icon_resource(1);
    #[cfg(not(windows))]
    let _ = rune::set_app_icon(include_bytes!("../assets/icon-512.png").to_vec());
    rune::run(AppSpec::<State> {
        min_size: Some((920.0, 620.0)),
        scenes: &[
            ("demo", |s| s.demo = true),
            ("demo-cleaner", |s| {
                s.demo = true;
                s.page = Page::Cleaner;
            }),
            ("demo-files", |s| {
                s.demo = true;
                s.page = Page::Files;
            }),
            ("demo-explore", |s| {
                s.demo = true;
                s.page = Page::Explore;
            }),
        ],
        poll: Some(poll),
        ..AppSpec::new("cleanitup", (1100.0, 720.0), app::init, frame::view)
    });
}
