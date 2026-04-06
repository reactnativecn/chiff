use chiff::engine::{select_engine, EngineKind};
use chiff::format::{detect_input_format, HermesForm, InputFormat};
use chiff::hermes::{
    parse_artifact, parse_header, parse_section_layout, HermesArtifact, HermesHeader,
    HermesSection, HermesSectionKind, HermesSectionLayout,
};

const HERMES_MAGIC: u64 = 0x1F19_03C1_03BC_1FC6;
const HERMES_DELTA_MAGIC: u64 = !HERMES_MAGIC;

fn hermes_bytes(magic: u64, version: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&magic.to_le_bytes());
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.resize(64, 0);
    bytes
}

#[derive(Debug, Clone, Copy)]
struct HeaderSpec {
    magic: u64,
    version: u32,
    file_length: u32,
    global_code_index: u32,
    function_count: u32,
    string_kind_count: u32,
    identifier_count: u32,
    string_count: u32,
    overflow_string_count: u32,
    string_storage_size: u32,
    big_int_count: u32,
    big_int_storage_size: u32,
    reg_exp_count: u32,
    reg_exp_storage_size: u32,
    literal_value_buffer_size: u32,
    obj_key_buffer_size: u32,
    obj_shape_table_count: u32,
    num_string_switch_imms: u32,
    segment_id: u32,
    cjs_module_count: u32,
    function_source_count: u32,
    debug_info_offset: u32,
    options_flags: u8,
}

impl Default for HeaderSpec {
    fn default() -> Self {
        Self {
            magic: HERMES_MAGIC,
            version: 99,
            file_length: 128,
            global_code_index: 0,
            function_count: 0,
            string_kind_count: 0,
            identifier_count: 0,
            string_count: 0,
            overflow_string_count: 0,
            string_storage_size: 0,
            big_int_count: 0,
            big_int_storage_size: 0,
            reg_exp_count: 0,
            reg_exp_storage_size: 0,
            literal_value_buffer_size: 0,
            obj_key_buffer_size: 0,
            obj_shape_table_count: 0,
            num_string_switch_imms: 0,
            segment_id: 0,
            cjs_module_count: 0,
            function_source_count: 0,
            debug_info_offset: 128,
            options_flags: 0,
        }
    }
}

fn hermes_header_bytes(spec: HeaderSpec) -> Vec<u8> {
    let mut bytes = vec![0; spec.file_length.max(128) as usize];
    bytes[0..8].copy_from_slice(&spec.magic.to_le_bytes());
    bytes[8..12].copy_from_slice(&spec.version.to_le_bytes());
    bytes[32..36].copy_from_slice(&spec.file_length.to_le_bytes());
    bytes[36..40].copy_from_slice(&spec.global_code_index.to_le_bytes());
    bytes[40..44].copy_from_slice(&spec.function_count.to_le_bytes());
    bytes[44..48].copy_from_slice(&spec.string_kind_count.to_le_bytes());
    bytes[48..52].copy_from_slice(&spec.identifier_count.to_le_bytes());
    bytes[52..56].copy_from_slice(&spec.string_count.to_le_bytes());
    bytes[56..60].copy_from_slice(&spec.overflow_string_count.to_le_bytes());
    bytes[60..64].copy_from_slice(&spec.string_storage_size.to_le_bytes());
    bytes[64..68].copy_from_slice(&spec.big_int_count.to_le_bytes());
    bytes[68..72].copy_from_slice(&spec.big_int_storage_size.to_le_bytes());
    bytes[72..76].copy_from_slice(&spec.reg_exp_count.to_le_bytes());
    bytes[76..80].copy_from_slice(&spec.reg_exp_storage_size.to_le_bytes());
    bytes[80..84].copy_from_slice(&spec.literal_value_buffer_size.to_le_bytes());
    bytes[84..88].copy_from_slice(&spec.obj_key_buffer_size.to_le_bytes());
    bytes[88..92].copy_from_slice(&spec.obj_shape_table_count.to_le_bytes());
    bytes[92..96].copy_from_slice(&spec.num_string_switch_imms.to_le_bytes());
    bytes[96..100].copy_from_slice(&spec.segment_id.to_le_bytes());
    bytes[100..104].copy_from_slice(&spec.cjs_module_count.to_le_bytes());
    bytes[104..108].copy_from_slice(&spec.function_source_count.to_le_bytes());
    bytes[108..112].copy_from_slice(&spec.debug_info_offset.to_le_bytes());
    bytes[112] = spec.options_flags;
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
    let bytes = hermes_header_bytes(HeaderSpec {
        file_length: 2048,
        function_count: 7,
        string_count: 19,
        debug_info_offset: 1536,
        ..HeaderSpec::default()
    });

    assert_eq!(
        parse_artifact(&bytes),
        Some(HermesArtifact {
            header: HermesHeader {
                version: 99,
                form: HermesForm::Execution,
                file_length: 2048,
                global_code_index: 0,
                function_count: 7,
                string_kind_count: 0,
                identifier_count: 0,
                string_count: 19,
                overflow_string_count: 0,
                string_storage_size: 0,
                big_int_count: 0,
                big_int_storage_size: 0,
                reg_exp_count: 0,
                reg_exp_storage_size: 0,
                literal_value_buffer_size: 0,
                obj_key_buffer_size: 0,
                obj_shape_table_count: 0,
                num_string_switch_imms: 0,
                segment_id: 0,
                cjs_module_count: 0,
                function_source_count: 0,
                debug_info_offset: 1536,
                options_flags: 0,
            },
            payload_len: bytes.len(),
            section_layout: HermesSectionLayout {
                sections: vec![
                    HermesSection {
                        kind: HermesSectionKind::FunctionHeaders,
                        offset: 128,
                        len: 84,
                    },
                    HermesSection {
                        kind: HermesSectionKind::SmallStringTable,
                        offset: 212,
                        len: 76,
                    },
                ],
                structured_end_offset: 288,
            },
        })
    );
}

