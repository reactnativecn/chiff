use chiff::{apply_patch, diff_bytes, Patch, PatchError, PatchOp};

const HERMES_MAGIC: u64 = 0x1F19_03C1_03BC_1FC6;

fn hermes_bytes(version: u32, payload_byte: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&HERMES_MAGIC.to_le_bytes());
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.resize(64, payload_byte);
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
