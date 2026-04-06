use chiff::{analyze_diff, apply_patch, EngineKind, EngineReason, StructuredHermesSupport};
use std::{fs, path::PathBuf};

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

#[test]
fn synthetic_version_mismatch_pair_falls_back_to_generic_binary() {
    let old = fs::read(fixture_path(
        "fixtures/synthetic/hermes-fallback/pairs/version-mismatch/v1/hermes/index.android.hbc",
    ))
    .unwrap();
    let new = fs::read(fixture_path(
        "fixtures/synthetic/hermes-fallback/pairs/version-mismatch/v2/hermes/index.android.hbc",
    ))
    .unwrap();

    let analysis = analyze_diff(&old, &new);

    assert_eq!(analysis.engine_decision.kind, EngineKind::GenericBinary);
    assert_eq!(
        analysis.engine_decision.reason,
        EngineReason::HermesVersionMismatch
    );
    assert_eq!(
        analysis.old_structured_hermes_support,
        StructuredHermesSupport::Supported {
            version: 98,
            form: chiff::HermesForm::Execution,
        }
    );
    assert_eq!(
        analysis.new_structured_hermes_support,
        StructuredHermesSupport::Supported {
            version: 99,
            form: chiff::HermesForm::Execution,
        }
    );
    assert_eq!(analysis.stats.op_count, 6);
    assert_eq!(analysis.stats.inserted_bytes, 8);
    assert_eq!(apply_patch(&old, &analysis.patch).unwrap(), new);
}

#[test]
fn synthetic_form_mismatch_pair_falls_back_to_generic_binary() {
    let old = fs::read(fixture_path(
        "fixtures/synthetic/hermes-fallback/pairs/form-mismatch/v1/hermes/index.android.hbc",
    ))
    .unwrap();
    let new = fs::read(fixture_path(
        "fixtures/synthetic/hermes-fallback/pairs/form-mismatch/v2/hermes/index.android.hbc",
    ))
    .unwrap();

    let analysis = analyze_diff(&old, &new);

    assert_eq!(analysis.engine_decision.kind, EngineKind::GenericBinary);
    assert_eq!(
        analysis.engine_decision.reason,
        EngineReason::HermesFormMismatch
    );
    assert_eq!(
        analysis.old_structured_hermes_support,
        StructuredHermesSupport::Supported {
            version: 99,
            form: chiff::HermesForm::Execution,
        }
    );
    assert_eq!(
        analysis.new_structured_hermes_support,
        StructuredHermesSupport::Supported {
            version: 99,
            form: chiff::HermesForm::Delta,
        }
    );
    assert_eq!(analysis.stats.op_count, 5);
    assert_eq!(analysis.stats.inserted_bytes, 17);
    assert_eq!(apply_patch(&old, &analysis.patch).unwrap(), new);
}

#[test]
fn synthetic_unsupported_version_pair_falls_back_to_generic_binary() {
    let old = fs::read(fixture_path(
        "fixtures/synthetic/hermes-fallback/pairs/unsupported-version/v1/hermes/index.android.hbc",
    ))
    .unwrap();
    let new = fs::read(fixture_path(
        "fixtures/synthetic/hermes-fallback/pairs/unsupported-version/v2/hermes/index.android.hbc",
    ))
    .unwrap();

    let analysis = analyze_diff(&old, &new);

    assert_eq!(analysis.engine_decision.kind, EngineKind::GenericBinary);
    assert_eq!(
        analysis.engine_decision.reason,
        EngineReason::HermesOldUnsupportedVersion
    );
    assert_eq!(
        analysis.old_structured_hermes_support,
        StructuredHermesSupport::UnsupportedVersion {
            version: 100,
            form: chiff::HermesForm::Execution,
        }
    );
    assert_eq!(
        analysis.new_structured_hermes_support,
        StructuredHermesSupport::UnsupportedVersion {
            version: 100,
            form: chiff::HermesForm::Execution,
        }
    );
    assert_eq!(analysis.stats.op_count, 4);
    assert_eq!(analysis.stats.inserted_bytes, 9);
    assert_eq!(apply_patch(&old, &analysis.patch).unwrap(), new);
}

#[test]
fn synthetic_invalid_header_pair_falls_back_to_generic_binary() {
    let old = fs::read(fixture_path(
        "fixtures/synthetic/hermes-fallback/pairs/invalid-header/v1/hermes/index.android.hbc",
    ))
    .unwrap();
    let new = fs::read(fixture_path(
        "fixtures/synthetic/hermes-fallback/pairs/invalid-header/v2/hermes/index.android.hbc",
    ))
    .unwrap();

    let analysis = analyze_diff(&old, &new);

    assert_eq!(analysis.engine_decision.kind, EngineKind::GenericBinary);
    assert_eq!(
        analysis.engine_decision.reason,
        EngineReason::HermesOldInvalidHeader
    );
    assert_eq!(
        analysis.old_structured_hermes_support,
        StructuredHermesSupport::InvalidHeader
    );
    assert_eq!(
        analysis.new_structured_hermes_support,
        StructuredHermesSupport::InvalidHeader
    );
    assert_eq!(analysis.stats.op_count, 3);
    assert_eq!(analysis.stats.inserted_bytes, 8);
    assert_eq!(apply_patch(&old, &analysis.patch).unwrap(), new);
}

#[test]
fn synthetic_arbitrary_binary_pair_uses_generic_binary_engine() {
    let old = fs::read(fixture_path(
        "fixtures/synthetic/generic-binary/pairs/arbitrary-binary/v1/binary/blob.bin",
    ))
    .unwrap();
    let new = fs::read(fixture_path(
        "fixtures/synthetic/generic-binary/pairs/arbitrary-binary/v2/binary/blob.bin",
    ))
    .unwrap();

    let analysis = analyze_diff(&old, &new);

    assert_eq!(analysis.engine_decision.kind, EngineKind::GenericBinary);
    assert_eq!(analysis.engine_decision.reason, EngineReason::BinaryPair);
    assert_eq!(
        analysis.old_structured_hermes_support,
        StructuredHermesSupport::NotHermes
    );
    assert_eq!(
        analysis.new_structured_hermes_support,
        StructuredHermesSupport::NotHermes
    );
    assert_eq!(analysis.stats.op_count, 2);
    assert_eq!(analysis.stats.copied_bytes, 2);
    assert_eq!(analysis.stats.inserted_bytes, 8);
    assert_eq!(apply_patch(&old, &analysis.patch).unwrap(), new);
}
