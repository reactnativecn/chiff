use chiff::{apply_patch, diff_bytes, Patch, PatchError, PatchOp, PatchStats};

const HERMES_MAGIC: u64 = 0x1F19_03C1_03BC_1FC6;

fn hermes_bytes(version: u32, payload_byte: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.resize(64, payload_byte);
    bytes
}

fn hermes_sectioned_bytes(
    version: u32,
    string_kind_count: u32,
    string_count: u32,
    string_kind_fill: &[u8],
    identifier_hashes: &[u8],
    string_table_fill: &[u8],
) -> Vec<u8> {
    let header_len = 128usize;
    let function_headers_len = 12usize;
    let string_kinds_len = string_kind_count as usize * 4;
    let identifier_hashes_len = identifier_hashes.len();
    let string_table_len = string_count as usize * 4;
    let structured_end = header_len
        + function_headers_len
        + string_kinds_len
        + identifier_hashes_len
        + string_table_len;
    let debug_info_offset = structured_end + 16;
    let file_length = debug_info_offset + 8;

    let mut bytes = vec![0; file_length];
    bytes[0..8].copy_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes[8..12].copy_from_slice(&version.to_le_bytes());
    bytes[32..36].copy_from_slice(&(file_length as u32).to_le_bytes());
    bytes[40..44].copy_from_slice(&1u32.to_le_bytes());
    bytes[44..48].copy_from_slice(&string_kind_count.to_le_bytes());
    bytes[48..52].copy_from_slice(&((identifier_hashes_len / 4) as u32).to_le_bytes());
    bytes[52..56].copy_from_slice(&string_count.to_le_bytes());
    bytes[108..112].copy_from_slice(&(debug_info_offset as u32).to_le_bytes());

    let function_headers_offset = header_len;
    let string_kinds_offset = function_headers_offset + function_headers_len;
    let identifier_hashes_offset = string_kinds_offset + string_kinds_len;
    let string_table_offset = identifier_hashes_offset + identifier_hashes_len;

    bytes[function_headers_offset..string_kinds_offset].fill(0xA1);
    bytes[string_kinds_offset..identifier_hashes_offset].copy_from_slice(string_kind_fill);
    bytes[identifier_hashes_offset..string_table_offset].copy_from_slice(identifier_hashes);
    bytes[string_table_offset..structured_end].copy_from_slice(string_table_fill);
    bytes[structured_end..debug_info_offset].fill(0xCC);
    bytes[debug_info_offset..file_length].fill(0xDD);

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

    let mut bytes = vec![0; file_length];
    bytes[0..8].copy_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&(file_length as u32).to_le_bytes());
    bytes[40..44].copy_from_slice(&(function_bodies.len() as u32).to_le_bytes());
    bytes[108..112].copy_from_slice(&(debug_info_offset as u32).to_le_bytes());

    let mut body_offset = bytecode_start as u32;
    for (index, body) in function_bodies.iter().enumerate() {
        let header = small_function_header(body_offset, body.len() as u32);
        let header_offset = header_len + index * 12;
        bytes[header_offset..header_offset + 12].copy_from_slice(&header);
        bytes[body_offset as usize..body_offset as usize + body.len()].copy_from_slice(body);
        body_offset += body.len() as u32;
    }

    bytes[debug_info_offset..file_length].fill(0xFE);
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

    let mut bytes = vec![0; file_length];
    bytes[0..8].copy_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&(file_length as u32).to_le_bytes());
    bytes[40..44].copy_from_slice(&(function_bodies.len() as u32).to_le_bytes());
    bytes[108..112].copy_from_slice(&(debug_info_offset as u32).to_le_bytes());

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

    bytes[debug_info_offset..file_length].fill(0xFB);
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

    let mut bytes = vec![0; file_length];
    bytes[0..8].copy_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&(file_length as u32).to_le_bytes());
    bytes[40..44].copy_from_slice(&(function_bodies.len() as u32).to_le_bytes());
    bytes[108..112].copy_from_slice(&(debug_info_offset as u32).to_le_bytes());

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

    bytes[debug_info_offset..file_length].fill(0xFD);
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

    let mut bytes = vec![0; file_length];
    bytes[0..8].copy_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&(file_length as u32).to_le_bytes());
    bytes[40..44].copy_from_slice(&(header_body_indices.len() as u32).to_le_bytes());
    bytes[108..112].copy_from_slice(&(debug_info_offset as u32).to_le_bytes());

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

    bytes[debug_info_offset..file_length].fill(0xFE);
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

    let mut bytes = vec![0; file_length];
    bytes[0..8].copy_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&(file_length as u32).to_le_bytes());
    bytes[40..44].copy_from_slice(&(function_bodies.len() as u32).to_le_bytes());
    bytes[108..112].copy_from_slice(&(debug_info_offset as u32).to_le_bytes());

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

    bytes[debug_info_offset..file_length].fill(0xFC);
    bytes
}

