//! The app's state and everything that changes it: scanning, selection, and the jobs that
//! change the disk.

use crate::junk::{self, Group, Mode};
use crate::ops::{self, Outcome, Rescan, Work};
use crate::scan::{self, APPS, N, OUTSIDE, SYSTEM, TRASH, Tree};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Overview,
    Cleaner,
    Files,
    Explore,
}

/// Where a listed item lives in the tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Where {
    Node(u32),
    File { dir: u32, name: Box<str> },
}

#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub files: u64,
    pub category: usize,
    pub modified: u64,
    pub at: Where,
    /// Why it can't be removed from here.
    pub lock: Option<&'static str>,
    /// A scanned location: its path, shown under its name.
    pub detail: Option<String>,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        matches!(self.at, Where::Node(_))
    }
}

#[derive(Clone)]
pub struct Pick {
    pub name: String,
    pub bytes: u64,
    pub category: usize,
    pub at: Where,
}

pub enum Scan {
    NotStarted,
    Running(Arc<scan::Progress>),
    /// The demo's scripted scan, holding the tree it reveals at the end.
    Demo(Box<Option<Tree>>),
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Clean,
    Recycle,
    Delete,
}

pub struct Plan {
    pub kind: Kind,
    pub items: Vec<(PathBuf, Pick)>,
    /// (group, target) for a clean.
    pub cleans: Vec<(usize, usize)>,
    pub bytes: u64,
    /// Some items hold app data or settings.
    pub app_data: bool,
}

pub struct Report {
    pub title: String,
    pub lines: Vec<String>,
}

pub enum Job {
    Idle,
    Confirm(Plan),
    Running(Arc<ops::Op>, Plan),
    Report(Report),
}

pub const ROWS: usize = 12;
pub const MAX_COLS: usize = 400;

pub struct State {
    pub demo: bool,
    pub page: Page,
    pub started: bool,
    pub began: f64,
    pub scan: Scan,
    pub tree: Option<Tree>,
    pub capacity: f64,
    pub used: f64,
    /// Bytes per category, and what the bar shows as it eases toward them.
    pub target: [f64; N],
    pub shown: [f64; N],
    pub growing: [f32; N],
    pub dim: [f32; N],
    pub path: String,
    pub files: u64,
    pub done_at: f64,
    pub took: f64,
    pub hover: Option<usize>,
    pub junk: Vec<Group>,
    pub measuring: Option<Arc<junk::Measure>>,
    /// The folder Explore shows; 0 lists the scanned locations.
    pub at: u32,
    pub listing: Vec<Entry>,
    /// Items the listing left out.
    pub more: usize,
    pub filter: Option<usize>,
    pub picks: BTreeMap<PathBuf, Pick>,
    pub job: Job,
    /// When the running job started, on the app clock.
    pub job_began: f64,
    pub clock: f64,
    pub frame: u32,
    pub dots: Vec<f32>,
    pub dots_to: Vec<f32>,
    pub owed: f32,
    pub rng: u64,
}

/// Lit dots settle toward the bottom of the bar.
pub fn lit_chance(row: usize) -> f32 {
    0.04 + 0.8 * (row as f32 / (ROWS - 1) as f32).powf(2.4)
}

pub fn init() -> State {
    let mut s = State {
        demo: false,
        page: Page::Overview,
        started: false,
        began: 0.0,
        scan: Scan::NotStarted,
        tree: None,
        capacity: 0.0,
        used: 0.0,
        target: [0.0; N],
        shown: [0.0; N],
        growing: [0.0; N],
        dim: [1.0; N],
        path: String::new(),
        files: 0,
        done_at: 0.0,
        took: 0.0,
        hover: None,
        junk: Vec::new(),
        measuring: None,
        at: 0,
        listing: Vec::new(),
        more: 0,
        filter: None,
        picks: BTreeMap::new(),
        job: Job::Idle,
        job_began: 0.0,
        clock: 0.0,
        frame: 0,
        dots: vec![0.0; MAX_COLS * ROWS],
        dots_to: vec![0.0; MAX_COLS * ROWS],
        owed: 0.0,
        rng: 0x9e37_79b9_7f4a_7c15,
    };
    for i in 0..s.dots.len() {
        let lit = if s.random() < lit_chance(i % ROWS) { 1.0 } else { 0.0 };
        s.dots[i] = lit;
        s.dots_to[i] = lit;
    }
    s
}

