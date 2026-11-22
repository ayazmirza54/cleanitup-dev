//! A made-up Windows machine for `--scene demo`: a scripted scan, a folder tree, junk and large
//! files. Jobs in the demo run dry, so nothing on the real disk is touched.

use crate::app::{Scan, State};
use crate::junk::{Group, Mode, Target};
use crate::scan::{APPS, CACHES, DEV, DOCS, FileEntry, MOVIES, MUSIC, N, OUTSIDE, PHOTOS, SYSTEM, TRASH, Tree};
use std::path::PathBuf;

pub const GB: [f64; N] = [52.6, 36.3, 58.6, 44.1, 41.7, 12.1, 34.2, 46.8, 31.4];
pub const CAPACITY: f64 = 512e9;
pub const USED: f64 = 357.8e9;
/// "Now" for the demo's file dates: 12 Nov 2026.
pub const NOW: u64 = 1_794_960_000;

const PATHS: [&[&str]; N] = [
    &["C:\\Program Files", "C:\\Program Files\\Adobe", "C:\\Program Files (x86)\\Steam\\steamapps"],
    &["~\\source\\repos\\web-app\\node_modules", "~\\.cargo\\registry", "~\\AppData\\Local\\npm-cache"],
    &["~\\Pictures\\Camera Roll", "~\\Pictures\\Lightroom", "~\\Pictures\\Screenshots"],
    &["~\\Videos", "~\\Videos\\Captures", "~\\Videos\\Family"],
    &["~\\Documents", "~\\Desktop", "~\\Downloads", "~\\Documents\\Archive"],
    &["~\\Music", "~\\Music\\Podcast"],
    &["C:\\ProgramData", "~\\AppData\\Roaming", "~\\AppData\\Local\\Packages"],
    &["~\\AppData\\Local\\Temp", "~\\AppData\\Local\\Google\\Chrome\\User Data", "~\\AppData\\Local\\Adobe\\Media Cache Files"],
    &["C:\\$Recycle.Bin"],
];
const START: f64 = 0.45;
const STEP: f64 = 0.62;
const DUR: f64 = 1.35;

/// The scripted scan: categories fill one after another, then the tree arrives.
pub fn step(s: &mut State, t: f64) -> bool {
    let end = START + STEP * (N - 1) as f64 + DUR;
    let mut active = None;
    for i in 0..N {
        let x = ((t - START - i as f64 * STEP) / DUR).clamp(0.0, 1.0);
        s.target[i] = GB[i] * 1e9 * (1.0 - (1.0 - x).powi(3));
        if x > 0.0 && x < 1.0 {
            active = Some(i);
        }
    }
    let paths: &[&str] = match active {
        Some(i) => PATHS[i],
        None if t < START => &["C:\\"],
        None => &["~\\AppData\\Local\\Temp", "C:\\$Recycle.Bin", "~\\AppData\\Local\\CrashDumps"],
    };
    s.path = paths[(t / 0.3) as usize % paths.len()].to_string();
    t >= end + 1.6
}

pub fn load(s: &mut State) {
    s.capacity = CAPACITY;
    s.used = USED;
    let tree = tree();
    s.junk = junk(&tree);
    s.files = 412_806;
    s.scan = Scan::Demo(Box::new(Some(tree)));
}

struct Build {
    t: Tree,
    sums: Vec<[u64; N]>,
    counts: Vec<u32>,
}

fn gb(g: f64) -> u64 {
    (g * 1e9) as u64
}

impl Build {
    fn dir(&mut self, parent: u32, name: &str, category: usize, own: f64) -> u32 {
        let depth = if parent == 0 {
            if category == DOCS { 0 } else { OUTSIDE }
        } else {
            let d = self.t.node(parent).depth;
            if d == OUTSIDE { OUTSIDE } else { d + 1 }
        };
        let i = self.t.push_node(name.into(), parent, category, depth);
        self.sums.push([0; N]);
        self.counts.push(0);
        self.sums[i as usize][category] += gb(own);
        if own > 0.0 {
            self.counts[i as usize] += (own * 140.0) as u32 + 3;
        }
        i
    }

