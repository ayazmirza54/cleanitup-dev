//! Walks folders on a background thread into a tree of sizes, sorting every file into one of
//! nine categories. The UI reads running totals from atomics while the walk runs, then takes
//! the finished [`Tree`].

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

pub const N: usize = 9;
pub const APPS: usize = 0;
pub const DEV: usize = 1;
pub const PHOTOS: usize = 2;
pub const MOVIES: usize = 3;
pub const DOCS: usize = 4;
pub const MUSIC: usize = 5;
pub const SYSTEM: usize = 6;
pub const CACHES: usize = 7;
pub const TRASH: usize = 8;

/// No node: the parent of the tree's top, or a node without a category split.
pub const NONE: u32 = u32::MAX;
/// `Node::depth` of a folder outside the home folder.
pub const OUTSIDE: u8 = u8::MAX;
/// How many of the largest files the tree keeps.
const TOP: usize = 600;

pub struct Node {
    /// A location's full path for the top's children, otherwise the folder's own name.
    pub name: Box<str>,
    pub parent: u32,
    pub category: u8,
    /// Levels below the home folder (0 is home itself), or [`OUTSIDE`].
    pub depth: u8,
    pub removed: bool,
    /// Bytes of the files directly inside.
    pub own: u64,
    pub own_files: u32,
    /// Index into [`Tree::splits`] when the files directly inside span several categories.
    pub split: u32,
    /// `own` plus every live folder below it.
    pub total: u64,
    pub files: u64,
    /// Live children, largest first once [`Tree::finish`] has run.
    pub children: Vec<u32>,
}

#[derive(Clone)]
pub struct FileEntry {
    pub dir: u32,
    pub name: Box<str>,
    pub bytes: u64,
    pub category: u8,
    /// Seconds since the Unix epoch.
    pub modified: u64,
}

pub struct Tree {
    /// Node 0 is a nameless top; its children are the scanned locations.
    pub nodes: Vec<Node>,
    pub splits: Vec<[u64; N]>,
    /// The largest files in the home folder, largest first.
    pub top: Vec<FileEntry>,
    /// Joins names into paths for display.
    pub sep: char,
}

impl Default for Tree {
    fn default() -> Self {
        let mut tree = Tree { nodes: Vec::new(), splits: Vec::new(), top: Vec::new(), sep: std::path::MAIN_SEPARATOR };
        tree.push_node("".into(), NONE, DOCS, OUTSIDE);
        tree
    }
}

impl Tree {
    pub fn push_node(&mut self, name: Box<str>, parent: u32, category: usize, depth: u8) -> u32 {
        let i = self.nodes.len() as u32;
        self.nodes.push(Node {
            name,
            parent,
            category: category as u8,
            depth,
            removed: false,
            own: 0,
            own_files: 0,
            split: NONE,
            total: 0,
            files: 0,
            children: Vec::new(),
        });
        if parent != NONE {
            self.nodes[parent as usize].children.push(i);
        }
        i
    }

    pub fn node(&self, i: u32) -> &Node {
        &self.nodes[i as usize]
    }

    /// Sets the bytes directly inside `i`, by category.
    pub fn set_own(&mut self, i: u32, sums: [u64; N], files: u32) {
        let node = &mut self.nodes[i as usize];
        node.own = sums.iter().sum();
        node.own_files = files;
        node.split = NONE;
        if sums.iter().filter(|&&b| b > 0).count() > 1 {
            node.split = self.splits.len() as u32;
            self.splits.push(sums);
        }
    }

    /// Totals, file counts and child order, after a walk or any change.
    pub fn finish(&mut self) {
        for node in &mut self.nodes {
            (node.total, node.files) = if node.removed { (0, 0) } else { (node.own, u64::from(node.own_files)) };
        }
        for i in (1..self.nodes.len()).rev() {
            let node = &self.nodes[i];
            if node.removed || node.parent == NONE {
                continue;
            }
            let (parent, total, files) = (node.parent as usize, node.total, node.files);
            self.nodes[parent].total += total;
            self.nodes[parent].files += files;
        }
        for i in 0..self.nodes.len() {
            let mut kids = std::mem::take(&mut self.nodes[i].children);
            kids.retain(|&c| !self.nodes[c as usize].removed);
            kids.sort_by_key(|&c| std::cmp::Reverse(self.nodes[c as usize].total));
            self.nodes[i].children = kids;
        }
        let nodes = &self.nodes;
        self.top.retain(|f| !nodes[f.dir as usize].removed);
        self.top.sort_by_key(|f| std::cmp::Reverse(f.bytes));
        self.top.truncate(TOP);
    }

