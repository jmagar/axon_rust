use super::*;
use std::os::windows::fs::{FileTimesExt as _, OpenOptionsExt as _};

#[test]
fn held_journal_directory_accepts_unchanged_root() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("journal");
    let directory = SecureJournalDir::open(&root).unwrap();
    std::fs::write(root.join("record.json"), b"original").unwrap();
    assert_eq!(
        directory.read(&root.join("record.json")).unwrap(),
        b"original"
    );
    directory.verify_path().unwrap();
}

#[test]
fn held_journal_directory_rejects_replacement_with_same_creation_time() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("journal");
    let displaced = temporary.path().join("displaced");
    let directory = SecureJournalDir::open(&root).unwrap();
    let created = std::fs::metadata(&root).unwrap().created().unwrap();
    std::fs::rename(&root, &displaced).unwrap();
    std::fs::create_dir(&root).unwrap();
    const FILE_WRITE_ATTRIBUTES: u32 = 0x100;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    std::fs::OpenOptions::new()
        .access_mode(FILE_WRITE_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(&root)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_created(created))
        .unwrap();
    assert_eq!(
        std::fs::metadata(&root).unwrap().created().unwrap(),
        created
    );
    std::fs::write(root.join("record.json"), b"replacement").unwrap();
    assert!(directory.verify_path().is_err());
    assert!(directory.read(&root.join("record.json")).is_err());
    assert_eq!(
        std::fs::read(root.join("record.json")).unwrap(),
        b"replacement"
    );
}
