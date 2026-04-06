use chiff::analyze_directory_pair;
use std::{collections::BTreeMap, path::PathBuf};

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

#[test]
fn mixed_baseline_corpus_has_expected_reason_and_support_summary() {
    let old_root = fixture_path("fixtures/corpus/mixed-baseline/old");
    let new_root = fixture_path("fixtures/corpus/mixed-baseline/new");
    let corpus = analyze_directory_pair(&old_root, &new_root).unwrap();

    assert_eq!(corpus.summary.paired, 7);
    assert_eq!(corpus.summary.copy_ops, 4_658);
    assert_eq!(corpus.summary.insert_ops, 17);
    assert_eq!(corpus.summary.copied_bytes, 4_791_603);
    assert_eq!(corpus.summary.inserted_bytes, 113);

    assert_eq!(
        corpus.summary.reason_counts,
        BTreeMap::from([
            (String::from("binary_pair"), 1),
            (String::from("hermes_form_mismatch"), 1),
            (String::from("hermes_old_invalid_header"), 1),
            (String::from("hermes_old_unsupported_version"), 1),
            (String::from("hermes_structured"), 1),
            (String::from("hermes_version_mismatch"), 1),
            (String::from("text_pair"), 1),
        ])
    );
    assert_eq!(
        corpus.summary.old_support_counts,
        BTreeMap::from([
            (String::from("invalid_header"), 1),
            (String::from("not_hermes"), 2),
            (String::from("supported"), 3),
            (String::from("unsupported_version"), 1),
        ])
    );
    assert_eq!(
        corpus.summary.new_support_counts,
        BTreeMap::from([
            (String::from("invalid_header"), 1),
            (String::from("not_hermes"), 2),
            (String::from("supported"), 3),
            (String::from("unsupported_version"), 1),
        ])
    );
}