fn append_signed_leb128(output: &mut Vec<u8>, mut value: i64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;

        let done = (value == 0 && (byte & 0x40) == 0) || (value == -1 && (byte & 0x40) != 0);
        if done {
            output.push(byte);
            break;
        }

        output.push(byte | 0x80);
    }
}

fn hermes_bytes_with_debug_info_filename(filename_storage: &[u8]) -> Vec<u8> {
    const DEBUG_INFO_OFFSET: usize = 128;
    const DEBUG_INFO_HEADER_LEN: usize = 16;
    const FILENAME_TABLE_LEN: usize = 8;
    const FILE_REGION_LEN: usize = 12;
    const FOOTER_LEN: usize = 20;

    let mut debug_data = Vec::new();
    append_signed_leb128(&mut debug_data, 7);
    append_signed_leb128(&mut debug_data, 10);
    append_signed_leb128(&mut debug_data, 3);
    append_signed_leb128(&mut debug_data, 0);
    append_signed_leb128(&mut debug_data, 0);
    append_signed_leb128(&mut debug_data, 1);
    append_signed_leb128(&mut debug_data, 0);
    append_signed_leb128(&mut debug_data, -1);

    let file_length = DEBUG_INFO_OFFSET
        + DEBUG_INFO_HEADER_LEN
        + FILENAME_TABLE_LEN
        + filename_storage.len()
        + FILE_REGION_LEN
        + debug_data.len()
        + FOOTER_LEN;
    let mut bytes = hermes_bytes(99, 0x00);
    bytes.resize(file_length, 0);
    bytes[32..36].copy_from_slice(&(file_length as u32).to_le_bytes());
    bytes[108..112].copy_from_slice(&(DEBUG_INFO_OFFSET as u32).to_le_bytes());

    let mut cursor = DEBUG_INFO_OFFSET;
    bytes[cursor..cursor + 4].copy_from_slice(&1u32.to_le_bytes());
    bytes[cursor + 4..cursor + 8].copy_from_slice(&(filename_storage.len() as u32).to_le_bytes());
    bytes[cursor + 8..cursor + 12].copy_from_slice(&1u32.to_le_bytes());
    bytes[cursor + 12..cursor + 16].copy_from_slice(&(debug_data.len() as u32).to_le_bytes());
    cursor += DEBUG_INFO_HEADER_LEN;

    bytes[cursor..cursor + 4].copy_from_slice(&0u32.to_le_bytes());
    bytes[cursor + 4..cursor + 8].copy_from_slice(&(filename_storage.len() as u32).to_le_bytes());
    cursor += FILENAME_TABLE_LEN;

    bytes[cursor..cursor + filename_storage.len()].copy_from_slice(filename_storage);
    cursor += filename_storage.len();

    bytes[cursor..cursor + 4].copy_from_slice(&0u32.to_le_bytes());
    bytes[cursor + 4..cursor + 8].copy_from_slice(&0u32.to_le_bytes());
    bytes[cursor + 8..cursor + 12].copy_from_slice(&0u32.to_le_bytes());
    cursor += FILE_REGION_LEN;

    bytes[cursor..cursor + debug_data.len()].copy_from_slice(&debug_data);
    bytes[file_length - FOOTER_LEN..file_length].fill(0xF7);
    bytes
}

#[test]
fn apply_patch_replays_copy_and_insert_ops() {
    let old = b"hello world";
    let patch = Patch {
        ops: vec![
            PatchOp::Copy { offset: 0, len: 6 },
            PatchOp::Insert(b"rust".to_vec()),
        ],
    };

    assert_eq!(apply_patch(old, &patch).unwrap(), b"hello rust");
}

#[test]
fn apply_patch_rejects_out_of_bounds_copy() {
    let old = b"abc";
    let patch = Patch {
        ops: vec![PatchOp::Copy { offset: 1, len: 4 }],
    };

    assert_eq!(
        apply_patch(old, &patch),
        Err(PatchError::InvalidCopyRange {
            offset: 1,
            len: 4,
            old_len: 3,
        })
    );
}

#[test]
fn patch_stats_reports_copy_and_insert_totals() {
    let patch = Patch {
        ops: vec![
            PatchOp::Copy { offset: 2, len: 4 },
            PatchOp::Insert(b"rust".to_vec()),
            PatchOp::Copy { offset: 10, len: 2 },
        ],
    };

    assert_eq!(
        patch.stats(),
        PatchStats {
            op_count: 3,
            copy_op_count: 2,
            insert_op_count: 1,
            copied_bytes: 6,
            inserted_bytes: 4,
        }
    );
}