const KEEP: &[&str] = &[
    "desktop", "documents", "downloads", "pictures", "music", "videos", "movies", "appdata", "library", "onedrive", "public",
    "applications", ".config", ".local", ".cache", ".ssh",
];

/// Why `at` can't be removed from here, if it can't.
pub fn lock(tree: &Tree, at: &Where) -> Option<&'static str> {
    let dir = match at {
        Where::Node(i) => *i,
        Where::File { dir, .. } => *dir,
    };
    let node = tree.node(dir);
    if node.category as usize == APPS {
        return Some(if cfg!(windows) { "Uninstall apps from Settings > Apps" } else { "Uninstall apps the way you installed them" });
    }
    if node.category as usize == TRASH {
        return Some("In the bin: empty it from Cleaner");
    }
    if node.depth == OUTSIDE {
        return Some("Outside your user folder");
    }
    if let Where::Node(i) = at {
        if node.parent == 0 {
            return Some("Your user folder itself stays");
        }
        if node.depth == 1 && KEEP.contains(&tree.node(*i).name.to_lowercase().as_str()) {
            return Some("This folder stays; open it to remove what's inside");
        }
        // AppData\Local, AppData\Roaming, Library\Application Support and the like: every
        // app keeps its data in them.
        let parent = tree.node(node.parent);
        if node.depth == 2 && matches!(parent.name.to_lowercase().as_str(), "appdata" | "library" | ".local" | ".config") {
            return Some("Apps keep their data here; open it to remove what's inside");
        }
    }
    None
}

impl State {
    pub fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn ready(&self) -> bool {
        matches!(self.scan, Scan::Ready)
    }

    pub fn found(&self, i: usize) -> bool {
        self.target[i] > 0.0 || self.shown[i] > 0.0
    }

    pub fn total(&self) -> f64 {
        self.shown.iter().sum()
    }

    pub fn busy(&self) -> bool {
        matches!(self.job, Job::Running(..))
    }

    // ---- scanning ----

    pub fn start(&mut self, clock: f64) {
        self.started = true;
        self.began = clock;
        if self.demo {
            crate::demo::load(self);
            return;
        }
        let (capacity, used) = scan::disk();
        (self.capacity, self.used) = (capacity as f64, used as f64);
        self.scan = Scan::Running(scan::start());
        self.junk = junk::groups();
        self.measure_junk();
    }

    pub fn measure_junk(&mut self) {
        if self.demo {
            return;
        }
        let specs = self.junk.iter().flat_map(|g| g.targets.iter().map(|t| (t.paths.clone(), t.mode, t.skip.clone()))).collect();
        self.measuring = Some(junk::measure_all(specs));
    }

    pub fn rescan(&mut self) {
        if self.busy() {
            return;
        }
        if let Scan::Running(p) = &self.scan {
            p.cancel.store(true, Relaxed);
        }
        self.scan = Scan::NotStarted;
        self.started = false;
        self.tree = None;
        self.target = [0.0; N];
        self.hover = None;
        self.at = 0;
        self.listing.clear();
        self.picks.clear();
        self.job = Job::Idle;
    }

    /// The tree arrived: totals, the listing and the bar follow it.
    pub fn take_tree(&mut self, tree: Tree) {
        self.tree = Some(tree);
        self.scan = Scan::Ready;
        self.refresh();
        self.list();
    }

    /// Category totals from the tree, with whatever the volume holds that the walk couldn't
    /// see (the OS, other users, unreadable folders) counted as system data.
    pub fn refresh(&mut self) {
        let Some(tree) = &self.tree else { return };
        let by = tree.by_category();
        let seen = tree.total() as f64;
        for (t, b) in self.target.iter_mut().zip(by) {
            *t = b as f64;
        }
        self.target[SYSTEM] += (self.used - seen).max(0.0);
    }

