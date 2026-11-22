//! What the Cleaner offers to remove: caches, temporary files, crash reports and the Recycle
//! Bin, per platform. Measuring and cleaning share one sweep, which never follows links and
//! never deletes a folder it was asked to clean, only what is inside it.

use crate::scan::on_disk;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// Everything inside.
    Contents,
    /// Everything inside not changed in the last day; apps may still be using newer files.
    Settled,
    /// Only files directly inside whose names start with one of these.
    Prefixed(&'static [&'static str]),
    /// The Recycle Bin or Trash, emptied the way the OS empties it.
    Bin,
}

#[derive(Clone, Debug)]
pub struct Target {
    pub name: String,
    pub paths: Vec<PathBuf>,
    pub mode: Mode,
    /// Folders inside `paths` that another target covers.
    pub skip: Vec<PathBuf>,
    pub on: bool,
    pub bytes: u64,
    pub files: u64,
    pub measured: bool,
    /// The folder can't be read without administrator rights.
    pub denied: bool,
}

pub struct Group {
    pub name: &'static str,
    pub detail: &'static str,
    pub targets: Vec<Target>,
    pub open: bool,
}

impl Group {
    pub fn bytes(&self) -> u64 {
        self.targets.iter().map(|t| t.bytes).sum()
    }
    pub fn chosen(&self) -> u64 {
        self.targets.iter().filter(|t| t.on && !t.denied).map(|t| t.bytes).sum()
    }
}

fn target(name: &str, paths: Vec<PathBuf>, mode: Mode, on: bool) -> Target {
    Target { name: name.into(), paths, mode, skip: Vec::new(), on, bytes: 0, files: 0, measured: false, denied: false }
}

fn group(name: &'static str, detail: &'static str, targets: Vec<Target>) -> Group {
    Group { name, detail, targets, open: false }
}

fn env(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).map(PathBuf::from)
}

#[cfg_attr(not(windows), allow(dead_code))]
/// Each Chromium profile's caches under a browser's user-data folder.
fn chromium_caches(user_data: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for shared in ["ShaderCache", "GrShaderCache", "GraphiteDawnCache"] {
        out.push(user_data.join(shared));
    }
    let Ok(entries) = std::fs::read_dir(user_data) else { return out };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "Default" || name.starts_with("Profile ") || name == "Guest Profile" {
            let profile = entry.path();
            for cache in ["Cache", "Code Cache", "GPUCache", "DawnGraphiteCache", "DawnWebGPUCache"] {
                out.push(profile.join(cache));
            }
            out.push(profile.join("Service Worker").join("CacheStorage"));
            out.push(profile.join("Service Worker").join("ScriptCache"));
        }
    }
    out
}

#[cfg_attr(not(windows), allow(dead_code))]
/// `cache2` in each Firefox profile under `profiles`.
fn firefox_caches(profiles: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(profiles) else { return Vec::new() };
    entries.flatten().map(|e| e.path().join("cache2")).collect()
}

/// Where deleted files wait: what a Recycle Bin move fills.
pub fn bin_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        env("SystemDrive").map(|d| PathBuf::from(format!("{}\\$Recycle.Bin", d.to_string_lossy()))).into_iter().collect()
    } else if cfg!(target_os = "macos") {
        vec![crate::scan::home().join(".Trash")]
    } else {
        let data = env("XDG_DATA_HOME").unwrap_or_else(|| crate::scan::home().join(".local").join("share"));
        vec![data.join("Trash")]
    }
}

