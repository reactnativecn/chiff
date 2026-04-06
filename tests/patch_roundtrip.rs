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