    // ---- navigation ----

    pub fn go(&mut self, page: Page) {
        if self.page != page {
            self.picks.clear();
        }
        self.page = page;
        if page == Page::Explore {
            self.list();
        }
    }

    pub fn open(&mut self, node: u32) {
        self.at = node;
        self.picks.clear();
        self.list();
    }

    pub fn show_category(&mut self, category: usize) {
        self.filter = Some(category);
        self.go(Page::Files);
    }

    /// What Explore shows for the current folder: its subfolders from the tree, and its files
    /// read fresh.
    pub fn list(&mut self) {
        self.listing.clear();
        self.more = 0;
        let Some(tree) = &self.tree else { return };
        if self.at != 0 && tree.node(self.at).removed {
            self.at = 0;
        }
        let at = self.at;
        let mut out = Vec::new();
        for &c in &tree.node(at).children {
            let node = tree.node(c);
            let where_ = Where::Node(c);
            out.push(Entry {
                name: if at == 0 { location_name(tree, c) } else { node.name.to_string() },
                path: tree.path(c),
                bytes: node.total,
                files: node.files,
                category: node.category as usize,
                modified: 0,
                lock: lock(tree, &where_),
                at: where_,
                detail: (at == 0).then(|| node.name.to_string()),
            });
        }
        if at != 0 {
            let category = tree.node(at).category as usize;
            let mut add = |name: &str, bytes: u64, modified: u64, path: PathBuf| {
                let where_ = Where::File { dir: at, name: name.into() };
                out.push(Entry {
                    name: name.to_string(),
                    path,
                    bytes,
                    files: 0,
                    category: scan::file_category(name, category),
                    modified,
                    lock: lock(tree, &where_),
                    at: where_,
                    detail: None,
                });
            };
            if self.demo {
                for f in tree.top.iter().filter(|f| f.dir == at) {
                    add(&f.name, f.bytes, f.modified, tree.path(at).join(&*f.name));
                }
            } else if let Ok(entries) = std::fs::read_dir(tree.path(at)) {
                for entry in entries.flatten().take(20_000) {
                    let Ok(meta) = entry.metadata() else { continue };
                    if meta.is_file() {
                        add(&entry.file_name().to_string_lossy(), scan::on_disk(&meta), scan::modified(&meta), entry.path());
                    }
                }
            }
        }
        out.sort_by_key(|e| std::cmp::Reverse(e.bytes));
        self.more = out.len().saturating_sub(300);
        out.truncate(300);
        self.listing = out;
    }

    // ---- selection ----

    pub fn toggle(&mut self, entry: &Entry) {
        if entry.lock.is_some() {
            return;
        }
        if self.picks.remove(&entry.path).is_none() {
            self.picks.insert(entry.path.clone(), Pick { name: entry.name.clone(), bytes: entry.bytes, category: entry.category, at: entry.at.clone() });
        }
    }

    pub fn toggle_all(&mut self, entries: &[Entry]) {
        let open: Vec<&Entry> = entries.iter().filter(|e| e.lock.is_none()).collect();
        if !open.is_empty() && open.iter().all(|e| self.picks.contains_key(&e.path)) {
            self.picks.clear();
        } else {
            for e in open {
                self.picks.entry(e.path.clone()).or_insert_with(|| Pick { name: e.name.clone(), bytes: e.bytes, category: e.category, at: e.at.clone() });
            }
        }
    }

    pub fn picked_bytes(&self) -> u64 {
        self.picks.values().map(|p| p.bytes).sum()
    }

    pub fn junk_chosen(&self) -> u64 {
        self.junk.iter().map(Group::chosen).sum()
    }

    pub fn junk_found(&self) -> u64 {
        self.junk.iter().map(Group::bytes).sum()
    }

