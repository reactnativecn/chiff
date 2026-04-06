use crate::{select_engine, EngineKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchOp {
    Copy { offset: usize, len: usize },
    Insert(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub ops: Vec<PatchOp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    InvalidCopyRange {
        offset: usize,
        len: usize,
        old_len: usize,
    },
}

pub fn apply_patch(old: &[u8], patch: &Patch) -> Result<Vec<u8>, PatchError> {
    let mut output = Vec::new();

    for op in &patch.ops {
        match op {
            PatchOp::Copy { offset, len } => {
                let end = offset.saturating_add(*len);
                let Some(segment) = old.get(*offset..end) else {
                    return Err(PatchError::InvalidCopyRange {
                        offset: *offset,
                        len: *len,
                        old_len: old.len(),
                    });
                };
                output.extend_from_slice(segment);
            }
            PatchOp::Insert(bytes) => output.extend_from_slice(bytes),
        }
    }

    Ok(output)
}

pub fn diff_bytes(old: &[u8], new: &[u8]) -> Patch {
    match select_engine(old, new) {
        EngineKind::Text | EngineKind::Hermes | EngineKind::GenericBinary => {
            if old == new {
                Patch {
                    ops: vec![PatchOp::Copy {
                        offset: 0,
                        len: old.len(),
                    }],
                }
            } else {
                diff_by_prefix_suffix(old, new)
            }
        }
    }
}

fn diff_by_prefix_suffix(old: &[u8], new: &[u8]) -> Patch {
    let prefix_len = common_prefix_len(old, new);
    let suffix_len = common_suffix_len(old, new, prefix_len);

    let old_mid_end = old.len() - suffix_len;
    let new_mid_end = new.len() - suffix_len;

    let mut ops = Vec::new();

    if prefix_len > 0 {
        ops.push(PatchOp::Copy {
            offset: 0,
            len: prefix_len,
        });
    }

    if prefix_len < new_mid_end {
        ops.push(PatchOp::Insert(new[prefix_len..new_mid_end].to_vec()));
    }

    if suffix_len > 0 {
        ops.push(PatchOp::Copy {
            offset: old_mid_end,
            len: suffix_len,
        });
    }

    if ops.is_empty() {
        ops.push(PatchOp::Insert(new.to_vec()));
    }

    Patch { ops }
}

fn common_prefix_len(old: &[u8], new: &[u8]) -> usize {
    old.iter()
        .zip(new.iter())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count()
}

fn common_suffix_len(old: &[u8], new: &[u8], prefix_len: usize) -> usize {
    let old_remaining = old.len().saturating_sub(prefix_len);
    let new_remaining = new.len().saturating_sub(prefix_len);
    let max_suffix_len = old_remaining.min(new_remaining);

    old[old.len().saturating_sub(max_suffix_len)..]
        .iter()
        .rev()
        .zip(new[new.len().saturating_sub(max_suffix_len)..].iter().rev())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count()
}