#[test]
fn parses_hermes_header_metadata() {
    let bytes = hermes_header_bytes(HeaderSpec {
        magic: HERMES_DELTA_MAGIC,
        file_length: 4096,
        global_code_index: 3,
        function_count: 12,
        string_kind_count: 2,
        identifier_count: 4,
        string_count: 33,
        overflow_string_count: 5,
        string_storage_size: 128,
        big_int_count: 6,
        big_int_storage_size: 256,
        reg_exp_count: 7,
        reg_exp_storage_size: 512,
        literal_value_buffer_size: 100,
        obj_key_buffer_size: 88,
        obj_shape_table_count: 9,
        num_string_switch_imms: 10,
        segment_id: 11,
        cjs_module_count: 12,
        function_source_count: 13,
        debug_info_offset: 3000,
        options_flags: 0b0000_0010,
        ..HeaderSpec::default()
    });

    assert_eq!(
        parse_header(&bytes),
        Some(HermesHeader {
            version: 99,
            form: HermesForm::Delta,
            file_length: 4096,
            global_code_index: 3,
            function_count: 12,
            string_kind_count: 2,
            identifier_count: 4,
            string_count: 33,
            overflow_string_count: 5,
            string_storage_size: 128,
            big_int_count: 6,
            big_int_storage_size: 256,
            reg_exp_count: 7,
            reg_exp_storage_size: 512,
            literal_value_buffer_size: 100,
            obj_key_buffer_size: 88,
            obj_shape_table_count: 9,
            num_string_switch_imms: 10,
            segment_id: 11,
            cjs_module_count: 12,
            function_source_count: 13,
            debug_info_offset: 3000,
            options_flags: 0b0000_0010,
        })
    );
}

#[test]
fn rejects_short_hermes_header() {
    let bytes = hermes_bytes(HERMES_MAGIC, 99);

    assert_eq!(parse_header(&bytes), None);
    assert_eq!(parse_artifact(&bytes), None);
}

#[test]
fn parses_hermes_section_layout_with_alignment() {
    let bytes = hermes_header_bytes(HeaderSpec {
        file_length: 512,
        function_count: 2,
        string_kind_count: 1,
        identifier_count: 3,
        string_count: 4,
        overflow_string_count: 1,
        string_storage_size: 3,
        big_int_count: 1,
        big_int_storage_size: 7,
        reg_exp_count: 2,
        reg_exp_storage_size: 1,
        literal_value_buffer_size: 5,
        obj_key_buffer_size: 2,
        obj_shape_table_count: 2,
        cjs_module_count: 1,
        function_source_count: 2,
        debug_info_offset: 320,
        ..HeaderSpec::default()
    });

    assert_eq!(
        parse_section_layout(&bytes),
        Some(HermesSectionLayout {
            sections: vec![
                HermesSection {
                    kind: HermesSectionKind::FunctionHeaders,
                    offset: 128,
                    len: 24,
                },
                HermesSection {
                    kind: HermesSectionKind::StringKinds,
                    offset: 152,
                    len: 4,
                },
                HermesSection {
                    kind: HermesSectionKind::IdentifierHashes,
                    offset: 156,
                    len: 12,
                },
                HermesSection {
                    kind: HermesSectionKind::SmallStringTable,
                    offset: 168,
                    len: 16,
                },
                HermesSection {
                    kind: HermesSectionKind::OverflowStringTable,
                    offset: 184,
                    len: 8,
                },
                HermesSection {
                    kind: HermesSectionKind::StringStorage,
                    offset: 192,
                    len: 3,
                },
                HermesSection {
                    kind: HermesSectionKind::LiteralValueBuffer,
                    offset: 196,
                    len: 5,
                },
                HermesSection {
                    kind: HermesSectionKind::ObjectKeyBuffer,
                    offset: 204,
                    len: 2,
                },
                HermesSection {
                    kind: HermesSectionKind::ObjectShapeTable,
                    offset: 208,
                    len: 16,
                },
                HermesSection {
                    kind: HermesSectionKind::BigIntTable,
                    offset: 224,
                    len: 8,
                },
                HermesSection {
                    kind: HermesSectionKind::BigIntStorage,
                    offset: 232,
                    len: 7,
                },
                HermesSection {
                    kind: HermesSectionKind::RegExpTable,
                    offset: 240,
                    len: 16,
                },
                HermesSection {
                    kind: HermesSectionKind::RegExpStorage,
                    offset: 256,
                    len: 1,
                },
                HermesSection {
                    kind: HermesSectionKind::CjsModuleTable,
                    offset: 260,
                    len: 8,
                },
                HermesSection {
                    kind: HermesSectionKind::FunctionSourceTable,
                    offset: 268,
                    len: 16,
                },
            ],
            structured_end_offset: 284,
        })
    );
}

#[test]
fn rejects_hermes_artifact_when_debug_info_overlaps_structured_sections() {
    let bytes = hermes_header_bytes(HeaderSpec {
        file_length: 512,
        function_count: 2,
        string_count: 4,
        debug_info_offset: 140,
        ..HeaderSpec::default()
    });

    assert_eq!(parse_section_layout(&bytes), None);
    assert_eq!(parse_artifact(&bytes), None);
}