    fn file(&mut self, dir: u32, name: &str, size: f64, category: usize, days: u64) {
        self.sums[dir as usize][category] += gb(size);
        self.counts[dir as usize] += 1;
        self.t.top.push(FileEntry { dir, name: name.into(), bytes: gb(size), category: category as u8, modified: NOW - days * 86_400 });
    }
}

fn tree() -> Tree {
    let mut tree = Tree::default();
    tree.sep = '\\';
    let mut b = Build { t: tree, sums: vec![[0; N]], counts: vec![0] };
    let home = b.dir(0, "C:\\Users\\Sam", DOCS, 0.4);

    let documents = b.dir(home, "Documents", DOCS, 1.0);
    let vms = b.dir(documents, "Virtual Machines", DOCS, 0.0);
    b.file(vms, "Ubuntu 24.04.vdi", 12.4, DOCS, 47);
    let archive = b.dir(documents, "Archive", DOCS, 2.6);
    b.file(archive, "Thesis drafts.zip", 1.3, DOCS, 610);
    b.dir(documents, "Taxes", DOCS, 0.4);
    let desktop = b.dir(home, "Desktop", DOCS, 1.6);
    b.file(desktop, "Client handoff.zip", 2.2, DOCS, 9);
    let downloads = b.dir(home, "Downloads", DOCS, 2.0);
    b.file(downloads, "Win11_24H2_English_x64.iso", 5.8, DOCS, 220);
    b.file(downloads, "Elden Ring backup.zip", 4.4, DOCS, 130);
    b.file(downloads, "Blender-4.2-windows-x64.msi", 0.38, DOCS, 75);
    b.file(downloads, "conference-talk.mp4", 1.7, MOVIES, 33);

    let pictures = b.dir(home, "Pictures", PHOTOS, 0.6);
    let camera = b.dir(pictures, "Camera Roll", PHOTOS, 21.4);
    b.file(camera, "IMG_4471.MOV", 2.1, PHOTOS, 18);
    b.file(camera, "IMG_3980.MOV", 1.6, PHOTOS, 96);
    let lightroom = b.dir(pictures, "Lightroom", PHOTOS, 14.2);
    b.file(lightroom, "Lightroom Catalog Previews.lrdata.zip", 3.4, PHOTOS, 12);
    b.dir(pictures, "Screenshots", PHOTOS, 2.3);

    let videos = b.dir(home, "Videos", MOVIES, 0.2);
    let family = b.dir(videos, "Family", MOVIES, 9.8);
    b.file(family, "Family trip 2025.mp4", 6.2, MOVIES, 310);
    b.file(family, "Wedding highlights 4K.mov", 4.9, MOVIES, 540);
    let captures = b.dir(videos, "Captures", MOVIES, 5.6);
    b.file(captures, "Screen Recording 2026-03-02.mp4", 1.4, MOVIES, 255);

    let music = b.dir(home, "Music", MUSIC, 7.4);
    let podcast = b.dir(music, "Podcast", MUSIC, 0.9);
    b.file(podcast, "Episode 42 raw.wav", 1.1, MUSIC, 64);

    let source = b.dir(home, "source", DOCS, 0.2);
    let repos = b.dir(source, "repos", DOCS, 0.6);
    let web = b.dir(repos, "web-app", DOCS, 0.3);
    b.dir(web, "node_modules", DEV, 1.9);
    b.dir(web, ".git", DEV, 0.6);
    let ml = b.dir(repos, "ml-experiments", DOCS, 0.4);
    b.file(ml, "training-set.parquet", 3.3, DOCS, 21);
    b.dir(ml, ".venv", DEV, 2.8);
    let cargo = b.dir(home, ".cargo", DEV, 0.3);
    b.dir(cargo, "registry", DEV, 1.2);
    let rustup = b.dir(home, ".rustup", DEV, 4.1);
    let _ = rustup;

    let appdata = b.dir(home, "AppData", SYSTEM, 0.0);
    let local = b.dir(appdata, "Local", SYSTEM, 0.6);
    let roaming = b.dir(appdata, "Roaming", SYSTEM, 3.4);
    b.dir(roaming, "Code", SYSTEM, 1.2);
    b.dir(local, "Temp", CACHES, 3.2);
    let google = b.dir(local, "Google", SYSTEM, 0.0);
    let chrome = b.dir(google, "Chrome", SYSTEM, 0.0);
    let chrome_data = b.dir(chrome, "User Data", SYSTEM, 0.9);
    let chrome_default = b.dir(chrome_data, "Default", SYSTEM, 0.7);
    b.dir(chrome_default, "Cache", CACHES, 1.8);
    let microsoft = b.dir(local, "Microsoft", SYSTEM, 0.4);
    let edge = b.dir(microsoft, "Edge", SYSTEM, 0.0);
    let edge_data = b.dir(edge, "User Data", SYSTEM, 0.3);
    let edge_default = b.dir(edge_data, "Default", SYSTEM, 0.2);
    b.dir(edge_default, "Cache", CACHES, 0.64);
    let windows = b.dir(microsoft, "Windows", SYSTEM, 0.2);
    b.dir(windows, "INetCache", CACHES, 0.13);
    b.dir(windows, "Explorer", CACHES, 0.31);
    b.dir(windows, "WER", CACHES, 0.086);
    let mozilla = b.dir(local, "Mozilla", SYSTEM, 0.0);
    let firefox = b.dir(mozilla, "Firefox", SYSTEM, 0.0);
    let profiles = b.dir(firefox, "Profiles", SYSTEM, 0.1);
    let profile = b.dir(profiles, "k3x9v2.default-release", SYSTEM, 0.5);
    b.dir(profile, "cache2", CACHES, 0.41);
    b.dir(local, "D3DSCache", CACHES, 0.095);
    let dumps = b.dir(local, "CrashDumps", CACHES, 0.0);
    b.file(dumps, "Photoshop.exe.18220.dmp", 1.1, CACHES, 4);
    b.dir(local, "npm-cache", DEV, 2.4);
    let pip = b.dir(local, "pip", DEV, 0.0);
    b.dir(pip, "Cache", DEV, 0.64);
    b.dir(local, "Packages", SYSTEM, 5.1);
    let adobe_cache = b.dir(local, "Adobe", SYSTEM, 0.0);
    b.dir(adobe_cache, "Media Cache Files", CACHES, 22.4);
    let spotify = b.dir(local, "Spotify", SYSTEM, 0.0);
    b.dir(spotify, "Storage", CACHES, 8.1);
    b.dir(local, "Programs", APPS, 6.2);

    let program_files = b.dir(0, "C:\\Program Files", APPS, 0.4);
    b.dir(program_files, "Adobe", APPS, 14.8);
    b.dir(program_files, "Microsoft Office", APPS, 4.1);
    b.dir(program_files, "NVIDIA Corporation", APPS, 2.3);
    let x86 = b.dir(0, "C:\\Program Files (x86)", APPS, 0.2);
    let steam = b.dir(x86, "Steam", APPS, 0.6);
    b.dir(steam, "steamapps", APPS, 21.0);
    let program_data = b.dir(0, "C:\\ProgramData", SYSTEM, 4.2);
    b.dir(program_data, "Package Cache", SYSTEM, 3.1);
    let bin = b.dir(0, "C:\\$Recycle.Bin", TRASH, 0.0);
    b.dir(bin, "S-1-5-21-1004", TRASH, 31.4);

    // Top up each category to the demo's numbers, in a folder that holds it.
    let fillers = [program_files, source, camera, family, documents, music, roaming, local, bin];
    let filler_cat = [APPS, DEV, PHOTOS, MOVIES, DOCS, MUSIC, SYSTEM, CACHES, TRASH];
    let system_unseen = 12.2;
    let mut by = [0u64; N];
    for s in &b.sums {
        for (o, v) in by.iter_mut().zip(s) {
            *o += v;
        }
    }
    for k in 0..N {
        let want = gb(GB[k] - if k == SYSTEM { system_unseen } else { 0.0 });
        let have = by[k];
        b.sums[fillers[k] as usize][filler_cat[k]] += want.saturating_sub(have);
    }
    for i in 0..b.t.nodes.len() {
        let (sums, count) = (b.sums[i], b.counts[i]);
        b.t.set_own(i as u32, sums, count);
    }
    b.t.finish();
    b.t
}