    /// Live bytes by category.
    pub fn by_category(&self) -> [u64; N] {
        let mut out = [0; N];
        for node in &self.nodes {
            if node.removed {
                continue;
            }
            if node.split != NONE {
                for (o, b) in out.iter_mut().zip(self.splits[node.split as usize]) {
                    *o += b;
                }
            } else {
                out[node.category as usize] += node.own;
            }
        }
        out
    }

    pub fn total(&self) -> u64 {
        self.nodes[0].total
    }

    pub fn path(&self, mut i: u32) -> PathBuf {
        let mut parts = Vec::new();
        while i != 0 && i != NONE {
            parts.push(&*self.node(i).name);
            if self.node(i).parent == 0 {
                break;
            }
            i = self.node(i).parent;
        }
        let mut path = PathBuf::from(parts.pop().unwrap_or_default());
        for part in parts.iter().rev() {
            path.push(part);
        }
        path
    }

    /// [`Self::path`] joined with the tree's separator, for display.
    pub fn display(&self, mut i: u32) -> String {
        let mut parts = Vec::new();
        while i != 0 && i != NONE {
            parts.push(&*self.node(i).name);
            if self.node(i).parent == 0 {
                break;
            }
            i = self.node(i).parent;
        }
        parts.reverse();
        let mut out = String::new();
        for (k, part) in parts.iter().enumerate() {
            if k > 0 && !out.ends_with(self.sep) {
                out.push(self.sep);
            }
            out.push_str(part);
        }
        out
    }

    /// The live node at `path`, if a scanned location holds it.
    pub fn find(&self, path: &Path) -> Option<u32> {
        let mut best: Option<(usize, u32)> = None;
        for &root in &self.node(0).children {
            let root_path = Path::new(&*self.node(root).name);
            if path.starts_with(root_path) {
                let depth = root_path.components().count();
                if best.is_none_or(|(d, _)| depth > d) {
                    best = Some((depth, root));
                }
            }
        }
        let (_, root) = best?;
        let rest = path.strip_prefix(Path::new(&*self.node(root).name)).ok()?;
        let mut at = root;
        for part in rest.components() {
            let name = part.as_os_str().to_string_lossy();
            at = *self.node(at).children.iter().find(|&&c| same_name(&self.node(c).name, &name))?;
        }
        Some(at)
    }

    pub fn within(&self, mut i: u32, ancestor: u32) -> bool {
        while i != NONE {
            if i == ancestor {
                return true;
            }
            i = self.node(i).parent;
        }
        false
    }

    fn mark_removed(&mut self, i: u32) {
        let mut stack = vec![i];
        while let Some(i) = stack.pop() {
            let node = &mut self.nodes[i as usize];
            node.removed = true;
            stack.extend(node.children.iter().copied());
        }
    }

    /// Drops a folder and everything in it. Call [`Self::finish`] after.
    pub fn remove_node(&mut self, i: u32) {
        self.mark_removed(i);
    }

    /// Drops one file directly inside `dir`. Call [`Self::finish`] after.
    pub fn remove_file(&mut self, dir: u32, name: &str, bytes: u64, category: usize) {
        let node = &mut self.nodes[dir as usize];
        node.own = node.own.saturating_sub(bytes);
        node.own_files = node.own_files.saturating_sub(1);
        if node.split != NONE {
            let split = &mut self.splits[node.split as usize][category];
            *split = split.saturating_sub(bytes);
        }
        self.top.retain(|f| !(f.dir == dir && &*f.name == name));
    }

