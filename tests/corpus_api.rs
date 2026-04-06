use chiff::{analyze_directory_pair, CorpusEntryStatus};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn analyze_directory_pair_reports_missing_entries_and_summary() {
    let fixture_root = unique_temp_dir("chiff-corpus-api");
    let old_root = fixture_root.join("old");
    let new_root = fixture_root.join("new");

    fs::create_dir_all(old_root.join("paired")).unwrap();
    fs::create_dir_all(new_root.join("paired")).unwrap();
    fs::create_dir_all(old_root.join("old-only")).unwrap();
    fs::create_dir_all(new_root.join("new-only")).unwrap();

    fs::write(old_root.join("paired/file.txt"), "const a = 1;\n").unwrap();
    fs::write(new_root.join("paired/file.txt"), "const a = 2;\n").unwrap();
    fs::write(old_root.join("old-only/blob.bin"), [0_u8, 1, 2, 3]).unwrap();
    fs::write(new_root.join("new-only/blob.bin"), [4_u8, 5, 6, 7]).unwrap();

    let corpus = analyze_directory_pair(&old_root, &new_root).unwrap();

    assert_eq!(corpus.summary.paired, 1);
    assert_eq!(corpus.summary.copy_ops, 2);
    assert_eq!(corpus.summary.insert_ops, 1);
    assert_eq!(corpus.summary.copied_bytes, 12);
    assert_eq!(corpus.summary.inserted_bytes, 1);
    assert_eq!(
        corpus.summary.reason_counts,
        BTreeMap::from([(String::from("text_pair"), 1)])
    );
    assert_eq!(
        corpus.summary.old_support_counts,
        BTreeMap::from([(String::from("not_hermes"), 1)])
    );
    assert_eq!(
        corpus.summary.new_support_counts,
        BTreeMap::from([(String::from("not_hermes"), 1)])
    );

    assert_eq!(corpus.entries.len(), 3);
    assert_eq!(
        corpus.entries[0].relative_path,
        PathBuf::from("new-only/blob.bin")
    );
    assert_eq!(corpus.entries[0].status, CorpusEntryStatus::MissingInOld);
    assert!(corpus.entries[0].diff_analysis.is_none());

    assert_eq!(
        corpus.entries[1].relative_path,
        PathBuf::from("old-only/blob.bin")
    );
    assert_eq!(corpus.entries[1].status, CorpusEntryStatus::MissingInNew);
    assert!(corpus.entries[1].diff_analysis.is_none());

    assert_eq!(
        corpus.entries[2].relative_path,
        PathBuf::from("paired/file.txt")
    );
    assert_eq!(corpus.entries[2].status, CorpusEntryStatus::Paired);
    assert!(corpus.entries[2].diff_analysis.is_some());

    remove_dir_if_exists(&fixture_root);
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}",
        prefix,
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    remove_dir_if_exists(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn remove_dir_if_exists(path: &Path) {
    match fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("failed to remove {}: {}", path.display(), error),
    }
}