    pub fn toggle_group(&mut self, g: usize) {
        let group = &mut self.junk[g];
        let all = group.targets.iter().filter(|t| !t.denied).all(|t| t.on);
        for t in &mut group.targets {
            t.on = !all && !t.denied;
        }
    }

    // ---- jobs ----

    pub fn ask(&mut self, kind: Kind) {
        if self.busy() || self.picks.is_empty() {
            return;
        }
        let items: Vec<(PathBuf, Pick)> = self.picks.iter().map(|(p, k)| (p.clone(), k.clone())).collect();
        let bytes = items.iter().map(|(_, k)| k.bytes).sum();
        let app_data = items.iter().any(|(_, k)| k.category == SYSTEM);
        self.job = Job::Confirm(Plan { kind, items, cleans: Vec::new(), bytes, app_data });
    }

    pub fn ask_clean(&mut self) {
        if self.busy() || self.measuring.is_some() {
            return;
        }
        let mut cleans = Vec::new();
        for (gi, g) in self.junk.iter().enumerate() {
            for (ti, t) in g.targets.iter().enumerate() {
                if t.on && !t.denied && t.bytes > 0 {
                    cleans.push((gi, ti));
                }
            }
        }
        if cleans.is_empty() {
            return;
        }
        let bytes = cleans.iter().map(|&(g, t)| self.junk[g].targets[t].bytes).sum();
        self.job = Job::Confirm(Plan { kind: Kind::Clean, items: Vec::new(), cleans, bytes, app_data: false });
    }

    pub fn cancel(&mut self) {
        if !self.busy() {
            self.job = Job::Idle;
        }
    }

    fn rescan_of(&self, path: &std::path::Path) -> Option<Rescan> {
        let tree = self.tree.as_ref()?;
        let node = tree.find(path)?;
        let n = tree.node(node);
        Some(Rescan { node, path: path.to_path_buf(), category: n.category as usize, depth: n.depth })
    }

    pub fn confirm(&mut self) {
        let Job::Confirm(plan) = std::mem::replace(&mut self.job, Job::Idle) else { return };
        let mut after = Vec::new();
        let work = match plan.kind {
            Kind::Clean => {
                let mut jobs = Vec::new();
                for &(g, t) in &plan.cleans {
                    let target = &self.junk[g].targets[t];
                    for p in &target.paths {
                        after.extend(self.rescan_of(p));
                    }
                    jobs.push(ops::CleanJob { name: target.name.clone(), paths: target.paths.clone(), mode: target.mode, skip: target.skip.clone(), bytes: target.bytes });
                }
                Work::Clean(jobs)
            }
            Kind::Recycle => {
                for bin in junk::bin_dirs() {
                    after.extend(self.rescan_of(&bin));
                }
                Work::Recycle(plan.items.iter().map(|(p, _)| p.clone()).collect())
            }
            Kind::Delete => {
                // A folder that was only partly deleted is walked again.
                for (p, pick) in &plan.items {
                    if let Where::Node(_) = pick.at {
                        after.extend(self.rescan_of(p));
                    }
                }
                Work::Delete(plan.items.iter().map(|(p, _)| p.clone()).collect())
            }
        };
        let op = ops::run(work, self.demo, after);
        self.job_began = self.clock;
        self.job = Job::Running(op, plan);
    }