    /// Empties a folder in place: what a cleanup that couldn't rescan it leaves.
    pub fn empty_node(&mut self, i: u32) {
        for c in self.node(i).children.clone() {
            self.mark_removed(c);
        }
        self.set_own(i, [0; N], 0);
        let nodes = &self.nodes;
        let keep: Vec<bool> = self.top.iter().map(|f| !self.within(f.dir, i) && !nodes[f.dir as usize].removed).collect();
        let mut k = keep.iter();
        self.top.retain(|_| *k.next().unwrap_or(&true));
    }

    /// Replaces what is under `at` with a fresh walk of the same folder (see [`subtree`]).
    /// Call [`Self::finish`] after.
    pub fn graft(&mut self, at: u32, mut sub: Tree) {
        self.empty_node(at);
        if sub.nodes.len() < 2 {
            return;
        }
        let base = self.nodes.len() as u32;
        let map = |k: u32| if k == 1 { at } else { base + k - 2 };
        let root_split = sub.nodes[1].split;
        let sums = if root_split != NONE { sub.splits[root_split as usize] } else {
            let mut s = [0; N];
            s[sub.nodes[1].category as usize] = sub.nodes[1].own;
            s
        };
        let files = sub.nodes[1].own_files;
        // The fresh walk names its root by its whole path; keep this node's own name.
        let category = self.node(at).category as usize;
        let mut sums_here = [0; N];
        for (k, b) in sums.iter().enumerate() {
            sums_here[if root_split != NONE { k } else { category }] += b;
        }
        self.set_own(at, sums_here, files);
        let kids: Vec<u32> = sub.nodes[1].children.iter().map(|&c| map(c)).collect();
        for k in 2..sub.nodes.len() {
            let node = &mut sub.nodes[k];
            let mut moved = Node {
                name: std::mem::take(&mut node.name),
                parent: map(node.parent),
                category: node.category,
                depth: node.depth,
                removed: node.removed,
                own: node.own,
                own_files: node.own_files,
                split: node.split,
                total: node.total,
                files: node.files,
                children: node.children.iter().map(|&c| map(c)).collect(),
            };
            if moved.split != NONE {
                let split = sub.splits[moved.split as usize];
                moved.split = self.splits.len() as u32;
                self.splits.push(split);
            }
            self.nodes.push(moved);
        }
        self.nodes[at as usize].children = kids;
        for mut f in sub.top {
            f.dir = map(f.dir);
            self.top.push(f);
        }
    }
}

fn same_name(a: &str, b: &str) -> bool {
    if cfg!(any(windows, target_os = "macos")) { a.eq_ignore_ascii_case(b) } else { a == b }
}

// ---- walking ----

#[derive(Default)]
pub struct Progress {
    pub bytes: [AtomicU64; N],
    pub files: AtomicU64,
    pub path: Mutex<String>,
    pub done: AtomicBool,
    pub cancel: AtomicBool,
    pub tree: Mutex<Option<Tree>>,
}

pub fn home() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// Capacity and used bytes of the volume the home folder is on.
pub fn disk() -> (u64, u64) {
    let home = home();
    let total = fs4::total_space(&home).unwrap_or(0);
    let free = fs4::free_space(&home).unwrap_or(0);
    (total, total.saturating_sub(free))
}

/// The whole scan, on its own thread.
pub fn start() -> Arc<Progress> {
    let progress = Arc::new(Progress::default());
    let worker = progress.clone();
    std::thread::spawn(move || {
        let home = home();
        let mut tree = Tree::default();
        let mut walker = Walker::new(Some(&worker), &home);
        for (root, category) in roots(&home) {
            let depth = if root == home { 0 } else { OUTSIDE };
            let name: Box<str> = root.to_string_lossy().into();
            walker.walk(&mut tree, &root, name, 0, category, depth);
            if worker.cancel.load(Relaxed) {
                return;
            }
        }
        walker.flush(&mut tree);
        tree.finish();
        *worker.tree.lock().unwrap() = Some(tree);
        worker.done.store(true, Relaxed);
    });
    progress
}