#[cfg(windows)]
fn platform_groups() -> Vec<Group> {
    let home = crate::scan::home();
    let local = env("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData").join("Local"));
    let roaming = env("APPDATA").unwrap_or_else(|| home.join("AppData").join("Roaming"));
    let windows = env("SystemRoot").unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    let data = env("ProgramData").unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
    vec![
        group("Temporary files", "Files apps left behind. Anything changed in the last day is kept.", vec![
            target("Your temp folder", vec![std::env::temp_dir()], Mode::Settled, true),
            target("Windows temp folder", vec![windows.join("Temp")], Mode::Settled, true),
        ]),
        group("Browser caches", "Pages and images browsers keep to load sites faster. Close browsers first to clear more.", vec![
            target("Google Chrome", chromium_caches(&local.join("Google").join("Chrome").join("User Data")), Mode::Contents, true),
            target("Microsoft Edge", chromium_caches(&local.join("Microsoft").join("Edge").join("User Data")), Mode::Contents, true),
            target("Brave", chromium_caches(&local.join("BraveSoftware").join("Brave-Browser").join("User Data")), Mode::Contents, true),
            target("Vivaldi", chromium_caches(&local.join("Vivaldi").join("User Data")), Mode::Contents, true),
            target("Opera", vec![local.join("Opera Software").join("Opera Stable").join("Cache")], Mode::Contents, true),
            target("Firefox", firefox_caches(&local.join("Mozilla").join("Firefox").join("Profiles")), Mode::Contents, true),
        ]),
        group("Windows caches", "Thumbnails, shaders and web content Windows rebuilds when it needs them.", vec![
            target("Thumbnail cache", vec![local.join("Microsoft").join("Windows").join("Explorer")], Mode::Prefixed(&["thumbcache_", "iconcache_"]), true),
            target("Internet cache", vec![local.join("Microsoft").join("Windows").join("INetCache")], Mode::Contents, true),
            target("DirectX shader cache", vec![local.join("D3DSCache")], Mode::Contents, true),
            target("Graphics driver caches", vec![local.join("NVIDIA").join("DXCache"), local.join("NVIDIA").join("GLCache"), local.join("AMD").join("DxCache"), local.join("AMD").join("DxcCache")], Mode::Contents, true),
        ]),
        group("Crash reports", "Memory dumps and error reports saved when apps crashed.", vec![
            target("Crash dumps", vec![local.join("CrashDumps")], Mode::Contents, true),
            target("Error reports", vec![local.join("Microsoft").join("Windows").join("WER"), data.join("Microsoft").join("Windows").join("WER")], Mode::Contents, true),
        ]),
        group("Recycle Bin", "Files you deleted. Emptying it can't be undone.", vec![
            target("Recycle Bin", bin_dirs(), Mode::Bin, true),
        ]),
        group("Developer caches", "Packages build tools download again when a project needs them.", vec![
            target("npm", vec![local.join("npm-cache"), roaming.join("npm-cache")], Mode::Contents, false),
            target("Yarn", vec![local.join("Yarn").join("Cache")], Mode::Contents, false),
            target("pip", vec![local.join("pip").join("Cache")], Mode::Contents, false),
            target("Cargo", vec![home.join(".cargo").join("registry").join("cache")], Mode::Contents, false),
            target("Gradle", vec![home.join(".gradle").join("caches")], Mode::Contents, false),
            target("NuGet", vec![local.join("NuGet").join("v3-cache")], Mode::Contents, false),
            target("Go", vec![local.join("go-build")], Mode::Contents, false),
        ]),
    ]
}

