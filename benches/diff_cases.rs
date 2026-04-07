use chiff::{apply_patch, diff_bytes};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

const HERMES_MAGIC: u64 = 0x1F19_03C1_03BC_1FC6;

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

fn large_function_header_with_info(
    bytecode_offset: u32,
    bytecode_size: u32,
    has_exception_handlers: bool,
    has_debug_offsets: bool,
) -> [u8; 36] {
    let mut bytes = [0u8; 36];
    bytes[0..4].copy_from_slice(&bytecode_offset.to_le_bytes());
    bytes[12..16].copy_from_slice(&bytecode_size.to_le_bytes());
    if has_exception_handlers {
        bytes[35] |= 1 << 3;
    }
    if has_debug_offsets {
        bytes[35] |= 1 << 4;
    }
    bytes
}

fn align4(value: usize) -> usize {
    (value + 3) & !3
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

    bytes[debug_info_offset..file_length].fill(0xEE);
    bytes
}

fn hermes_small_uint_switch_function_bytes(instruction: [u8; 18], jump_table: &[u32]) -> Vec<u8> {
    let header_len = 128usize;
    let function_headers_len = 12usize;
    let bytecode_start = header_len + function_headers_len;
    let bytecode_size = instruction.len();
    let jump_table_offset = align4(bytecode_start + bytecode_size);
    let jump_table_size = jump_table.len() * 4;
    let debug_info_offset = jump_table_offset + jump_table_size;
    let file_length = debug_info_offset + 8;

    let mut bytes = vec![0; file_length];
    bytes[0..8].copy_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&(file_length as u32).to_le_bytes());
    bytes[40..44].copy_from_slice(&1u32.to_le_bytes());
    bytes[108..112].copy_from_slice(&(debug_info_offset as u32).to_le_bytes());

    let header = small_function_header(bytecode_start as u32, bytecode_size as u32);
    bytes[header_len..header_len + 12].copy_from_slice(&header);
    bytes[bytecode_start..bytecode_start + bytecode_size].copy_from_slice(&instruction);

    for (index, entry) in jump_table.iter().enumerate() {
        let offset = jump_table_offset + index * 4;
        bytes[offset..offset + 4].copy_from_slice(&entry.to_le_bytes());
    }

    bytes[debug_info_offset..file_length].fill(0xED);
    bytes
}

fn hermes_small_string_switch_function_bytes(
    instruction: [u8; 18],
    jump_table: &[(u32, u32)],
) -> Vec<u8> {
    let header_len = 128usize;
    let function_headers_len = 12usize;
    let bytecode_start = header_len + function_headers_len;
    let bytecode_size = instruction.len();
    let jump_table_offset = align4(bytecode_start + bytecode_size);
    let jump_table_size = jump_table.len() * 8;
    let debug_info_offset = jump_table_offset + jump_table_size;
    let file_length = debug_info_offset + 8;

    let mut bytes = vec![0; file_length];
    bytes[0..8].copy_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&(file_length as u32).to_le_bytes());
    bytes[40..44].copy_from_slice(&1u32.to_le_bytes());
    bytes[108..112].copy_from_slice(&(debug_info_offset as u32).to_le_bytes());

    let header = small_function_header(bytecode_start as u32, bytecode_size as u32);
    bytes[header_len..header_len + 12].copy_from_slice(&header);
    bytes[bytecode_start..bytecode_start + bytecode_size].copy_from_slice(&instruction);

    for (index, (string_id, jump_target)) in jump_table.iter().enumerate() {
        let offset = jump_table_offset + index * 8;
        bytes[offset..offset + 4].copy_from_slice(&string_id.to_le_bytes());
        bytes[offset + 4..offset + 8].copy_from_slice(&jump_target.to_le_bytes());
    }

    bytes[debug_info_offset..file_length].fill(0xEA);
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

    bytes[debug_info_offset..file_length].fill(0xEB);
    bytes
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

    bytes[debug_info_offset..file_length].fill(0xEC);
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

fn signed_leb128_bytes(values: &[i64]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in values {
        append_signed_leb128(&mut bytes, *value);
    }
    bytes
}