/// A fresh walk of one folder, to graft over its node after a cleanup. Its root is node 1.
pub fn subtree(path: &Path, category: usize, depth: u8) -> Tree {
    let mut tree = Tree::default();
    let mut walker = Walker::new(None, &home());
    walker.walk(&mut tree, path, path.to_string_lossy().into(), 0, category, depth);
    walker.flush(&mut tree);
    tree.finish();
    tree
}

fn roots(home: &Path) -> Vec<(PathBuf, usize)> {
    let mut roots = vec![(home.to_path_buf(), DOCS)];
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(target_os = "macos") {
        roots.push(("/Applications".into(), APPS));
        roots.push(("/Library/Caches".into(), CACHES));
    } else if cfg!(windows) {
        roots.extend(env("ProgramFiles").map(|p| (p, APPS)));
        roots.extend(env("ProgramFiles(x86)").map(|p| (p, APPS)));
        roots.extend(env("ProgramData").map(|p| (p, SYSTEM)));
        roots.extend(env("SystemDrive").map(|d| (PathBuf::from(format!("{}\\$Recycle.Bin", d.to_string_lossy())), TRASH)));
    } else {
        roots.push(("/opt".into(), APPS));
        roots.push(("/usr".into(), APPS));
    }
    roots.dedup();
    roots
}

const DEV_DIRS: &[&str] = &[
    "node_modules", ".git", ".cargo", ".rustup", ".npm", ".pnpm-store", ".yarn", ".gradle", ".m2", ".nuget", ".venv", "venv",
    "__pycache__", "deriveddata", "developer", ".android", ".docker", ".vscode", ".conda", "go", ".pyenv", ".nvm", ".bun",
];
const CACHE_DIRS: &[&str] = &["cache", "caches", ".cache", "temp", "tmp", "code cache", "gpucache", "cacheddata", "inetcache", "crashdumps"];
const IMAGE: &[&str] = &["jpg", "jpeg", "png", "heic", "heif", "gif", "raw", "cr2", "nef", "arw", "dng", "tif", "tiff", "bmp", "webp", "psd"];
const VIDEO: &[&str] = &["mp4", "mov", "mkv", "avi", "m4v", "webm", "wmv", "flv", "mpg", "mpeg", "mts"];
const AUDIO: &[&str] = &["mp3", "m4a", "flac", "wav", "aac", "ogg", "aiff", "wma", "opus"];

/// The category of a folder, from its lowercased name and its parent's category and depth.
pub fn dir_category(name: &str, parent: usize, depth: u8) -> usize {
    if matches!(parent, APPS | DEV | CACHES | TRASH) {
        return parent;
    }
    if DEV_DIRS.contains(&name) && (name != "go" || depth == 1) {
        return DEV;
    }
    if CACHE_DIRS.contains(&name) {
        return CACHES;
    }
    if depth == 1 {
        return match name {
            ".trash" => TRASH,
            "pictures" | "photos" => PHOTOS,
            "movies" | "videos" => MOVIES,
            "music" => MUSIC,
            "applications" => APPS,
            "library" | "appdata" => SYSTEM,
            _ if name.starts_with('.') => SYSTEM,
            _ => parent,
        };
    }
    if parent == SYSTEM {
        return match name {
            "trash" => TRASH,
            "programs" => APPS,
            _ => SYSTEM,
        };
    }
    parent
}

