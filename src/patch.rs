use crate::{parse_artifact, select_engine, EngineKind, HermesSection, HermesSectionKind};

const HERMES_HEADER_LEN: usize = 128;
const HERMES_SECTION_ORDER: [HermesSectionKind; 15] = [
    HermesSectionKind::FunctionHeaders,
    HermesSectionKind::StringKinds,
    HermesSectionKind::IdentifierHashes,
    HermesSectionKind::SmallStringTable,
    HermesSectionKind::OverflowStringTable,
    HermesSectionKind::StringStorage,
    HermesSectionKind::LiteralValueBuffer,
    HermesSectionKind::ObjectKeyBuffer,
    HermesSectionKind::ObjectShapeTable,
    HermesSectionKind::BigIntTable,
    HermesSectionKind::BigIntStorage,
    HermesSectionKind::RegExpTable,
    HermesSectionKind::RegExpStorage,
    HermesSectionKind::CjsModuleTable,
    HermesSectionKind::FunctionSourceTable,
];

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
        EngineKind::Hermes => {
            diff_hermes_bytes(old, new).unwrap_or_else(|| diff_generic_bytes(old, new))
        }
        EngineKind::Text | EngineKind::GenericBinary => diff_generic_bytes(old, new),
    }
}

fn diff_generic_bytes(old: &[u8], new: &[u8]) -> Patch {
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

fn diff_hermes_bytes(old: &[u8], new: &[u8]) -> Option<Patch> {
    if old == new {
        return Some(diff_generic_bytes(old, new));
    }

    let old_artifact = parse_artifact(old)?;
    let new_artifact = parse_artifact(new)?;

    let mut ops = Vec::new();

    append_region_diff(
        &mut ops,
        old,
        new,
        0..HERMES_HEADER_LEN,
        0..HERMES_HEADER_LEN,
    );

    let old_structured = section_ranges_with_padding(
        &old_artifact.section_layout.sections,
        old_artifact.section_layout.structured_end_offset as usize,
    );
    let new_structured = section_ranges_with_padding(
        &new_artifact.section_layout.sections,
        new_artifact.section_layout.structured_end_offset as usize,
    );

    for kind in HERMES_SECTION_ORDER {
        let old_range = find_section_range(kind, &old_structured).unwrap_or(0..0);
        let new_range = find_section_range(kind, &new_structured).unwrap_or(0..0);

        append_region_diff(&mut ops, old, new, old_range, new_range);
    }

    if let (Some(old_functions), Some(new_functions)) = (
        old_artifact.function_layout.as_ref(),
        new_artifact.function_layout.as_ref(),
    ) {
        append_region_diff(
            &mut ops,
            old,
            new,
            old_artifact.section_layout.structured_end_offset as usize
                ..old_functions.bytecode_region_start as usize,
            new_artifact.section_layout.structured_end_offset as usize
                ..new_functions.bytecode_region_start as usize,
        );

        let function_count = old_functions
            .functions
            .len()
            .max(new_functions.functions.len());

        for function_index in 0..function_count {
            let old_range = old_functions
                .functions
                .get(function_index)
                .map(|function| {
                    function.bytecode_offset as usize..function.body_end_offset as usize
                })
                .unwrap_or(0..0);
            let new_range = new_functions
                .functions
                .get(function_index)
                .map(|function| {
                    function.bytecode_offset as usize..function.body_end_offset as usize
                })
                .unwrap_or(0..0);

            append_region_diff(&mut ops, old, new, old_range, new_range);
        }

        if old_functions.info_blocks.is_empty() && new_functions.info_blocks.is_empty() {
            append_region_diff(
                &mut ops,
                old,
                new,
                old_functions.bytecode_region_end as usize
                    ..old_artifact.header.debug_info_offset as usize,
                new_functions.bytecode_region_end as usize
                    ..new_artifact.header.debug_info_offset as usize,
            );
        } else {
            let info_block_count = old_functions
                .info_blocks
                .len()
                .max(new_functions.info_blocks.len());

            for info_block_index in 0..info_block_count {
                let old_range = old_functions
                    .info_blocks
                    .get(info_block_index)
                    .map(|block| block.offset as usize..block.end_offset as usize)
                    .unwrap_or(0..0);
                let new_range = new_functions
                    .info_blocks
                    .get(info_block_index)
                    .map(|block| block.offset as usize..block.end_offset as usize)
                    .unwrap_or(0..0);

                append_region_diff(&mut ops, old, new, old_range, new_range);
            }
        }
    } else {
        append_region_diff(
            &mut ops,
            old,
            new,
            old_artifact.section_layout.structured_end_offset as usize
                ..old_artifact.header.debug_info_offset as usize,
            new_artifact.section_layout.structured_end_offset as usize
                ..new_artifact.header.debug_info_offset as usize,
        );
    }

    append_region_diff(
        &mut ops,
        old,
        new,
        old_artifact.header.debug_info_offset as usize..old_artifact.header.file_length as usize,
        new_artifact.header.debug_info_offset as usize..new_artifact.header.file_length as usize,
    );

    if ops.is_empty() {
        ops.push(PatchOp::Insert(new.to_vec()));
    }

    Some(Patch { ops })
}

fn diff_by_prefix_suffix(old: &[u8], new: &[u8]) -> Patch {
    let mut ops = Vec::new();
    append_prefix_suffix_diff(&mut ops, old, new, 0);
    Patch { ops }
}

fn append_region_diff(
    ops: &mut Vec<PatchOp>,
    old: &[u8],
    new: &[u8],
    old_range: std::ops::Range<usize>,
    new_range: std::ops::Range<usize>,
) {
    if old_range.is_empty() && new_range.is_empty() {
        return;
    }

    append_prefix_suffix_diff(
        ops,
        &old[old_range.clone()],
        &new[new_range],
        old_range.start,
    );
}

fn append_prefix_suffix_diff(ops: &mut Vec<PatchOp>, old: &[u8], new: &[u8], old_base: usize) {
    let prefix_len = common_prefix_len(old, new);
    let suffix_len = common_suffix_len(old, new, prefix_len);

    let old_mid_end = old.len() - suffix_len;
    let new_mid_end = new.len() - suffix_len;
    let mut emitted = false;

    if prefix_len > 0 {
        push_op(
            ops,
            PatchOp::Copy {
                offset: old_base,
                len: prefix_len,
            },
        );
        emitted = true;
    }

    if prefix_len < new_mid_end {
        push_op(ops, PatchOp::Insert(new[prefix_len..new_mid_end].to_vec()));
        emitted = true;
    }

    if suffix_len > 0 {
        push_op(
            ops,
            PatchOp::Copy {
                offset: old_base + old_mid_end,
                len: suffix_len,
            },
        );
        emitted = true;
    }

    if !emitted && !new.is_empty() {
        push_op(ops, PatchOp::Insert(new.to_vec()));
    }
}

fn push_op(ops: &mut Vec<PatchOp>, op: PatchOp) {
    match op {
        PatchOp::Copy { offset: _, len } if len == 0 => {}
        PatchOp::Insert(bytes) if bytes.is_empty() => {}
        PatchOp::Copy { offset, len } => match ops.last_mut() {
            Some(PatchOp::Copy {
                offset: previous_offset,
                len: previous_len,
            }) if previous_offset.saturating_add(*previous_len) == offset => {
                *previous_len += len;
            }
            _ => ops.push(PatchOp::Copy { offset, len }),
        },
        PatchOp::Insert(bytes) => match ops.last_mut() {
            Some(PatchOp::Insert(previous_bytes)) => previous_bytes.extend_from_slice(&bytes),
            _ => ops.push(PatchOp::Insert(bytes)),
        },
    }
}

fn section_ranges_with_padding(
    sections: &[HermesSection],
    structured_end_offset: usize,
) -> Vec<(HermesSectionKind, std::ops::Range<usize>)> {
    sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let end = sections
                .get(index + 1)
                .map(|next| next.offset as usize)
                .unwrap_or(structured_end_offset);

            (section.kind, section.offset as usize..end)
        })
        .collect()
}

fn find_section_range(
    kind: HermesSectionKind,
    sections: &[(HermesSectionKind, std::ops::Range<usize>)],
) -> Option<std::ops::Range<usize>> {
    sections
        .iter()
        .find(|(candidate, _)| *candidate == kind)
        .map(|(_, range)| range.clone())
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
