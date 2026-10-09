use lore::context::memory;
use std::{
    fs,
    time::{Duration, SystemTime},
};

fn path(root: &std::path::Path, n: usize) -> std::path::PathBuf {
    root.join(format!("decision-{n:064x}.json"))
}

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    // macOS temporary roots may use /var -> /private/var. Give the guard
    // the real fixture root; symlinks inside it remain forbidden.
    let root = dir.path().canonicalize().unwrap();
    (dir, root)
}

#[test]
fn retention_is_bounded_and_only_managed_decision_findings_are_evicted() {
    let (_dir, root) = fixture();
    fs::write(root.join("user-notes.json"), "keep").unwrap();
    for n in 0..70 {
        fs::write(path(&root, n), "{}").unwrap();
    }
    let destination = path(&root, 99);
    let report = memory::prepare_write(&root, &destination).unwrap();
    assert_eq!(report.removed, 7);
    assert_eq!(report.retained, memory::MAX_FINDINGS - 1);
    fs::write(destination, "{}").unwrap();
    assert_eq!(
        fs::read_to_string(root.join("user-notes.json")).unwrap(),
        "keep"
    );
    assert_eq!(
        fs::read_dir(&root).unwrap().count(),
        memory::MAX_FINDINGS + 1
    );
}

#[test]
fn expired_and_oversized_records_are_removed_before_new_persistence() {
    let (_dir, root) = fixture();
    let expired = path(&root, 0);
    fs::write(&expired, "{}").unwrap();
    fs::File::options()
        .write(true)
        .open(&expired)
        .unwrap()
        .set_times(
            fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(31 * 86_400)),
        )
        .unwrap();
    let oversized = path(&root, 1);
    fs::write(&oversized, vec![b'x'; memory::MAX_FINDING_BYTES + 1]).unwrap();
    let report = memory::prepare_write(&root, &path(&root, 2)).unwrap();
    assert_eq!(report.removed, 2);
    assert!(!expired.exists() && !oversized.exists());
}

#[test]
fn corrupted_interrupted_and_future_metadata_cannot_be_a_fresh_finding() {
    assert!(!memory::fresh("not a date"));
    assert!(!memory::fresh("2000-01-01T00:00:00Z"));
    assert!(!memory::fresh("2999-01-01T00:00:00Z"));
    assert!(memory::fresh(&lore::util::now()));
    let (_dir, root) = fixture();
    fs::write(root.join(".lore-write-interrupted"), "partial").unwrap();
    let report = memory::prepare_write(&root, &path(&root, 1)).unwrap();
    assert_eq!(report.retained, 0);
    assert_eq!(report.removed, 0);
}

#[cfg(unix)]
#[test]
fn retention_does_not_follow_symlinks() {
    let (_dir, root) = fixture();
    let source = root.join("source.txt");
    fs::write(&source, "source").unwrap();
    std::os::unix::fs::symlink(&source, path(&root, 1)).unwrap();
    assert!(memory::prepare_write(&root, &path(&root, 2)).is_err());
    assert_eq!(fs::read_to_string(source).unwrap(), "source");
}