fn path_of(tree: &Tree, parts: &[&str]) -> PathBuf {
    let mut p = PathBuf::from(parts[0]);
    for part in &parts[1..] {
        p.push(part);
    }
    debug_assert!(tree.find(&p).is_some(), "demo path {p:?}");
    p
}

fn junk(tree: &Tree) -> Vec<Group> {
    let local = ["C:\\Users\\Sam", "AppData", "Local"];
    let at = |rest: &[&str]| {
        let mut parts = local.to_vec();
        parts.extend_from_slice(rest);
        path_of(tree, &parts)
    };
    let t = |name: &str, paths: Vec<PathBuf>, mode: Mode, on: bool, size: f64| Target {
        name: name.into(),
        paths,
        mode,
        skip: Vec::new(),
        on,
        bytes: gb(size),
        files: (size * 900.0) as u64 + 1,
        measured: true,
        denied: false,
    };
    let mut windows_temp = t("Windows temp folder", vec![PathBuf::from("C:\\Windows\\Temp")], Mode::Settled, false, 0.0);
    windows_temp.denied = true;
    let g = |name, detail, targets| Group { name, detail, targets, open: false };
    let mut groups = vec![
        g("Temporary files", "Files apps left behind. Anything changed in the last day is kept.", vec![
            t("Your temp folder", vec![at(&["Temp"])], Mode::Settled, true, 3.2),
            windows_temp,
        ]),
        g("Browser caches", "Pages and images browsers keep to load sites faster. Close browsers first to clear more.", vec![
            t("Google Chrome", vec![at(&["Google", "Chrome", "User Data", "Default", "Cache"])], Mode::Contents, true, 1.8),
            t("Microsoft Edge", vec![at(&["Microsoft", "Edge", "User Data", "Default", "Cache"])], Mode::Contents, true, 0.64),
            t("Firefox", vec![at(&["Mozilla", "Firefox", "Profiles", "k3x9v2.default-release", "cache2"])], Mode::Contents, true, 0.41),
        ]),
        g("Windows caches", "Thumbnails, shaders and web content Windows rebuilds when it needs them.", vec![
            t("Thumbnail cache", vec![at(&["Microsoft", "Windows", "Explorer"])], Mode::Prefixed(&["thumbcache_"]), true, 0.31),
            t("Internet cache", vec![at(&["Microsoft", "Windows", "INetCache"])], Mode::Contents, true, 0.13),
            t("DirectX shader cache", vec![at(&["D3DSCache"])], Mode::Contents, true, 0.095),
        ]),
        g("Crash reports", "Memory dumps and error reports saved when apps crashed.", vec![
            t("Crash dumps", vec![at(&["CrashDumps"])], Mode::Contents, true, 1.1),
            t("Error reports", vec![at(&["Microsoft", "Windows", "WER"])], Mode::Contents, true, 0.086),
        ]),
        g("Recycle Bin", "Files you deleted. Emptying it can't be undone.", vec![
            t("Recycle Bin", vec![path_of(tree, &["C:\\$Recycle.Bin"])], Mode::Bin, true, 31.4),
        ]),
        g("Developer caches", "Packages build tools download again when a project needs them.", vec![
            t("npm", vec![at(&["npm-cache"])], Mode::Contents, false, 2.4),
            t("pip", vec![at(&["pip", "Cache"])], Mode::Contents, false, 0.64),
            t("Cargo", vec![path_of(tree, &["C:\\Users\\Sam", ".cargo", "registry"])], Mode::Contents, false, 1.2),
        ]),
    ];
    groups[1].open = true;
    groups
}