#[cfg(target_os = "macos")]
fn platform_groups() -> Vec<Group> {
    let home = crate::scan::home();
    let lib = home.join("Library");
    let caches = lib.join("Caches");
    vec![
        group("Temporary files", "Files apps left behind. Anything changed in the last day is kept.", vec![
            target("Your temp folder", vec![std::env::temp_dir()], Mode::Settled, true),
        ]),
        group("Browser caches", "Pages and images browsers keep to load sites faster. Quit browsers first to clear more.", vec![
            target("Safari", vec![caches.join("com.apple.Safari")], Mode::Contents, true),
            target("Google Chrome", vec![caches.join("Google").join("Chrome")], Mode::Contents, true),
            target("Microsoft Edge", vec![caches.join("Microsoft Edge")], Mode::Contents, true),
            target("Brave", vec![caches.join("BraveSoftware")], Mode::Contents, true),
            target("Firefox", vec![caches.join("Firefox")], Mode::Contents, true),
        ]),
        group("App caches and logs", "Data apps rebuild when they need it, and their old logs.", vec![
            target("Other app caches", vec![caches.clone()], Mode::Contents, true),
            target("Logs", vec![lib.join("Logs")], Mode::Contents, true),
        ]),
        group("Trash", "Files you deleted. Emptying it can't be undone.", vec![
            target("Trash", bin_dirs(), Mode::Bin, true),
        ]),
        group("Developer caches", "Build products and packages tools make again when a project needs them.", vec![
            target("Xcode build data", vec![lib.join("Developer").join("Xcode").join("DerivedData")], Mode::Contents, false),
            target("Simulator caches", vec![lib.join("Developer").join("CoreSimulator").join("Caches")], Mode::Contents, false),
            target("npm", vec![home.join(".npm").join("_cacache")], Mode::Contents, false),
            target("Yarn", vec![caches.join("Yarn")], Mode::Contents, false),
            target("pip", vec![caches.join("pip")], Mode::Contents, false),
            target("Homebrew", vec![caches.join("Homebrew")], Mode::Contents, false),
            target("Cargo", vec![home.join(".cargo").join("registry").join("cache")], Mode::Contents, false),
            target("Gradle", vec![home.join(".gradle").join("caches")], Mode::Contents, false),
        ]),
    ]
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_groups() -> Vec<Group> {
    let home = crate::scan::home();
    let cache = env("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache"));
    vec![
        group("Browser caches", "Pages and images browsers keep to load sites faster. Close browsers first to clear more.", vec![
            target("Google Chrome", vec![cache.join("google-chrome")], Mode::Contents, true),
            target("Chromium", vec![cache.join("chromium")], Mode::Contents, true),
            target("Brave", vec![cache.join("BraveSoftware")], Mode::Contents, true),
            target("Firefox", vec![cache.join("mozilla")], Mode::Contents, true),
        ]),
        group("App caches", "Data apps rebuild when they need it.", vec![
            target("Thumbnails", vec![cache.join("thumbnails")], Mode::Contents, true),
            target("Other app caches", vec![cache.clone()], Mode::Contents, true),
        ]),
        group("Trash", "Files you deleted. Emptying it can't be undone.", vec![
            target("Trash", bin_dirs(), Mode::Bin, true),
        ]),
        group("Developer caches", "Packages build tools download again when a project needs them.", vec![
            target("npm", vec![home.join(".npm").join("_cacache")], Mode::Contents, false),
            target("Yarn", vec![cache.join("yarn")], Mode::Contents, false),
            target("pip", vec![cache.join("pip")], Mode::Contents, false),
            target("Cargo", vec![home.join(".cargo").join("registry").join("cache")], Mode::Contents, false),
            target("Gradle", vec![home.join(".gradle").join("caches")], Mode::Contents, false),
            target("Go", vec![cache.join("go-build")], Mode::Contents, false),
        ]),
    ]
}

/// This machine's groups: only folders that exist, and no folder counted twice.
pub fn groups() -> Vec<Group> {
    let mut groups = platform_groups();
    for g in &mut groups {
        for t in &mut g.targets {
            t.paths.retain(|p| p.is_dir());
            t.paths.dedup();
        }
        g.targets.retain(|t| !t.paths.is_empty());
    }
    groups.retain(|g| !g.targets.is_empty());
    let all: Vec<(usize, usize, PathBuf)> = groups
        .iter()
        .enumerate()
        .flat_map(|(gi, g)| g.targets.iter().enumerate().flat_map(move |(ti, t)| t.paths.iter().map(move |p| (gi, ti, p.clone()))))
        .collect();
    for (gi, ti, p) in &all {
        for (gj, tj, q) in &all {
            if (gi, ti) != (gj, tj) && q != p && q.starts_with(p) {
                groups[*gi].targets[*ti].skip.push(q.clone());
            }
        }
    }
    groups
}

// ---- measuring and cleaning ----

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub bytes: u64,
    pub files: u64,
    /// Files that couldn't be deleted, usually because an app has them open.
    pub skipped: u64,
    pub denied: bool,
}

pub struct Measure {
    /// One entry per target, in group order.
    pub sizes: Mutex<Vec<Option<Stats>>>,
    pub done: AtomicBool,
}

