//! The rules that keep deletes safe: what a clean touches, what stays locked, and how the tree
//! follows removals.

use crate::app::{Where, lock};
use crate::junk::{self, Mode};
use crate::scan::{self, APPS, DOCS, OUTSIDE, SYSTEM, TRASH, Tree};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn sandbox(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("storage-scan-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![7u8; bytes]).unwrap();
}

fn age(path: &Path, days: u64) {
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(days * 86_400)).unwrap();
}

#[test]
fn a_clean_empties_the_folder_but_keeps_it() {
    let root = sandbox("contents");
    write(&root.join("a.bin"), 5000);
    write(&root.join("sub/deeper/b.bin"), 5000);
    let stats = junk::clean(&[root.clone()], Mode::Contents, &[]);
    assert_eq!(stats.files, 2);
    assert!(root.is_dir(), "the cleaned folder itself stays");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0, "emptied subfolders go too");
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn a_clean_never_follows_links_out() {
    let root = sandbox("links");
    let outside = sandbox("links-outside");
    write(&outside.join("precious.txt"), 100);
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
    write(&root.join("junk.bin"), 100);
    junk::clean(&[root.clone()], Mode::Contents, &[]);
    assert!(outside.join("precious.txt").exists(), "what a link points at is never touched");
    assert!(fs::symlink_metadata(root.join("link")).is_ok(), "the link itself is left alone");
    assert!(!root.join("junk.bin").exists());
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn temp_cleaning_keeps_files_from_the_last_day() {
    let root = sandbox("settled");
    write(&root.join("old.tmp"), 100);
    age(&root.join("old.tmp"), 3);
    write(&root.join("in-use.tmp"), 100);
    let stats = junk::clean(&[root.clone()], Mode::Settled, &[]);
    assert_eq!(stats.files, 1);
    assert!(!root.join("old.tmp").exists());
    assert!(root.join("in-use.tmp").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn folders_another_target_covers_are_skipped() {
    let root = sandbox("skip");
    write(&root.join("app/cache.bin"), 100);
    write(&root.join("pip/wheel.whl"), 100);
    let measured = junk::measure(&[root.clone()], Mode::Contents, &[root.join("pip")]);
    assert_eq!(measured.files, 1, "measuring and cleaning agree");
    junk::clean(&[root.clone()], Mode::Contents, &[root.join("pip")]);
    assert!(root.join("pip/wheel.whl").exists());
    assert!(!root.join("app/cache.bin").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn prefixed_targets_only_touch_matching_files_at_the_top() {
    let root = sandbox("prefixed");
    write(&root.join("thumbcache_256.db"), 100);
    write(&root.join("ExplorerStartupLog.etl"), 100);
    write(&root.join("nested/thumbcache_32.db"), 100);
    let stats = junk::clean(&[root.clone()], Mode::Prefixed(&["thumbcache_"]), &[]);
    assert_eq!(stats.files, 1);
    assert!(root.join("ExplorerStartupLog.etl").exists());
    assert!(root.join("nested/thumbcache_32.db").exists());
    let _ = fs::remove_dir_all(&root);
}

/// home ─┬ AppData ─ Local ─ Spotify (cache)
///       ├ Videos ─ trip.mp4
///       └ notes.txt
/// Program Files ─ App
/// Recycle Bin ─ old
fn tree() -> (Tree, [u32; 7]) {
    let mut t = Tree::default();
    let home = t.push_node("/home/sam".into(), 0, DOCS, 0);
    let appdata = t.push_node("AppData".into(), home, SYSTEM, 1);
    let local = t.push_node("Local".into(), appdata, SYSTEM, 2);
    let spotify = t.push_node("Spotify".into(), local, SYSTEM, 3);
    let videos = t.push_node("Videos".into(), home, scan::MOVIES, 1);
    let apps = t.push_node("/opt".into(), 0, APPS, OUTSIDE);
    let bin = t.push_node("/bin-root".into(), 0, TRASH, OUTSIDE);
    let mut own = |t: &mut Tree, i: u32, c: usize, b: u64| {
        let mut sums = [0; scan::N];
        sums[c] = b;
        t.set_own(i, sums, 1);
    };
    own(&mut t, home, DOCS, 10);
    own(&mut t, spotify, SYSTEM, 300);
    own(&mut t, videos, scan::MOVIES, 5000);
    own(&mut t, apps, APPS, 70);
    own(&mut t, bin, TRASH, 40);
    t.top.push(scan::FileEntry { dir: videos, name: "trip.mp4".into(), bytes: 5000, category: scan::MOVIES as u8, modified: 0 });
    t.finish();
    (t, [home, appdata, local, spotify, videos, apps, bin])
}

#[test]
fn what_stays_locked() {
    let (t, [home, appdata, local, spotify, videos, apps, bin]) = tree();
    assert!(lock(&t, &Where::Node(home)).is_some(), "the home folder itself");
    assert!(lock(&t, &Where::Node(appdata)).is_some(), "known top folders themselves");
    assert!(lock(&t, &Where::Node(videos)).is_some());
    assert!(lock(&t, &Where::Node(local)).is_some(), "AppData\\Local holds every app's data");
    assert!(lock(&t, &Where::Node(spotify)).is_none(), "one app's data folder can go");
    assert!(lock(&t, &Where::File { dir: videos, name: "trip.mp4".into() }).is_none());
    assert!(lock(&t, &Where::Node(apps)).is_some(), "apps are uninstalled, not deleted");
    assert!(lock(&t, &Where::File { dir: bin, name: "x".into() }).is_some(), "the bin is emptied from Cleaner");
}

#[test]
fn removals_flow_up_the_tree_and_out_of_the_lists() {
    let (mut t, [home, _, _, spotify, videos, _, _]) = tree();
    assert_eq!(t.node(home).total, 5310);
    t.remove_file(videos, "trip.mp4", 5000, scan::MOVIES);
    t.remove_node(spotify);
    t.finish();
    assert_eq!(t.node(home).total, 10);
    assert!(t.top.is_empty());
    assert_eq!(t.by_category()[scan::MOVIES], 0);
    assert_eq!(t.by_category()[SYSTEM], 0);
}

#[test]
fn a_folder_walked_again_replaces_its_old_contents() {
    let root = sandbox("graft");
    write(&root.join("left.bin"), 4096 * 3);
    let mut t = Tree::default();
    let at = t.push_node(root.to_string_lossy().into(), 0, DOCS, OUTSIDE);
    let gone = t.push_node("gone".into(), at, DOCS, OUTSIDE);
    let mut sums = [0; scan::N];
    sums[DOCS] = 1 << 30;
    t.set_own(gone, sums, 1);
    t.finish();
    assert_eq!(t.find(&root.join("gone")), Some(gone));
    t.graft(at, scan::subtree(&root, DOCS, OUTSIDE));
    t.finish();
    assert!(t.node(gone).removed);
    assert!(t.node(at).total < 1 << 20, "only what is still on disk counts");
    let _ = fs::remove_dir_all(&root);
}