#[test]
fn diff_bytes_roundtrips_utf8_text() {
    let old = b"const answer = 41;\n";
    let new = b"const answer = 42;\n";

    let patch = diff_bytes(old, new);

    assert_eq!(apply_patch(old, &patch).unwrap(), new);
}

#[test]
fn diff_bytes_roundtrips_generic_binary() {
    let old = [0, 1, 2, 3, 4, 5];
    let new = [9, 8, 7, 6];

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
}

#[test]
fn diff_bytes_roundtrips_hermes_bytecode() {
    let old = hermes_bytes(99, 0x11);
    let mut new = hermes_bytes(99, 0x11);
    new[20] = 0x22;
    new[21] = 0x33;

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
}

#[test]
fn diff_bytes_roundtrips_truncated_supported_hermes_via_generic_fallback() {
    let old = hermes_bytes(99, 0x11);
    let mut new = hermes_bytes(99, 0x11);
    new[24] = 0x44;
    new[25] = 0x55;

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
}

#[test]
fn diff_bytes_roundtrips_unsupported_same_version_hermes_via_generic_fallback() {
    let old = hermes_bytes(100, 0x11);
    let mut new = hermes_bytes(100, 0x11);
    new[20] = 0x22;
    new[21] = 0x33;

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
}

#[test]
fn diff_bytes_preserves_unchanged_hermes_section_between_shifted_changes() {
    let old = hermes_sectioned_bytes(
        99,
        1,
        1,
        &[0x10, 0x11, 0x12, 0x13],
        &[0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37],
        &[0x50, 0x51, 0x52, 0x53],
    );
    let new = hermes_sectioned_bytes(
        99,
        2,
        2,
        &[0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97],
        &[0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37],
        &[0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7],
    );

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
    assert!(patch.ops.iter().any(|op| {
        matches!(
            op,
            PatchOp::Copy { offset, len } if *offset == 144 && *len == 8
        )
    }));
}

#[test]
fn diff_bytes_preserves_unchanged_hermes_function_after_offset_shift() {
    let old = hermes_small_function_bytes(&[b"\x01\x02\x03", b"\xAA\xBB\xCC\xDD"]);
    let new = hermes_small_function_bytes(&[b"\x10\x11\x12\x13\x14", b"\xAA\xBB\xCC\xDD"]);

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
    assert!(patch.ops.iter().any(|op| {
        matches!(
            op,
            PatchOp::Copy { offset, len } if *offset == 155 && *len >= 4
        )
    }));
}

#[test]
fn diff_bytes_preserves_unchanged_overflowed_hermes_function_after_offset_shift() {
    let old = hermes_overflow_function_bytes(&[b"\x01\x02\x03", b"\xAA\xBB\xCC\xDD"]);
    let new = hermes_overflow_function_bytes(&[b"\x10\x11\x12\x13\x14", b"\xAA\xBB\xCC\xDD"]);

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
    assert!(patch.ops.iter().any(|op| {
        matches!(
            op,
            PatchOp::Copy { offset, len } if *offset == 155 && *len >= 4
        )
    }));
}

#[test]
fn diff_bytes_roundtrips_overflowed_hermes_with_shared_bytecode_offsets() {
    let old = hermes_overflow_function_bytes_with_header_map(
        &[b"\x01\x02\x03", b"\xAA\xBB\xCC\xDD"],
        &[1, 0, 1],
    );
    let new = hermes_overflow_function_bytes_with_header_map(
        &[b"\x10\x11\x12\x13\x14", b"\xAA\xBB\xCC\xDD"],
        &[1, 0, 1],
    );

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
    assert!(patch
        .ops
        .iter()
        .any(|op| { matches!(op, PatchOp::Copy { len, .. } if *len >= 4) }));
}

#[test]
fn diff_bytes_preserves_unchanged_overflowed_info_block_between_changed_neighbors() {
    let bodies = [
        b"\x01\x02".as_slice(),
        b"\x11\x12\x13".as_slice(),
        b"\x21".as_slice(),
    ];
    let old = hermes_overflow_function_bytes_with_debug(
        &bodies,
        &[None, Some(0x2222_2222), Some(0x3333_3333)],
    );
    let new = hermes_overflow_function_bytes_with_debug(
        &bodies,
        &[Some(0x1111_1111), Some(0x2222_2222), Some(0x4444_4444)],
    );

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
    assert!(patch.ops.iter().any(|op| {
        matches!(
            op,
            PatchOp::Copy { offset, len } if *offset == 208 && *len >= 40
        )
    }));
}