pub fn file_category(name: &str, dir: usize) -> usize {
    if dir != DOCS {
        return dir;
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let ext = ext.as_str();
    if IMAGE.contains(&ext) {
        PHOTOS
    } else if VIDEO.contains(&ext) {
        MOVIES
    } else if AUDIO.contains(&ext) {
        MUSIC
    } else {
        DOCS
    }
}

/// Bytes a file takes on disk. Cloud placeholders (OneDrive, iCloud) take none.
#[cfg(unix)]
pub fn on_disk(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.blocks() * 512
}
#[cfg(windows)]
pub fn on_disk(meta: &std::fs::Metadata) -> u64 {
    use std::os::windows::fs::MetadataExt;
    const OFFLINE: u32 = 0x1000;
    const RECALL_ON_OPEN: u32 = 0x40000;
    const RECALL_ON_DATA_ACCESS: u32 = 0x400000;
    if meta.file_attributes() & (OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0 { 0 } else { meta.len() }
}
#[cfg(not(any(unix, windows)))]
pub fn on_disk(meta: &std::fs::Metadata) -> u64 {
    meta.len()
}

#[cfg(unix)]
fn device(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.dev()
}
#[cfg(not(unix))]
fn device(_: &std::fs::Metadata) -> u64 {
    0
}

pub fn modified(meta: &std::fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

struct Walker<'a> {
    progress: Option<&'a Progress>,
    home: String,
    seen: u64,
    /// Candidates for [`Tree::top`], trimmed as they pile up.
    top: Vec<FileEntry>,
    floor: u64,
}

impl<'a> Walker<'a> {
    fn new(progress: Option<&'a Progress>, home: &Path) -> Self {
        Walker { progress, home: home.to_string_lossy().into_owned(), seen: 0, top: Vec::new(), floor: 0 }
    }

    fn offer(&mut self, file: FileEntry) {
        if self.top.len() >= TOP && file.bytes <= self.floor {
            return;
        }
        self.top.push(file);
        if self.top.len() >= TOP * 2 {
            self.top.sort_by_key(|f| std::cmp::Reverse(f.bytes));
            self.top.truncate(TOP);
            self.floor = self.top.last().map_or(0, |f| f.bytes);
        }
    }

    fn flush(&mut self, tree: &mut Tree) {
        tree.top.append(&mut self.top);
    }

    fn walk(&mut self, tree: &mut Tree, root: &Path, name: Box<str>, parent: u32, category: usize, depth: u8) -> Option<u32> {
        let meta = std::fs::symlink_metadata(root).ok()?;
        if !meta.is_dir() {
            return None;
        }
        let dev = device(&meta);
        let root_node = tree.push_node(name, parent, category, depth);
        let mut stack = vec![(root_node, root.to_path_buf())];
        while let Some((at, dir)) = stack.pop() {
            if let Some(p) = self.progress {
                if p.cancel.load(Relaxed) {
                    return None;
                }
                self.seen += 1;
                if self.seen % 48 == 1 {
                    let shown = dir.to_string_lossy();
                    let shown = match shown.strip_prefix(&self.home) {
                        Some(rest) => format!("~{rest}"),
                        None => shown.into_owned(),
                    };
                    *p.path.lock().unwrap() = shown;
                }
            }
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            let (category, depth) = (tree.node(at).category as usize, tree.node(at).depth);
            let mut sums = [0u64; N];
            let mut files = 0u32;
            for entry in entries.flatten() {
                // Doesn't follow links; on Windows it comes with the listing.
                let Ok(meta) = entry.metadata() else { continue };
                let kind = meta.file_type();
                if kind.is_symlink() {
                    continue;
                }
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if kind.is_dir() {
                    // Stay on this volume: no network mounts, no /proc.
                    if device(&meta) != dev {
                        continue;
                    }
                    let child_depth = if depth == OUTSIDE { OUTSIDE } else { depth.saturating_add(1).min(OUTSIDE - 1) };
                    let child_category = if depth == OUTSIDE { category } else { dir_category(&name.to_lowercase(), category, child_depth) };
                    let child = tree.push_node(name.as_ref().into(), at, child_category, child_depth);
                    stack.push((child, entry.path()));
                } else if kind.is_file() {
                    let file_cat = file_category(&name, category);
                    let bytes = on_disk(&meta);
                    sums[file_cat] += bytes;
                    files += 1;
                    if depth != OUTSIDE && bytes >= 1 << 20 {
                        self.offer(FileEntry { dir: at, name: name.as_ref().into(), bytes, category: file_cat as u8, modified: modified(&meta) });
                    }
                }
            }
            tree.set_own(at, sums, files);
            if let Some(p) = self.progress {
                for (total, sum) in p.bytes.iter().zip(sums) {
                    if sum > 0 {
                        total.fetch_add(sum, Relaxed);
                    }
                }
                p.files.fetch_add(u64::from(files), Relaxed);
            }
        }
        Some(root_node)
    }
}
