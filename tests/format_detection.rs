use chiff::engine::{select_engine, EngineKind};
use chiff::format::{detect_input_format, HermesForm, InputFormat};
use chiff::hermes::{
    parse_artifact, parse_function_layout, parse_header, parse_section_layout, HermesArtifact,
    HermesFunction, HermesFunctionInfoBlock, HermesFunctionLayout, HermesHeader, HermesSection,
    HermesSectionKind, HermesSectionLayout,
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

fn small_function_header(offset: u32, bytecode_size: u32) -> [u8; 12] {
    let mut bytes = [0u8; 12];
    let w1 = offset & ((1 << 25) - 1);
    let w2 = bytecode_size & ((1 << 14) - 1);
    bytes[0..4].copy_from_slice(&w1.to_le_bytes());
    bytes[4..8].copy_from_slice(&w2.to_le_bytes());
    bytes
}

fn small_function_header_with_info(
    offset: u32,
    bytecode_size: u32,
    info_offset: u32,
    has_exception_handlers: bool,
    has_debug_offsets: bool,
) -> [u8; 12] {
    let mut bytes = small_function_header(offset, bytecode_size);
    let word3 = info_offset & 0x00FF_FFFF;
    bytes[8..12].copy_from_slice(&word3.to_le_bytes());
    if has_exception_handlers {
        bytes[11] |= 1 << 3;
    }
    if has_debug_offsets {
        bytes[11] |= 1 << 4;
    }
    bytes
}

fn overflow_function_header(large_header_offset: u32) -> [u8; 12] {
    let mut bytes = [0u8; 12];
    let low = large_header_offset & 0x00FF_FFFF;
    let high = (large_header_offset >> 24) & 0xFF;
    let w1 = low;
    let w2 = high << 14;
    bytes[0..4].copy_from_slice(&w1.to_le_bytes());
    bytes[4..8].copy_from_slice(&w2.to_le_bytes());
    bytes[11] = 1 << 5;
    bytes
}

fn large_function_header(bytecode_offset: u32, bytecode_size: u32) -> [u8; 36] {
    let mut bytes = [0u8; 36];
    bytes[0..4].copy_from_slice(&bytecode_offset.to_le_bytes());
    bytes[12..16].copy_from_slice(&bytecode_size.to_le_bytes());
    bytes
}

fn large_function_header_with_info(
    bytecode_offset: u32,
    bytecode_size: u32,
    has_exception_handlers: bool,
    has_debug_offsets: bool,
) -> [u8; 36] {
    let mut bytes = large_function_header(bytecode_offset, bytecode_size);
    if has_exception_handlers {
        bytes[35] |= 1 << 3;
    }
    if has_debug_offsets {
        bytes[35] |= 1 << 4;
    }
    bytes
}

fn hermes_small_function_bytes(function_bodies: &[&[u8]]) -> Vec<u8> {
    let header_len = 128usize;
    let function_headers_len = function_bodies.len() * 12;
    let bytecode_start = header_len + function_headers_len;
    let debug_info_offset =
        bytecode_start + function_bodies.iter().map(|body| body.len()).sum::<usize>();
    let file_length = debug_info_offset + 8;

    let mut bytes = hermes_header_bytes(HeaderSpec {
        file_length: file_length as u32,
        function_count: function_bodies.len() as u32,
        debug_info_offset: debug_info_offset as u32,
        ..HeaderSpec::default()
    });

    let mut body_offset = bytecode_start as u32;
    for (index, body) in function_bodies.iter().enumerate() {
        let header = small_function_header(body_offset, body.len() as u32);
        let header_offset = header_len + index * 12;
        bytes[header_offset..header_offset + 12].copy_from_slice(&header);
        bytes[body_offset as usize..body_offset as usize + body.len()].copy_from_slice(body);
        body_offset += body.len() as u32;
    }

    bytes[debug_info_offset..file_length].fill(0xEE);
    bytes
}

fn hermes_small_function_bytes_with_info(
    function_bodies: &[&[u8]],
    exception_handler_counts: &[Option<u32>],
    debug_offsets: &[Option<u32>],
) -> Vec<u8> {
    assert_eq!(function_bodies.len(), exception_handler_counts.len());
    assert_eq!(function_bodies.len(), debug_offsets.len());

    let header_len = 128usize;
    let function_headers_len = function_bodies.len() * 12;
    let bytecode_start = header_len + function_headers_len;
    let bytecode_end =
        bytecode_start + function_bodies.iter().map(|body| body.len()).sum::<usize>();
    let info_start = align4(bytecode_end);

    let mut info_offset = info_start;
    let mut info_offsets = vec![None; function_bodies.len()];
    for (index, (exception_count, debug_offset)) in exception_handler_counts
        .iter()
        .zip(debug_offsets)
        .enumerate()
    {
        if exception_count.is_some() || debug_offset.is_some() {
            info_offsets[index] = Some(info_offset);
            let mut block_end = info_offset;
            if let Some(exception_count) = exception_count {
                block_end = align4(block_end);
                block_end += 4 + *exception_count as usize * 12;
            }
            if debug_offset.is_some() {
                block_end = align4(block_end);
                block_end += 4;
            }
            info_offset = align4(block_end);
        }
    }

    let debug_info_offset = info_offset;
    let file_length = debug_info_offset + 8;

    let mut bytes = hermes_header_bytes(HeaderSpec {
        file_length: file_length as u32,
        function_count: function_bodies.len() as u32,
        debug_info_offset: debug_info_offset as u32,
        ..HeaderSpec::default()
    });

    let mut body_offset = bytecode_start as u32;
    for (index, body) in function_bodies.iter().enumerate() {
        let header = match info_offsets[index] {
            Some(info_offset) => small_function_header_with_info(
                body_offset,
                body.len() as u32,
                info_offset as u32,
                exception_handler_counts[index].is_some(),
                debug_offsets[index].is_some(),
            ),
            None => small_function_header(body_offset, body.len() as u32),
        };
        let header_offset = header_len + index * 12;
        bytes[header_offset..header_offset + 12].copy_from_slice(&header);
        bytes[body_offset as usize..body_offset as usize + body.len()].copy_from_slice(body);
        body_offset += body.len() as u32;
    }

    for (index, info_offset) in info_offsets.into_iter().enumerate() {
        let Some(mut cursor) = info_offset else {
            continue;
        };
        if let Some(exception_count) = exception_handler_counts[index] {
            cursor = align4(cursor);
            bytes[cursor..cursor + 4].copy_from_slice(&exception_count.to_le_bytes());
            let mut entry_cursor = cursor + 4;
            for entry_index in 0..exception_count as usize {
                let entry = [
                    (0xC0 + index as u8),
                    entry_index as u8,
                    0xA0,
                    0xA1,
                    0xA2,
                    0xA3,
                    0xA4,
                    0xA5,
                    0xA6,
                    0xA7,
                    0xA8,
                    0xA9,
                ];
                bytes[entry_cursor..entry_cursor + 12].copy_from_slice(&entry);
                entry_cursor += 12;
            }
            cursor = entry_cursor;
        }
        if let Some(debug_offset) = debug_offsets[index] {
            cursor = align4(cursor);
            bytes[cursor..cursor + 4].copy_from_slice(&debug_offset.to_le_bytes());
        }
    }

    bytes[debug_info_offset..file_length].fill(0xEB);
    bytes
}

fn align4(value: usize) -> usize {
    (value + 3) & !3
}

fn hermes_overflow_function_bytes(function_bodies: &[&[u8]]) -> Vec<u8> {
    let header_len = 128usize;
    let function_headers_len = function_bodies.len() * 12;
    let bytecode_start = header_len + function_headers_len;
    let bytecode_end =
        bytecode_start + function_bodies.iter().map(|body| body.len()).sum::<usize>();
    let info_start = align4(bytecode_end);

    let mut info_offset = info_start;
    let mut large_header_offsets = Vec::with_capacity(function_bodies.len());
    for _ in function_bodies {
        large_header_offsets.push(info_offset);
        info_offset = align4(info_offset + 36);
    }

    let debug_info_offset = info_offset;
    let file_length = debug_info_offset + 8;

    let mut bytes = hermes_header_bytes(HeaderSpec {
        file_length: file_length as u32,
        function_count: function_bodies.len() as u32,
        debug_info_offset: debug_info_offset as u32,
        ..HeaderSpec::default()
    });

    let mut body_offset = bytecode_start as u32;
    for (index, body) in function_bodies.iter().enumerate() {
        let header = overflow_function_header(large_header_offsets[index] as u32);
        let header_offset = header_len + index * 12;
        bytes[header_offset..header_offset + 12].copy_from_slice(&header);
        bytes[body_offset as usize..body_offset as usize + body.len()].copy_from_slice(body);

        let large_header = large_function_header(body_offset, body.len() as u32);
        let large_header_offset = large_header_offsets[index];
        bytes[large_header_offset..large_header_offset + 36].copy_from_slice(&large_header);

        body_offset += body.len() as u32;
    }

    bytes[debug_info_offset..file_length].fill(0xED);
    bytes
}

fn hermes_overflow_function_bytes_with_header_map(
    function_bodies: &[&[u8]],
    header_body_indices: &[usize],
) -> Vec<u8> {
    assert_eq!(function_bodies.len(), 2);
    assert_eq!(header_body_indices.len(), 3);

    let header_len = 128usize;
    let function_headers_len = header_body_indices.len() * 12;
    let bytecode_start = header_len + function_headers_len;
    let bytecode_end =
        bytecode_start + function_bodies.iter().map(|body| body.len()).sum::<usize>();
    let info_start = align4(bytecode_end);

    let mut info_offset = info_start;
    let mut large_header_offsets = Vec::with_capacity(header_body_indices.len());
    for _ in header_body_indices {
        large_header_offsets.push(info_offset);
        info_offset = align4(info_offset + 36);
    }

    let debug_info_offset = info_offset;
    let file_length = debug_info_offset + 8;

    let mut bytes = hermes_header_bytes(HeaderSpec {
        file_length: file_length as u32,
        function_count: header_body_indices.len() as u32,
        debug_info_offset: debug_info_offset as u32,
        ..HeaderSpec::default()
    });

    let first_body_offset = bytecode_start as u32;
    let second_body_offset = first_body_offset + function_bodies[0].len() as u32;
    let body_offsets = [first_body_offset, second_body_offset];

    bytes[first_body_offset as usize..first_body_offset as usize + function_bodies[0].len()]
        .copy_from_slice(function_bodies[0]);
    bytes[second_body_offset as usize..second_body_offset as usize + function_bodies[1].len()]
        .copy_from_slice(function_bodies[1]);

    for (index, body_index) in header_body_indices.iter().copied().enumerate() {
        let header = overflow_function_header(large_header_offsets[index] as u32);
        let header_offset = header_len + index * 12;
        bytes[header_offset..header_offset + 12].copy_from_slice(&header);

        let large_header = large_function_header(
            body_offsets[body_index],
            function_bodies[body_index].len() as u32,
        );
        let large_header_offset = large_header_offsets[index];
        bytes[large_header_offset..large_header_offset + 36].copy_from_slice(&large_header);
    }

    bytes[debug_info_offset..file_length].fill(0xEE);
    bytes
}

fn hermes_overflow_function_bytes_with_debug(
    function_bodies: &[&[u8]],
    debug_offsets: &[Option<u32>],
) -> Vec<u8> {
    hermes_overflow_function_bytes_with_info(
        function_bodies,
        &vec![None; function_bodies.len()],
        debug_offsets,
    )
}

fn hermes_overflow_function_bytes_with_info(
    function_bodies: &[&[u8]],
    exception_handler_counts: &[Option<u32>],
    debug_offsets: &[Option<u32>],
) -> Vec<u8> {
    assert_eq!(function_bodies.len(), exception_handler_counts.len());
    assert_eq!(function_bodies.len(), debug_offsets.len());

    let header_len = 128usize;
    let function_headers_len = function_bodies.len() * 12;
    let bytecode_start = header_len + function_headers_len;
    let bytecode_end =
        bytecode_start + function_bodies.iter().map(|body| body.len()).sum::<usize>();
    let info_start = align4(bytecode_end);

    let mut info_offset = info_start;
    let mut large_header_offsets = Vec::with_capacity(function_bodies.len());
    for (exception_count, debug_offset) in exception_handler_counts.iter().zip(debug_offsets) {
        large_header_offsets.push(info_offset);
        let mut block_end = info_offset + 36;
        if let Some(exception_count) = exception_count {
            block_end = align4(block_end);
            block_end += 4 + *exception_count as usize * 12;
        }
        if debug_offset.is_some() {
            block_end = align4(block_end);
            block_end += 4;
        }
        info_offset = align4(block_end);
    }

    let debug_info_offset = info_offset;
    let file_length = debug_info_offset + 8;

    let mut bytes = hermes_header_bytes(HeaderSpec {
        file_length: file_length as u32,
        function_count: function_bodies.len() as u32,
        debug_info_offset: debug_info_offset as u32,
        ..HeaderSpec::default()
    });

    let mut body_offset = bytecode_start as u32;
    for (index, body) in function_bodies.iter().enumerate() {
        let header = overflow_function_header(large_header_offsets[index] as u32);
        let header_offset = header_len + index * 12;
        bytes[header_offset..header_offset + 12].copy_from_slice(&header);
        bytes[body_offset as usize..body_offset as usize + body.len()].copy_from_slice(body);

        let large_header = large_function_header_with_info(
            body_offset,
            body.len() as u32,
            exception_handler_counts[index].is_some(),
            debug_offsets[index].is_some(),
        );
        let large_header_offset = large_header_offsets[index];
        bytes[large_header_offset..large_header_offset + 36].copy_from_slice(&large_header);

        let mut info_cursor = large_header_offset + 36;
        if let Some(exception_count) = exception_handler_counts[index] {
            info_cursor = align4(info_cursor);
            bytes[info_cursor..info_cursor + 4].copy_from_slice(&exception_count.to_le_bytes());
            let mut entry_cursor = info_cursor + 4;
            for entry_index in 0..exception_count as usize {
                let entry = [
                    (0xE0 + index as u8),
                    entry_index as u8,
                    0xA0,
                    0xA1,
                    0xA2,
                    0xA3,
                    0xA4,
                    0xA5,
                    0xA6,
                    0xA7,
                    0xA8,
                    0xA9,
                ];
                bytes[entry_cursor..entry_cursor + 12].copy_from_slice(&entry);
                entry_cursor += 12;
            }
            info_cursor = entry_cursor;
        }

        if let Some(debug_offset) = debug_offsets[index] {
            info_cursor = align4(info_cursor);
            bytes[info_cursor..info_cursor + 4].copy_from_slice(&debug_offset.to_le_bytes());
        }

        body_offset += body.len() as u32;
    }

    bytes[debug_info_offset..file_length].fill(0xEC);
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
            function_layout: None,
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

#[test]
fn parses_small_function_layout_from_function_headers() {
    let bytes = hermes_small_function_bytes(&[b"\x01\x02\x03", b"\x10\x11\x12\x13"]);

    assert_eq!(
        parse_function_layout(&bytes),
        Some(HermesFunctionLayout {
            functions: vec![
                HermesFunction {
                    index: 0,
                    header_offset: 128,
                    bytecode_offset: 152,
                    bytecode_size: 3,
                    body_end_offset: 155,
                },
                HermesFunction {
                    index: 1,
                    header_offset: 140,
                    bytecode_offset: 155,
                    bytecode_size: 4,
                    body_end_offset: 159,
                },
            ],
            info_blocks: vec![],
            bytecode_region_start: 152,
            bytecode_region_end: 159,
        })
    );
}

#[test]
fn parse_artifact_exposes_small_function_layout() {
    let bytes = hermes_small_function_bytes(&[b"\xAA", b"\xBB\xCC"]);

    let artifact = parse_artifact(&bytes).unwrap();

    assert_eq!(
        artifact.function_layout,
        Some(HermesFunctionLayout {
            functions: vec![
                HermesFunction {
                    index: 0,
                    header_offset: 128,
                    bytecode_offset: 152,
                    bytecode_size: 1,
                    body_end_offset: 153,
                },
                HermesFunction {
                    index: 1,
                    header_offset: 140,
                    bytecode_offset: 153,
                    bytecode_size: 2,
                    body_end_offset: 155,
                },
            ],
            info_blocks: vec![],
            bytecode_region_start: 152,
            bytecode_region_end: 155,
        })
    );
}

#[test]
fn parses_small_function_info_blocks_with_exception_handlers_and_debug_offsets() {
    let bytes = hermes_small_function_bytes_with_info(
        &[b"\x11\x12", b"\x21\x22\x23"],
        &[Some(1), None],
        &[Some(0x0102_0304), Some(0x0506_0708)],
    );

    assert_eq!(
        parse_function_layout(&bytes),
        Some(HermesFunctionLayout {
            functions: vec![
                HermesFunction {
                    index: 0,
                    header_offset: 128,
                    bytecode_offset: 152,
                    bytecode_size: 2,
                    body_end_offset: 154,
                },
                HermesFunction {
                    index: 1,
                    header_offset: 140,
                    bytecode_offset: 154,
                    bytecode_size: 3,
                    body_end_offset: 160,
                },
            ],
            info_blocks: vec![
                HermesFunctionInfoBlock {
                    index: 0,
                    offset: 160,
                    large_header_end_offset: 160,
                    exception_table_offset: Some(160),
                    exception_table_end_offset: Some(176),
                    debug_offsets_offset: Some(176),
                    debug_offsets_end_offset: Some(180),
                    payload_end_offset: 180,
                    end_offset: 180,
                    has_exception_handlers: true,
                    has_debug_offsets: true,
                },
                HermesFunctionInfoBlock {
                    index: 1,
                    offset: 180,
                    large_header_end_offset: 180,
                    exception_table_offset: None,
                    exception_table_end_offset: None,
                    debug_offsets_offset: Some(180),
                    debug_offsets_end_offset: Some(184),
                    payload_end_offset: 184,
                    end_offset: 184,
                    has_exception_handlers: false,
                    has_debug_offsets: true,
                },
            ],
            bytecode_region_start: 152,
            bytecode_region_end: 160,
        })
    );
}

#[test]
fn parses_overflowed_function_layout_from_large_headers() {
    let bytes = hermes_overflow_function_bytes(&[b"\x21\x22\x23", b"\x31\x32\x33\x34"]);

    assert_eq!(
        parse_function_layout(&bytes),
        Some(HermesFunctionLayout {
            functions: vec![
                HermesFunction {
                    index: 0,
                    header_offset: 128,
                    bytecode_offset: 152,
                    bytecode_size: 3,
                    body_end_offset: 155,
                },
                HermesFunction {
                    index: 1,
                    header_offset: 140,
                    bytecode_offset: 155,
                    bytecode_size: 4,
                    body_end_offset: 160,
                },
            ],
            info_blocks: vec![
                HermesFunctionInfoBlock {
                    index: 0,
                    offset: 160,
                    large_header_end_offset: 196,
                    exception_table_offset: None,
                    exception_table_end_offset: None,
                    debug_offsets_offset: None,
                    debug_offsets_end_offset: None,
                    payload_end_offset: 196,
                    end_offset: 196,
                    has_exception_handlers: false,
                    has_debug_offsets: false,
                },
                HermesFunctionInfoBlock {
                    index: 1,
                    offset: 196,
                    large_header_end_offset: 232,
                    exception_table_offset: None,
                    exception_table_end_offset: None,
                    debug_offsets_offset: None,
                    debug_offsets_end_offset: None,
                    payload_end_offset: 232,
                    end_offset: 232,
                    has_exception_handlers: false,
                    has_debug_offsets: false,
                },
            ],
            bytecode_region_start: 152,
            bytecode_region_end: 160,
        })
    );
}

#[test]
fn parses_overflowed_function_layout_with_non_monotonic_duplicate_offsets() {
    let bytes = hermes_overflow_function_bytes_with_header_map(
        &[b"\x21\x22\x23", b"\x31\x32\x33\x34"],
        &[1, 0, 1],
    );

    assert_eq!(
        parse_function_layout(&bytes),
        Some(HermesFunctionLayout {
            functions: vec![
                HermesFunction {
                    index: 0,
                    header_offset: 128,
                    bytecode_offset: 167,
                    bytecode_size: 4,
                    body_end_offset: 172,
                },
                HermesFunction {
                    index: 1,
                    header_offset: 140,
                    bytecode_offset: 164,
                    bytecode_size: 3,
                    body_end_offset: 167,
                },
                HermesFunction {
                    index: 2,
                    header_offset: 152,
                    bytecode_offset: 167,
                    bytecode_size: 4,
                    body_end_offset: 172,
                },
            ],
            info_blocks: vec![
                HermesFunctionInfoBlock {
                    index: 0,
                    offset: 172,
                    large_header_end_offset: 208,
                    exception_table_offset: None,
                    exception_table_end_offset: None,
                    debug_offsets_offset: None,
                    debug_offsets_end_offset: None,
                    payload_end_offset: 208,
                    end_offset: 208,
                    has_exception_handlers: false,
                    has_debug_offsets: false,
                },
                HermesFunctionInfoBlock {
                    index: 1,
                    offset: 208,
                    large_header_end_offset: 244,
                    exception_table_offset: None,
                    exception_table_end_offset: None,
                    debug_offsets_offset: None,
                    debug_offsets_end_offset: None,
                    payload_end_offset: 244,
                    end_offset: 244,
                    has_exception_handlers: false,
                    has_debug_offsets: false,
                },
                HermesFunctionInfoBlock {
                    index: 2,
                    offset: 244,
                    large_header_end_offset: 280,
                    exception_table_offset: None,
                    exception_table_end_offset: None,
                    debug_offsets_offset: None,
                    debug_offsets_end_offset: None,
                    payload_end_offset: 280,
                    end_offset: 280,
                    has_exception_handlers: false,
                    has_debug_offsets: false,
                },
            ],
            bytecode_region_start: 164,
            bytecode_region_end: 172,
        })
    );
}

#[test]
fn parses_overflowed_function_info_blocks_with_debug_offsets() {
    let bytes = hermes_overflow_function_bytes_with_debug(
        &[b"\x41\x42", b"\x51\x52\x53", b"\x61"],
        &[None, Some(0x1122_3344), Some(0x5566_7788)],
    );

    assert_eq!(
        parse_function_layout(&bytes),
        Some(HermesFunctionLayout {
            functions: vec![
                HermesFunction {
                    index: 0,
                    header_offset: 128,
                    bytecode_offset: 164,
                    bytecode_size: 2,
                    body_end_offset: 166,
                },
                HermesFunction {
                    index: 1,
                    header_offset: 140,
                    bytecode_offset: 166,
                    bytecode_size: 3,
                    body_end_offset: 169,
                },
                HermesFunction {
                    index: 2,
                    header_offset: 152,
                    bytecode_offset: 169,
                    bytecode_size: 1,
                    body_end_offset: 172,
                },
            ],
            info_blocks: vec![
                HermesFunctionInfoBlock {
                    index: 0,
                    offset: 172,
                    large_header_end_offset: 208,
                    exception_table_offset: None,
                    exception_table_end_offset: None,
                    debug_offsets_offset: None,
                    debug_offsets_end_offset: None,
                    payload_end_offset: 208,
                    end_offset: 208,
                    has_exception_handlers: false,
                    has_debug_offsets: false,
                },
                HermesFunctionInfoBlock {
                    index: 1,
                    offset: 208,
                    large_header_end_offset: 244,
                    exception_table_offset: None,
                    exception_table_end_offset: None,
                    debug_offsets_offset: Some(244),
                    debug_offsets_end_offset: Some(248),
                    payload_end_offset: 248,
                    end_offset: 248,
                    has_exception_handlers: false,
                    has_debug_offsets: true,
                },
                HermesFunctionInfoBlock {
                    index: 2,
                    offset: 248,
                    large_header_end_offset: 284,
                    exception_table_offset: None,
                    exception_table_end_offset: None,
                    debug_offsets_offset: Some(284),
                    debug_offsets_end_offset: Some(288),
                    payload_end_offset: 288,
                    end_offset: 288,
                    has_exception_handlers: false,
                    has_debug_offsets: true,
                },
            ],
            bytecode_region_start: 164,
            bytecode_region_end: 172,
        })
    );
}

#[test]
fn parses_overflowed_function_info_blocks_with_exception_handlers_and_debug_offsets() {
    let bytes = hermes_overflow_function_bytes_with_info(
        &[b"\x71\x72\x73", b"\x81"],
        &[Some(1), None],
        &[Some(0x0102_0304), Some(0x0506_0708)],
    );

    assert_eq!(
        parse_function_layout(&bytes),
        Some(HermesFunctionLayout {
            functions: vec![
                HermesFunction {
                    index: 0,
                    header_offset: 128,
                    bytecode_offset: 152,
                    bytecode_size: 3,
                    body_end_offset: 155,
                },
                HermesFunction {
                    index: 1,
                    header_offset: 140,
                    bytecode_offset: 155,
                    bytecode_size: 1,
                    body_end_offset: 156,
                },
            ],
            info_blocks: vec![
                HermesFunctionInfoBlock {
                    index: 0,
                    offset: 156,
                    large_header_end_offset: 192,
                    exception_table_offset: Some(192),
                    exception_table_end_offset: Some(208),
                    debug_offsets_offset: Some(208),
                    debug_offsets_end_offset: Some(212),
                    payload_end_offset: 212,
                    end_offset: 212,
                    has_exception_handlers: true,
                    has_debug_offsets: true,
                },
                HermesFunctionInfoBlock {
                    index: 1,
                    offset: 212,
                    large_header_end_offset: 248,
                    exception_table_offset: None,
                    exception_table_end_offset: None,
                    debug_offsets_offset: Some(248),
                    debug_offsets_end_offset: Some(252),
                    payload_end_offset: 252,
                    end_offset: 252,
                    has_exception_handlers: false,
                    has_debug_offsets: true,
                },
            ],
            bytecode_region_start: 152,
            bytecode_region_end: 156,
        })
    );
}