    /// A job ended: fold what it did into the tree and say how it went.
    pub fn finish(&mut self, op: &ops::Op, plan: Plan) {
        let outcomes = op.outcomes.lock().unwrap().clone();
        let mut lines = Vec::new();
        let title;
        let mut demo_freed = 0u64;
        match plan.kind {
            Kind::Clean => {
                let left = op.left.lock().unwrap().clone();
                for (&(g, t), left) in plan.cleans.iter().zip(left) {
                    let target = &mut self.junk[g].targets[t];
                    if self.demo
                        && let Some(tree) = &mut self.tree {
                            for p in &target.paths {
                                if let Some(node) = tree.find(p) {
                                    tree.empty_node(node);
                                }
                            }
                        }
                    target.bytes = left.bytes;
                    target.files = left.files;
                }
                let freed = op.freed.load(Relaxed);
                let skipped = op.skipped.load(Relaxed);
                demo_freed = freed;
                title = format!("Freed {}", crate::look::size(freed));
                if !self.demo {
                    lines.push(format!("Removed {}.", crate::look::plural(op.removed.load(Relaxed), "file", "files")));
                }
                if skipped > 0 {
                    lines.push(format!(
                        "{} in use, so they were kept. Close the apps using them and clean again.",
                        crate::look::plural(skipped, "file was", "files were")
                    ));
                }
            }
            Kind::Recycle | Kind::Delete => {
                let mut done = 0u64;
                let mut bytes = 0u64;
                let mut failed = Vec::new();
                for ((_, pick), outcome) in plan.items.iter().zip(&outcomes) {
                    match outcome {
                        Outcome::Done => {
                            done += 1;
                            bytes += pick.bytes;
                            self.picks.retain(|_, p| p.at != pick.at);
                            if let Some(tree) = &mut self.tree {
                                match &pick.at {
                                    Where::Node(i) => tree.remove_node(*i),
                                    Where::File { dir, name } => tree.remove_file(*dir, name, pick.bytes, pick.category),
                                }
                            }
                        }
                        Outcome::Failed(e) => failed.push(format!("{}: {e}", pick.name)),
                    }
                }
                let things = crate::look::plural(done, "item", "items");
                if plan.kind == Kind::Recycle {
                    title = format!("Moved {things} to the {}", ops::BIN);
                    if done > 0 {
                        lines.push(format!("{} stays on disk until you empty the {}. You can restore anything from there.", crate::look::size(bytes), ops::BIN));
                        if self.demo {
                            self.demo_to_bin(bytes);
                        }
                    }
                } else {
                    title = format!("Deleted {things}");
                    if done > 0 {
                        lines.push(format!("Freed {}.", crate::look::size(bytes)));
                    }
                    demo_freed = bytes;
                }
                if !failed.is_empty() {
                    lines.push(format!("{} couldn't be removed:", crate::look::plural(failed.len() as u64, "item", "items")));
                    lines.extend(failed.into_iter().take(4));
                }
            }
        }
        if let Some(tree) = &mut self.tree {
            for (node, sub) in op.rescans.lock().unwrap().drain(..) {
                tree.graft(node, sub);
            }
            tree.finish();
        }
        if self.demo {
            self.used = (self.used - demo_freed as f64).max(0.0);
        } else {
            let (capacity, used) = scan::disk();
            (self.capacity, self.used) = (capacity as f64, used as f64);
            if plan.kind != Kind::Clean {
                self.measure_junk();
            }
        }
        if self.demo {
            lines.push("This is the demo, so nothing on your disk changed.".into());
        }
        self.refresh();
        self.list();
        self.job = Job::Report(Report { title, lines });
    }

    /// The demo's Recycle Bin grows by what was moved there.
    fn demo_to_bin(&mut self, bytes: u64) {
        for g in &mut self.junk {
            for t in &mut g.targets {
                if t.mode == Mode::Bin {
                    t.bytes += bytes;
                }
            }
        }
        if let Some(tree) = &mut self.tree
            && let Some(&bin) = tree.node(0).children.iter().find(|&&c| tree.node(c).category as usize == TRASH) {
                let mut sums = [0; N];
                sums[TRASH] = tree.node(bin).own + bytes;
                let files = tree.node(bin).own_files + 1;
                tree.set_own(bin, sums, files);
            }
    }
}

/// A scanned location's name in Explore.
pub fn location_name(tree: &Tree, node: u32) -> String {
    let n = tree.node(node);
    if n.depth == 0 {
        return "Your files".into();
    }
    if n.category as usize == TRASH {
        return ops::BIN.into();
    }
    // Split on both separators, so the demo's Windows paths read right everywhere.
    n.name.rsplit(['/', '\\']).find(|p| !p.is_empty()).unwrap_or(&n.name).to_string()
}