#[test]
fn diff_bytes_preserves_unchanged_exception_table_inside_changed_overflowed_info_block() {
    let old = hermes_overflow_function_bytes_with_info(
        &[b"\x01\x02", b"\x11\x12\x13"],
        &[None, Some(1)],
        &[None, Some(0xAAAA_AAAA)],
    );
    let new = hermes_overflow_function_bytes_with_info(
        &[b"\x01\x02\x03\x04", b"\x11\x12\x13"],
        &[None, Some(1)],
        &[None, Some(0xBBBB_BBBB)],
    );

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
    assert!(patch.ops.iter().any(|op| {
        matches!(
            op,
            PatchOp::Copy { offset, len }
                if *offset <= 232 && offset.saturating_add(*len) >= 248
        )
    }));
}

#[test]
fn diff_bytes_preserves_unchanged_exception_table_inside_changed_small_info_block() {
    let old = hermes_small_function_bytes_with_info(
        &[b"\x01\x02", b"\x11\x12\x13"],
        &[None, Some(1)],
        &[None, Some(0xAAAA_AAAA)],
    );
    let new = hermes_small_function_bytes_with_info(
        &[b"\x01\x02\x03\x04", b"\x11\x12\x13"],
        &[None, Some(1)],
        &[None, Some(0xBBBB_BBBB)],
    );

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
    assert!(patch.ops.iter().any(|op| {
        matches!(
            op,
            PatchOp::Copy { offset, len }
                if *offset <= 160 && offset.saturating_add(*len) >= 176
        )
    }));
}

#[test]
fn diff_bytes_preserves_unchanged_hermes_debug_data_when_only_filename_storage_changes() {
    let old = hermes_bytes_with_debug_info_filename(b"app\0");
    let new = hermes_bytes_with_debug_info_filename(b"application\0");

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
    assert!(patch.ops.iter().any(|op| {
        matches!(
            op,
            PatchOp::Copy { offset, len }
                if *offset <= 168 && offset.saturating_add(*len) >= 176
        )
    }));
}

#[test]
fn diff_bytes_preserves_common_prefix_and_suffix() {
    let old = b"abcXYZdef";
    let new = b"abc123def";

    let patch = diff_bytes(old, new);

    assert_eq!(
        patch,
        Patch {
            ops: vec![
                PatchOp::Copy { offset: 0, len: 3 },
                PatchOp::Insert(b"123".to_vec()),
                PatchOp::Copy { offset: 6, len: 3 },
            ],
        }
    );
}

#[test]
fn diff_bytes_preserves_common_prefix_for_append_only_change() {
    let old = b"hello";
    let new = b"hello world";

    let patch = diff_bytes(old, new);

    assert_eq!(
        patch,
        Patch {
            ops: vec![
                PatchOp::Copy { offset: 0, len: 5 },
                PatchOp::Insert(b" world".to_vec()),
            ],
        }
    );
}

#[test]
fn diff_bytes_preserves_common_middle_anchor_for_text() {
    let old = b"aaMIDDLEzz";
    let new = b"xxMIDDLEyy";

    let patch = diff_bytes(old, new);

    assert_eq!(apply_patch(old, &patch).unwrap(), new);
    assert!(patch.ops.iter().any(|op| {
        matches!(
            op,
            PatchOp::Copy { offset, len } if *offset <= 2 && offset.saturating_add(*len) >= 8
        )
    }));
}

#[test]
fn diff_bytes_preserves_common_middle_anchor_within_hermes_function_body() {
    let old = hermes_small_function_bytes(&[b"\x01\x02\x10\x11\x12\x13\x03\x04"]);
    let new = hermes_small_function_bytes(&[b"\xAA\xBB\x10\x11\x12\x13\xCC\xDD"]);

    let patch = diff_bytes(&old, &new);

    assert_eq!(apply_patch(&old, &patch).unwrap(), new);
    assert!(patch.ops.iter().any(|op| {
        matches!(
            op,
            PatchOp::Copy { offset, len }
                if *offset <= 142 && offset.saturating_add(*len) >= 146
        )
    }));
}

#[test]
fn diff_bytes_preserves_common_line_anchor_for_text() {
    let old = b"alpha\nKEEP\nomega\n";
    let new = b"beta\nKEEP\ngamma\n";

    let patch = diff_bytes(old, new);

    assert_eq!(apply_patch(old, &patch).unwrap(), new);
    assert!(patch.ops.iter().any(|op| {
        matches!(
            op,
            PatchOp::Copy { offset, len } if *offset <= 6 && offset.saturating_add(*len) >= 11
        )
    }));
}
