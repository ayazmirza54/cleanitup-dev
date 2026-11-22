//! Work that changes the disk, on its own thread: moving to the Recycle Bin, deleting, and
//! cleaning junk. Each piece reports how it went so the UI can update its tree.

use crate::junk::{self, Mode};
use crate::scan::{self, Tree};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

/// A folder to walk again once the work is done, and the node it replaces.
#[derive(Clone)]
pub struct Rescan {
    pub node: u32,
    pub path: PathBuf,
    pub category: usize,
    pub depth: u8,
}

pub struct CleanJob {
    pub name: String,
    pub paths: Vec<PathBuf>,
    pub mode: Mode,
    pub skip: Vec<PathBuf>,
    /// What the Cleaner measured, which a dry run reports as freed.
    pub bytes: u64,
}

pub enum Work {
    Clean(Vec<CleanJob>),
    Recycle(Vec<PathBuf>),
    Delete(Vec<PathBuf>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Done,
    Failed(String),
}

#[derive(Default)]
pub struct Op {
    pub steps: usize,
    pub step: AtomicUsize,
    /// Each step's name, for a dry run to show as it plays.
    pub names: Vec<String>,
    pub current: Mutex<String>,
    pub freed: AtomicU64,
    pub removed: AtomicU64,
    pub skipped: AtomicU64,
    /// One per item (Recycle, Delete) or per job (Clean), in order.
    pub outcomes: Mutex<Vec<Outcome>>,
    /// Cleaned targets measured again, for the Cleaner's list.
    pub left: Mutex<Vec<junk::Stats>>,
    /// Fresh walks to graft over the tree.
    pub rescans: Mutex<Vec<(u32, Tree)>>,
    pub done: AtomicBool,
}

impl Op {
    pub fn fraction(&self) -> f32 {
        if self.steps == 0 { 1.0 } else { self.step.load(Relaxed) as f32 / self.steps as f32 }
    }
}

/// Starts `work`. A `dry` run touches nothing on disk (the demo); `after` lists folders to
/// walk again when it ends.
pub fn run(work: Work, dry: bool, after: Vec<Rescan>) -> Arc<Op> {
    let steps = match &work {
        Work::Clean(jobs) => jobs.len(),
        Work::Recycle(items) | Work::Delete(items) => items.len(),
    };
    let names: Vec<String> = match &work {
        Work::Clean(jobs) => jobs.iter().map(|j| j.name.clone()).collect(),
        Work::Recycle(items) | Work::Delete(items) => items.iter().map(|p| name_of(p)).collect(),
    };
    if dry {
        // Nothing happens on disk; the caller plays the steps on its own clock and sets
        // `done`, so recordings of the demo come out the same every time.
        let op = Op { steps, names, ..Op::default() };
        if let Work::Clean(jobs) = &work {
            for job in jobs {
                op.freed.fetch_add(job.bytes, Relaxed);
                op.left.lock().unwrap().push(junk::Stats::default());
            }
        }
        *op.outcomes.lock().unwrap() = vec![Outcome::Done; steps];
        return Arc::new(op);
    }
    let op = Arc::new(Op { steps, names, ..Op::default() });
    let worker = op.clone();
    std::thread::spawn(move || {
        match work {
            Work::Clean(jobs) => {
                for job in jobs {
                    *worker.current.lock().unwrap() = job.name.clone();
                    let stats = junk::clean(&job.paths, job.mode, &job.skip);
                    let left = junk::measure(&job.paths, job.mode, &job.skip);
                    worker.freed.fetch_add(stats.bytes, Relaxed);
                    worker.removed.fetch_add(stats.files, Relaxed);
                    worker.skipped.fetch_add(stats.skipped, Relaxed);
                    worker.left.lock().unwrap().push(left);
                    worker.outcomes.lock().unwrap().push(Outcome::Done);
                    worker.step.fetch_add(1, Relaxed);
                }
            }
            Work::Recycle(items) => {
                for item in items {
                    *worker.current.lock().unwrap() = name_of(&item);
                    let outcome = match trash::delete(&item) {
                        Ok(()) => Outcome::Done,
                        Err(e) => Outcome::Failed(e.to_string()),
                    };
                    worker.outcomes.lock().unwrap().push(outcome);
                    worker.step.fetch_add(1, Relaxed);
                }
            }
            Work::Delete(items) => {
                for item in items {
                    *worker.current.lock().unwrap() = name_of(&item);
                    let result = match std::fs::symlink_metadata(&item) {
                        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&item),
                        Ok(_) => std::fs::remove_file(&item),
                        Err(e) => Err(e),
                    };
                    let outcome = match result {
                        Ok(()) => Outcome::Done,
                        Err(e) => Outcome::Failed(e.to_string()),
                    };
                    worker.outcomes.lock().unwrap().push(outcome);
                    worker.step.fetch_add(1, Relaxed);
                }
            }
        }
        for r in after {
            *worker.current.lock().unwrap() = "Updating sizes".into();
            let tree = scan::subtree(&r.path, r.category, r.depth);
            worker.rescans.lock().unwrap().push((r.node, tree));
        }
        worker.done.store(true, Relaxed);
    });
    op
}

fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(|| path.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
}

/// Opens the file manager with `path` selected.
pub fn reveal(path: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let dir = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}

pub const BIN: &str = if cfg!(windows) { "Recycle Bin" } else { "Trash" };
pub const FILE_MANAGER: &str = if cfg!(windows) {
    "Explorer"
} else if cfg!(target_os = "macos") {
    "Finder"
} else {
    "Files"
};