fn hermes_bytes_with_debug_info(filename_storage: &[u8], debug_data: &[u8]) -> Vec<u8> {
    const DEBUG_INFO_OFFSET: usize = 128;
    const DEBUG_INFO_HEADER_LEN: usize = 16;
    const FILENAME_TABLE_LEN: usize = 8;
    const FILE_REGION_LEN: usize = 12;
    const FOOTER_LEN: usize = 20;

    let file_length = DEBUG_INFO_OFFSET
        + DEBUG_INFO_HEADER_LEN
        + FILENAME_TABLE_LEN
        + filename_storage.len()
        + FILE_REGION_LEN
        + debug_data.len()
        + FOOTER_LEN;
    let mut bytes = vec![0; file_length];
    bytes[0..8].copy_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
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

    bytes[cursor..cursor + debug_data.len()].copy_from_slice(debug_data);
    bytes[file_length - FOOTER_LEN..file_length].fill(0xE9);
    bytes
}

fn bench_diff_text(c: &mut Criterion) {
    let old = "const value = 41;\n".repeat(512);
    let mut new = old.clone();
    new.replace_range(4096..4113, "const value = 42;");

    c.bench_function("diff/text-small-change", |b| {
        b.iter(|| diff_bytes(black_box(old.as_bytes()), black_box(new.as_bytes())))
    });

    let old_middle = b"aaMIDDLEzz".repeat(256);
    let new_middle = b"xxMIDDLEyy".repeat(256);
    c.bench_function("diff/text-middle-anchor", |b| {
        b.iter(|| diff_bytes(black_box(&old_middle), black_box(&new_middle)))
    });
}

fn bench_diff_hermes_small(c: &mut Criterion) {
    let old = hermes_small_function_bytes(&[
        &[0x01, 0x02, 0x03, 0x04],
        &[0x11, 0x12, 0x13, 0x14, 0x15],
        &[0x21, 0x22, 0x23],
    ]);
    let new = hermes_small_function_bytes(&[
        &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        &[0x11, 0x12, 0x13, 0x14, 0x15],
        &[0x21, 0x22, 0x23],
    ]);

    c.bench_function("diff/hermes-small-function-shift", |b| {
        b.iter(|| diff_bytes(black_box(&old), black_box(&new)))
    });

    let old_anchor = hermes_small_function_bytes(&[b"\x01\x02\x10\x11\x12\x13\x03\x04"]);
    let new_anchor = hermes_small_function_bytes(&[b"\xAA\xBB\x10\x11\x12\x13\xCC\xDD"]);
    c.bench_function("diff/hermes-small-middle-anchor", |b| {
        b.iter(|| diff_bytes(black_box(&old_anchor), black_box(&new_anchor)))
    });

    let old_instruction = hermes_small_function_bytes(&[&[5, 0x10, 5, 0x20, 5, 0x30]]);
    let new_instruction = hermes_small_function_bytes(&[&[5, 0x11, 5, 0x20, 5, 0x31]]);
    c.bench_function("diff/hermes-small-instruction-split", |b| {
        b.iter(|| diff_bytes(black_box(&old_instruction), black_box(&new_instruction)))
    });

    let mut old_switch = [0_u8; 18];
    old_switch[0] = 167;
    old_switch[1] = 1;
    old_switch[2..6].copy_from_slice(&18_u32.to_le_bytes());
    old_switch[6..10].copy_from_slice(&4_i32.to_le_bytes());
    old_switch[10..14].copy_from_slice(&7_u32.to_le_bytes());
    old_switch[14..18].copy_from_slice(&7_u32.to_le_bytes());
    let mut new_switch = old_switch;
    new_switch[1] = 2;
    new_switch[6..10].copy_from_slice(&8_i32.to_le_bytes());
    let old_switch = hermes_small_uint_switch_function_bytes(old_switch, &[0x1234_5678]);
    let new_switch = hermes_small_uint_switch_function_bytes(new_switch, &[0x1234_5678]);
    c.bench_function("diff/hermes-small-switch-tail", |b| {
        b.iter(|| diff_bytes(black_box(&old_switch), black_box(&new_switch)))
    });

    let mut old_string_switch = [0_u8; 18];
    old_string_switch[0] = 168;
    old_string_switch[1] = 1;
    old_string_switch[2..6].copy_from_slice(&7_u32.to_le_bytes());
    old_string_switch[6..10].copy_from_slice(&18_u32.to_le_bytes());
    old_string_switch[10..14].copy_from_slice(&4_i32.to_le_bytes());
    old_string_switch[14..18].copy_from_slice(&1_u32.to_le_bytes());
    let mut new_string_switch = old_string_switch;
    new_string_switch[1] = 2;
    new_string_switch[10..14].copy_from_slice(&8_i32.to_le_bytes());
    let old_string_switch =
        hermes_small_string_switch_function_bytes(old_string_switch, &[(11, 0x1234_5678)]);
    let new_string_switch =
        hermes_small_string_switch_function_bytes(new_string_switch, &[(11, 0x1234_5678)]);
    c.bench_function("diff/hermes-small-string-switch-tail", |b| {
        b.iter(|| diff_bytes(black_box(&old_string_switch), black_box(&new_string_switch)))
    });
}