/// Measures every target on its own thread.
pub fn measure_all(specs: Vec<(Vec<PathBuf>, Mode, Vec<PathBuf>)>) -> Arc<Measure> {
    let measure = Arc::new(Measure { sizes: Mutex::new(vec![None; specs.len()]), done: AtomicBool::new(false) });
    let worker = measure.clone();
    std::thread::spawn(move || {
        for (k, (paths, mode, skip)) in specs.into_iter().enumerate() {
            let stats = sweep_all(&paths, mode, &skip, false);
            worker.sizes.lock().unwrap()[k] = Some(stats);
        }
        worker.done.store(true, Relaxed);
    });
    measure
}

pub fn measure(paths: &[PathBuf], mode: Mode, skip: &[PathBuf]) -> Stats {
    sweep_all(paths, mode, skip, false)
}

/// Deletes what a target covers and says what went. Files in use are skipped.
pub fn clean(paths: &[PathBuf], mode: Mode, skip: &[PathBuf]) -> Stats {
    if mode == Mode::Bin && cfg!(any(windows, all(unix, not(target_os = "macos")))) {
        let before = measure(paths, mode, skip);
        empty_bin();
        let after = measure(paths, mode, skip);
        return Stats { bytes: before.bytes.saturating_sub(after.bytes), files: before.files.saturating_sub(after.files), skipped: after.files, denied: false };
    }
    sweep_all(paths, mode, skip, true)
}

#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
fn empty_bin() {
    if let Ok(items) = trash::os_limited::list()
        && !items.is_empty() {
            let _ = trash::os_limited::purge_all(items);
        }
}
#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
fn empty_bin() {}

fn sweep_all(paths: &[PathBuf], mode: Mode, skip: &[PathBuf], delete: bool) -> Stats {
    let mut stats = Stats::default();
    let cutoff = SystemTime::now() - Duration::from_secs(24 * 60 * 60);
    for path in paths {
        sweep(path, mode, skip, delete, cutoff, &mut stats, 0);
    }
    stats
}

/// Measures or deletes what is inside `dir`; true when `dir` is now empty.
fn sweep(dir: &Path, mode: Mode, skip: &[PathBuf], delete: bool, cutoff: SystemTime, stats: &mut Stats, depth: usize) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            if depth == 0 && e.kind() == ErrorKind::PermissionDenied {
                stats.denied = true;
            }
            return false;
        }
    };
    let mut empty = true;
    for entry in entries.flatten() {
        let path = entry.path();
        // Doesn't follow links, so nothing outside `dir` is ever touched.
        let Ok(meta) = entry.metadata() else {
            empty = false;
            continue;
        };
        if meta.file_type().is_symlink() || skip.contains(&path) {
            empty = false;
            continue;
        }
        if meta.is_dir() {
            if matches!(mode, Mode::Prefixed(_)) || depth > 64 {
                empty = false;
                continue;
            }
            let now_empty = sweep(&path, mode, skip, delete, cutoff, stats, depth + 1);
            if !(delete && now_empty && std::fs::remove_dir(&path).is_ok()) {
                empty = false;
            }
            continue;
        }
        if let Mode::Prefixed(prefixes) = mode {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !prefixes.iter().any(|p| name.starts_with(p)) {
                empty = false;
                continue;
            }
        }
        if mode == Mode::Settled && meta.modified().is_ok_and(|m| m > cutoff) {
            empty = false;
            continue;
        }
        let bytes = on_disk(&meta);
        if !delete {
            stats.bytes += bytes;
            stats.files += 1;
            continue;
        }
        if remove_file(&path) {
            stats.bytes += bytes;
            stats.files += 1;
        } else {
            stats.skipped += 1;
            empty = false;
        }
    }
    empty
}

fn remove_file(path: &Path) -> bool {
    match std::fs::remove_file(path) {
        Ok(()) => true,
        #[cfg(windows)]
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            // A read-only file: clear the flag and try once more.
            let Ok(meta) = std::fs::symlink_metadata(path) else { return false };
            let mut perms = meta.permissions();
            if !perms.readonly() {
                return false;
            }
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            std::fs::set_permissions(path, perms).is_ok() && std::fs::remove_file(path).is_ok()
        }
        Err(_) => false,
    }
}
