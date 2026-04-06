use chiff::engine::{select_engine, EngineKind};
use chiff::format::{detect_input_format, HermesForm, InputFormat};
use chiff::hermes::{parse_artifact, parse_header, HermesArtifact, HermesHeader};

const HERMES_MAGIC: u64 = 0x1F19_03C1_03BC_1FC6;
const HERMES_DELTA_MAGIC: u64 = !HERMES_MAGIC;

fn hermes_bytes(magic: u64, version: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&magic.to_le_bytes());
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.resize(64, 0);
    bytes
}

fn hermes_header_bytes(
    magic: u64,
    version: u32,
    file_length: u32,
    function_count: u32,
    string_count: u32,
    debug_info_offset: u32,
) -> Vec<u8> {
    let mut bytes = vec![0; 128];
    bytes[0..8].copy_from_slice(&magic.to_le_bytes());
    bytes[8..12].copy_from_slice(&version.to_le_bytes());
    bytes[32..36].copy_from_slice(&file_length.to_le_bytes());
    bytes[40..44].copy_from_slice(&function_count.to_le_bytes());
    bytes[52..56].copy_from_slice(&string_count.to_le_bytes());
    bytes[108..112].copy_from_slice(&debug_info_offset.to_le_bytes());
    bytes
}

#[test]
fn detects_utf8_text_bundle() {
    let bundle = br#"function greet(name) { return `hi ${name}`; }"#;

    assert_eq!(detect_input_format(bundle), InputFormat::TextUtf8);
}

#[test]
fn detects_hermes_execution_bytecode() {
    let bytes = hermes_bytes(HERMES_MAGIC, 99);

    assert_eq!(
        detect_input_format(&bytes),
        InputFormat::HermesBytecode {
            version: 99,
            form: HermesForm::Execution,
        }
    );
}

#[test]
fn detects_hermes_delta_bytecode() {
    let bytes = hermes_bytes(HERMES_DELTA_MAGIC, 99);

    assert_eq!(
        detect_input_format(&bytes),
        InputFormat::HermesBytecode {
            version: 99,
            form: HermesForm::Delta,
        }
    );
}

#[test]
fn selects_text_engine_for_two_text_inputs() {
    let old = b"const a = 1;\n";
    let new = b"const a = 2;\n";

    assert_eq!(select_engine(old, new), EngineKind::Text);
}

#[test]
fn selects_hermes_engine_for_same_version_hermes_inputs() {
    let old = hermes_bytes(HERMES_MAGIC, 99);
    let new = hermes_bytes(HERMES_MAGIC, 99);

    assert_eq!(select_engine(&old, &new), EngineKind::Hermes);
}

#[test]
fn falls_back_to_generic_binary_when_hermes_versions_differ() {
    let old = hermes_bytes(HERMES_MAGIC, 98);
    let new = hermes_bytes(HERMES_MAGIC, 99);

    assert_eq!(select_engine(&old, &new), EngineKind::GenericBinary);
}

#[test]
fn falls_back_to_generic_binary_for_mixed_formats() {
    let old = b"export const enabled = true;\n";
    let new = hermes_bytes(HERMES_MAGIC, 99);

    assert_eq!(select_engine(old, &new), EngineKind::GenericBinary);
}

#[test]
fn parses_hermes_artifact_without_footer() {
    let bytes = hermes_header_bytes(HERMES_MAGIC, 99, 2048, 7, 19, 1536);

    assert_eq!(
        parse_artifact(&bytes),
        Some(HermesArtifact {
            header: HermesHeader {
                version: 99,
                form: HermesForm::Execution,
                file_length: 2048,
                function_count: 7,
                string_count: 19,
                debug_info_offset: 1536,
            },
            payload_len: bytes.len(),
        })
    );
}

#[test]
fn parses_hermes_header_metadata() {
    let bytes = hermes_header_bytes(HERMES_DELTA_MAGIC, 99, 4096, 12, 33, 3000);

    assert_eq!(
        parse_header(&bytes),
        Some(HermesHeader {
            version: 99,
            form: HermesForm::Delta,
            file_length: 4096,
            function_count: 12,
            string_count: 33,
            debug_info_offset: 3000,
        })
    );
}

#[test]
fn rejects_short_hermes_header() {
    let bytes = hermes_bytes(HERMES_MAGIC, 99);

    assert_eq!(parse_header(&bytes), None);
    assert_eq!(parse_artifact(&bytes), None);
}