fn bench_diff_hermes_overflow_info(c: &mut Criterion) {
    let old = hermes_overflow_function_bytes_with_info(
        &[&[0x01, 0x02], &[0x11, 0x12, 0x13]],
        &[None, Some(1)],
        &[None, Some(0xAAAA_AAAA)],
    );
    let new = hermes_overflow_function_bytes_with_info(
        &[&[0x01, 0x02, 0x03, 0x04], &[0x11, 0x12, 0x13]],
        &[None, Some(1)],
        &[None, Some(0xBBBB_BBBB)],
    );
    let patch = diff_bytes(&old, &new);

    c.bench_function("diff/hermes-overflow-info-subregion", |b| {
        b.iter(|| diff_bytes(black_box(&old), black_box(&new)))
    });
    c.bench_function("apply/hermes-overflow-info-subregion", |b| {
        b.iter(|| apply_patch(black_box(&old), black_box(&patch)).unwrap())
    });
}

fn bench_diff_hermes_small_info(c: &mut Criterion) {
    let old = hermes_small_function_bytes_with_info(
        &[&[0x01, 0x02], &[0x11, 0x12, 0x13]],
        &[None, Some(1)],
        &[None, Some(0xAAAA_AAAA)],
    );
    let new = hermes_small_function_bytes_with_info(
        &[&[0x01, 0x02, 0x03, 0x04], &[0x11, 0x12, 0x13]],
        &[None, Some(1)],
        &[None, Some(0xBBBB_BBBB)],
    );
    let patch = diff_bytes(&old, &new);

    c.bench_function("diff/hermes-small-info-subregion", |b| {
        b.iter(|| diff_bytes(black_box(&old), black_box(&new)))
    });
    c.bench_function("apply/hermes-small-info-subregion", |b| {
        b.iter(|| apply_patch(black_box(&old), black_box(&patch)).unwrap())
    });
}

fn bench_diff_hermes_debug_stream(c: &mut Criterion) {
    let old =
        hermes_bytes_with_debug_info(b"app\0", &signed_leb128_bytes(&[7, 10, 3, 0, 0, 1, 5, -1]));
    let new =
        hermes_bytes_with_debug_info(b"app\0", &signed_leb128_bytes(&[7, 130, 3, 0, 0, 1, 5, -1]));

    c.bench_function("diff/hermes-debug-stream-varint-shift", |b| {
        b.iter(|| diff_bytes(black_box(&old), black_box(&new)))
    });

    let old_records = hermes_bytes_with_debug_info(
        b"app\0",
        &signed_leb128_bytes(&[7, 10, 3, 0, 0, 1, 0, 1, 1, 0, 2, 1, 0, -1]),
    );
    let new_records = hermes_bytes_with_debug_info(
        b"app\0",
        &signed_leb128_bytes(&[7, 10, 3, 0, 9, 1, 0, 8, 1, 0, 1, 1, 0, 7, 1, 0, -1]),
    );
    c.bench_function("diff/hermes-debug-stream-record-anchor", |b| {
        b.iter(|| diff_bytes(black_box(&old_records), black_box(&new_records)))
    });

    let old_field = hermes_bytes_with_debug_info(
        b"app\0",
        &signed_leb128_bytes(&[7, 10, 3, 0, 0, 5, 1, 9, 1, 1, 0, -1]),
    );
    let new_field = hermes_bytes_with_debug_info(
        b"app\0",
        &signed_leb128_bytes(&[7, 10, 3, 0, 0, 7, 1, 4, 9, 2, 1, 0, -1]),
    );
    c.bench_function("diff/hermes-debug-stream-record-field", |b| {
        b.iter(|| diff_bytes(black_box(&old_field), black_box(&new_field)))
    });
}

fn benchmark_diff_cases(c: &mut Criterion) {
    bench_diff_text(c);
    bench_diff_hermes_small(c);
    bench_diff_hermes_overflow_info(c);
    bench_diff_hermes_small_info(c);
    bench_diff_hermes_debug_stream(c);
}

criterion_group!(benches, benchmark_diff_cases);
criterion_main!(benches);
