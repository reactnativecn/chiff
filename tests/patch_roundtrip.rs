use chiff::{apply_patch, diff_bytes, Patch, PatchError, PatchOp};

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

fn large_function_header_with_debug(
    bytecode_offset: u32,
    bytecode_size: u32,
    has_debug_offsets: bool,
) -> [u8; 36] {
    let mut bytes = large_function_header(bytecode_offset, bytecode_size);
    if has_debug_offsets {
        bytes[35] = 1 << 4;
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

fn hermes_overflow_function_bytes_with_debug(
    function_bodies: &[&[u8]],
    debug_offsets: &[Option<u32>],
) -> Vec<u8> {
    assert_eq!(function_bodies.len(), debug_offsets.len());

    let header_len = 128usize;
    let function_headers_len = function_bodies.len() * 12;
    let bytecode_start = header_len + function_headers_len;
    let bytecode_end =
        bytecode_start + function_bodies.iter().map(|body| body.len()).sum::<usize>();
    let info_start = align4(bytecode_end);

    let mut info_offset = info_start;
    let mut large_header_offsets = Vec::with_capacity(function_bodies.len());
    for debug_offset in debug_offsets {
        large_header_offsets.push(info_offset);
        info_offset = align4(info_offset + 36 + usize::from(debug_offset.is_some()) * 4);
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

        let large_header = large_function_header_with_debug(
            body_offset,
            body.len() as u32,
            debug_offsets[index].is_some(),
        );
        let large_header_offset = large_header_offsets[index];
        bytes[large_header_offset..large_header_offset + 36].copy_from_slice(&large_header);

        if let Some(debug_offset) = debug_offsets[index] {
            bytes[large_header_offset + 36..large_header_offset + 40]
                .copy_from_slice(&debug_offset.to_le_bytes());
        }

        body_offset += body.len() as u32;
    }

    bytes[debug_info_offset..file_length].fill(0xFC);
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
