use crate::{
    assess_structured_hermes, can_use_structured_hermes, detect_input_format,
    hermes_opcodes::{
        HERMES_V98_V99_OPCODE_SIZES, HERMES_V98_V99_STRING_SWITCH_IMM_OPCODE,
        HERMES_V98_V99_UINT_SWITCH_IMM_OPCODE,
    },
    parse_artifact, select_engine_decision, EngineDecision, EngineKind, HermesDebugDataStream,
    HermesDebugInfoLayout, HermesFunction, HermesFunctionInfoBlock, HermesSection,
    HermesSectionKind, InputFormat, StructuredHermesSupport,
};
use std::collections::{HashMap, VecDeque};

const HERMES_HEADER_LEN: usize = 128;
const RESYNC_ANCHOR_WINDOW: usize = 4;
const RESYNC_MIN_MATCH_LEN: usize = 4;
const RESYNC_MAX_POSITIONS_PER_KEY: usize = 8;
const RESYNC_FULL_SCAN_MAX_REGION_LEN: usize = 256 * 1024;
const RESYNC_MAX_REGION_LEN: usize = 8 * 1024 * 1024;
const RESYNC_TARGET_SAMPLE_COUNT: usize = 16 * 1024;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchStats {
    pub op_count: usize,
    pub copy_op_count: usize,
    pub insert_op_count: usize,
    pub copied_bytes: usize,
    pub inserted_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffAnalysis {
    pub patch: Patch,
    pub stats: PatchStats,
    pub engine_decision: EngineDecision,
    pub old_structured_hermes_support: StructuredHermesSupport,
    pub new_structured_hermes_support: StructuredHermesSupport,
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
    analyze_diff(old, new).patch
}

pub fn analyze_diff(old: &[u8], new: &[u8]) -> DiffAnalysis {
    let engine_decision = select_engine_decision(old, new);
    let old_structured_hermes_support = assess_structured_hermes(old);
    let new_structured_hermes_support = assess_structured_hermes(new);

    let patch = match engine_decision.kind {
        EngineKind::Hermes => {
            diff_hermes_bytes(old, new).unwrap_or_else(|| diff_generic_bytes(old, new))
        }
        EngineKind::Text | EngineKind::GenericBinary => diff_generic_bytes(old, new),
    }
    .normalized();
    let stats = patch.stats();

    DiffAnalysis {
        patch,
        stats,
        engine_decision,
        old_structured_hermes_support,
        new_structured_hermes_support,
    }
}

impl Patch {
    pub fn normalized(self) -> Self {
        let mut normalized_ops = Vec::with_capacity(self.ops.len());
        for op in self.ops {
            push_op(&mut normalized_ops, op);
        }
        Self {
            ops: normalized_ops,
        }
    }

    pub fn stats(&self) -> PatchStats {
        let mut stats = PatchStats {
            op_count: self.ops.len(),
            copy_op_count: 0,
            insert_op_count: 0,
            copied_bytes: 0,
            inserted_bytes: 0,
        };

        for op in &self.ops {
            match op {
                PatchOp::Copy { len, .. } => {
                    stats.copy_op_count += 1;
                    stats.copied_bytes += len;
                }
                PatchOp::Insert(bytes) => {
                    stats.insert_op_count += 1;
                    stats.inserted_bytes += bytes.len();
                }
            }
        }

        stats
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

    match (detect_input_format(old), detect_input_format(new)) {
        (
            InputFormat::HermesBytecode {
                version: old_version,
                form: old_form,
            },
            InputFormat::HermesBytecode {
                version: new_version,
                form: new_form,
            },
        ) if old_version == new_version
            && old_form == new_form
            && can_use_structured_hermes(old)
            && can_use_structured_hermes(new) => {}
        _ => return None,
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

        let old_function_ranges = unique_function_body_ranges(&old_functions.functions);
        let new_function_ranges = unique_function_body_ranges(&new_functions.functions);
        let function_range_count = old_function_ranges.len().max(new_function_ranges.len());

        for function_index in 0..function_range_count {
            let old_function = old_functions.functions.get(function_index);
            let new_function = new_functions.functions.get(function_index);
            let old_range = old_function_ranges
                .get(function_index)
                .cloned()
                .unwrap_or(0..0);
            let new_range = new_function_ranges
                .get(function_index)
                .cloned()
                .unwrap_or(0..0);

            match (
                old_function.and_then(|function| parse_function_body_layout(old, function)),
                new_function.and_then(|function| parse_function_body_layout(new, function)),
            ) {
                (Some(old_body), Some(new_body)) => {
                    let structured_ops = build_function_body_diff(old, new, &old_body, &new_body);
                    let coarse_ops =
                        build_region_diff(old, new, old_range.clone(), new_range.clone());
                    let structured_stats = Patch {
                        ops: structured_ops.clone(),
                    }
                    .stats();
                    let coarse_stats = Patch {
                        ops: coarse_ops.clone(),
                    }
                    .stats();

                    if structured_stats.inserted_bytes < coarse_stats.inserted_bytes
                        || (structured_stats.inserted_bytes == coarse_stats.inserted_bytes
                            && structured_stats.op_count < coarse_stats.op_count)
                    {
                        ops.extend(structured_ops);
                    } else {
                        ops.extend(coarse_ops);
                    }
                }
                _ => append_region_diff(&mut ops, old, new, old_range, new_range),
            }
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
            let old_info_start = old_functions
                .info_blocks
                .first()
                .map(|block| block.offset as usize)
                .unwrap_or(old_artifact.header.debug_info_offset as usize);
            let new_info_start = new_functions
                .info_blocks
                .first()
                .map(|block| block.offset as usize)
                .unwrap_or(new_artifact.header.debug_info_offset as usize);

            append_region_diff(
                &mut ops,
                old,
                new,
                old_functions.bytecode_region_end as usize..old_info_start,
                new_functions.bytecode_region_end as usize..new_info_start,
            );

            let info_block_count = old_functions
                .info_blocks
                .len()
                .max(new_functions.info_blocks.len());

            for info_block_index in 0..info_block_count {
                append_info_block_diff(
                    &mut ops,
                    old,
                    new,
                    old_functions.info_blocks.get(info_block_index),
                    new_functions.info_blocks.get(info_block_index),
                );
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

    if let (Some(old_debug), Some(new_debug)) = (
        old_artifact.debug_info_layout.as_ref(),
        new_artifact.debug_info_layout.as_ref(),
    ) {
        append_debug_info_diff(&mut ops, old, new, old_debug, new_debug);
    } else {
        append_region_diff(
            &mut ops,
            old,
            new,
            old_artifact.header.debug_info_offset as usize
                ..old_artifact.header.file_length as usize,
            new_artifact.header.debug_info_offset as usize
                ..new_artifact.header.file_length as usize,
        );
    }

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

fn build_region_diff(
    old: &[u8],
    new: &[u8],
    old_range: std::ops::Range<usize>,
    new_range: std::ops::Range<usize>,
) -> Vec<PatchOp> {
    let mut ops = Vec::new();
    append_region_diff(&mut ops, old, new, old_range, new_range);
    ops
}

fn build_function_body_diff(
    old: &[u8],
    new: &[u8],
    old_body: &HermesFunctionBodyLayout,
    new_body: &HermesFunctionBodyLayout,
) -> Vec<PatchOp> {
    let mut ops = Vec::new();
    let instruction_count = old_body.instructions.len().max(new_body.instructions.len());
    for index in 0..instruction_count {
        append_region_diff(
            &mut ops,
            old,
            new,
            old_body.instructions.get(index).cloned().unwrap_or(0..0),
            new_body.instructions.get(index).cloned().unwrap_or(0..0),
        );
    }

    let post_opcode_segment_count = old_body
        .post_opcode_segments
        .len()
        .max(new_body.post_opcode_segments.len());
    for index in 0..post_opcode_segment_count {
        append_region_diff(
            &mut ops,
            old,
            new,
            old_body
                .post_opcode_segments
                .get(index)
                .cloned()
                .unwrap_or(0..0),
            new_body
                .post_opcode_segments
                .get(index)
                .cloned()
                .unwrap_or(0..0),
        );
    }

    ops
}

fn append_info_block_diff(
    ops: &mut Vec<PatchOp>,
    old: &[u8],
    new: &[u8],
    old_block: Option<&HermesFunctionInfoBlock>,
    new_block: Option<&HermesFunctionInfoBlock>,
) {
    let old_ranges = info_block_ranges(old_block);
    let new_ranges = info_block_ranges(new_block);

    append_region_diff(
        ops,
        old,
        new,
        old_ranges.large_header,
        new_ranges.large_header,
    );
    append_region_diff(
        ops,
        old,
        new,
        old_ranges.pre_exception_padding,
        new_ranges.pre_exception_padding,
    );
    append_region_diff(
        ops,
        old,
        new,
        old_ranges.exception_table,
        new_ranges.exception_table,
    );
    append_region_diff(
        ops,
        old,
        new,
        old_ranges.pre_debug_padding,
        new_ranges.pre_debug_padding,
    );
    append_region_diff(
        ops,
        old,
        new,
        old_ranges.debug_offsets,
        new_ranges.debug_offsets,
    );
    append_region_diff(
        ops,
        old,
        new,
        old_ranges.trailing_padding,
        new_ranges.trailing_padding,
    );
}

fn append_debug_info_diff(
    ops: &mut Vec<PatchOp>,
    old: &[u8],
    new: &[u8],
    old_debug: &HermesDebugInfoLayout,
    new_debug: &HermesDebugInfoLayout,
) {
    append_region_diff(
        ops,
        old,
        new,
        old_debug.header_offset as usize..old_debug.header_end_offset as usize,
        new_debug.header_offset as usize..new_debug.header_end_offset as usize,
    );
    append_region_diff(
        ops,
        old,
        new,
        old_debug.filename_table_offset as usize..old_debug.filename_table_end_offset as usize,
        new_debug.filename_table_offset as usize..new_debug.filename_table_end_offset as usize,
    );
    append_region_diff(
        ops,
        old,
        new,
        old_debug.filename_storage_offset as usize..old_debug.filename_storage_end_offset as usize,
        new_debug.filename_storage_offset as usize..new_debug.filename_storage_end_offset as usize,
    );
    append_region_diff(
        ops,
        old,
        new,
        old_debug.file_regions_offset as usize..old_debug.file_regions_end_offset as usize,
        new_debug.file_regions_offset as usize..new_debug.file_regions_end_offset as usize,
    );
    append_debug_data_diff(ops, old, new, old_debug, new_debug);
    append_region_diff(
        ops,
        old,
        new,
        old_debug.debug_data_end_offset as usize..old_debug.end_offset as usize,
        new_debug.debug_data_end_offset as usize..new_debug.end_offset as usize,
    );
}

fn append_debug_data_diff(
    ops: &mut Vec<PatchOp>,
    old: &[u8],
    new: &[u8],
    old_debug: &HermesDebugInfoLayout,
    new_debug: &HermesDebugInfoLayout,
) {
    if old_debug.streams.is_empty() && new_debug.streams.is_empty() {
        append_region_diff(
            ops,
            old,
            new,
            old_debug.debug_data_offset as usize..old_debug.debug_data_end_offset as usize,
            new_debug.debug_data_offset as usize..new_debug.debug_data_end_offset as usize,
        );
        return;
    }

    let mut old_streams_by_function = HashMap::<u32, VecDeque<&HermesDebugDataStream>>::new();
    for stream in &old_debug.streams {
        old_streams_by_function
            .entry(stream.function_index)
            .or_default()
            .push_back(stream);
    }

    for stream in &new_debug.streams {
        let old_stream = old_streams_by_function
            .get_mut(&stream.function_index)
            .and_then(VecDeque::pop_front);
        match old_stream {
            Some(old_stream) if !old_stream.segments.is_empty() && !stream.segments.is_empty() => {
                let structured_ops = build_debug_stream_diff(old, new, old_stream, stream);
                let coarse_ops = build_region_diff(
                    old,
                    new,
                    old_stream.offset as usize..old_stream.end_offset as usize,
                    stream.offset as usize..stream.end_offset as usize,
                );
                let structured_stats = Patch {
                    ops: structured_ops.clone(),
                }
                .stats();
                let coarse_stats = Patch {
                    ops: coarse_ops.clone(),
                }
                .stats();

                if structured_stats.inserted_bytes < coarse_stats.inserted_bytes
                    || (structured_stats.inserted_bytes == coarse_stats.inserted_bytes
                        && structured_stats.op_count < coarse_stats.op_count)
                {
                    ops.extend(structured_ops);
                } else {
                    ops.extend(coarse_ops);
                }
            }
            Some(old_stream) => append_region_diff(
                ops,
                old,
                new,
                old_stream.offset as usize..old_stream.end_offset as usize,
                stream.offset as usize..stream.end_offset as usize,
            ),
            None => append_region_diff(
                ops,
                old,
                new,
                0..0,
                stream.offset as usize..stream.end_offset as usize,
            ),
        }
    }
}

fn build_debug_stream_diff(
    old: &[u8],
    new: &[u8],
    old_stream: &HermesDebugDataStream,
    new_stream: &HermesDebugDataStream,
) -> Vec<PatchOp> {
    let mut ops = Vec::new();
    let segment_count = old_stream.segments.len().max(new_stream.segments.len());
    for index in 0..segment_count {
        append_region_diff(
            &mut ops,
            old,
            new,
            old_stream
                .segments
                .get(index)
                .map(|range| range.start as usize..range.end as usize)
                .unwrap_or(0..0),
            new_stream
                .segments
                .get(index)
                .map(|range| range.start as usize..range.end as usize)
                .unwrap_or(0..0),
        );
    }
    ops
}

#[derive(Debug, Clone)]
struct HermesInfoBlockRanges {
    large_header: std::ops::Range<usize>,
    pre_exception_padding: std::ops::Range<usize>,
    exception_table: std::ops::Range<usize>,
    pre_debug_padding: std::ops::Range<usize>,
    debug_offsets: std::ops::Range<usize>,
    trailing_padding: std::ops::Range<usize>,
}

#[derive(Debug, Clone, Copy)]
struct MiddleAnchor {
    old_start: usize,
    new_start: usize,
    len: usize,
}

#[derive(Debug, Clone)]
struct HermesFunctionBodyLayout {
    instructions: Vec<std::ops::Range<usize>>,
    post_opcode_segments: Vec<std::ops::Range<usize>>,
}

fn info_block_ranges(block: Option<&HermesFunctionInfoBlock>) -> HermesInfoBlockRanges {
    let Some(block) = block else {
        return HermesInfoBlockRanges {
            large_header: 0..0,
            pre_exception_padding: 0..0,
            exception_table: 0..0,
            pre_debug_padding: 0..0,
            debug_offsets: 0..0,
            trailing_padding: 0..0,
        };
    };

    let large_header = block.offset as usize..block.large_header_end_offset as usize;
    let pre_exception_padding = match block.exception_table_offset {
        Some(exception_offset) => block.large_header_end_offset as usize..exception_offset as usize,
        None => 0..0,
    };
    let exception_table = match (
        block.exception_table_offset,
        block.exception_table_end_offset,
    ) {
        (Some(start), Some(end)) => start as usize..end as usize,
        _ => 0..0,
    };
    let pre_debug_padding = match block.debug_offsets_offset {
        Some(debug_offset) => {
            let start = block
                .exception_table_end_offset
                .unwrap_or(block.large_header_end_offset);
            start as usize..debug_offset as usize
        }
        None => 0..0,
    };
    let debug_offsets = match (block.debug_offsets_offset, block.debug_offsets_end_offset) {
        (Some(start), Some(end)) => start as usize..end as usize,
        _ => 0..0,
    };
    let trailing_padding = block.payload_end_offset as usize..block.end_offset as usize;

    HermesInfoBlockRanges {
        large_header,
        pre_exception_padding,
        exception_table,
        pre_debug_padding,
        debug_offsets,
        trailing_padding,
    }
}

fn parse_function_body_layout(
    bytes: &[u8],
    function: &HermesFunction,
) -> Option<HermesFunctionBodyLayout> {
    let opcode_start = function.bytecode_offset as usize;
    let opcode_end = opcode_start.checked_add(function.bytecode_size as usize)?;
    let body_end = function.body_end_offset as usize;
    if opcode_end > body_end || body_end > bytes.len() {
        return None;
    }

    let mut cursor = opcode_start;
    let mut instructions = Vec::new();
    let mut jump_tables = Vec::new();

    while cursor < opcode_end {
        let opcode = *bytes.get(cursor)?;
        let size = usize::from(*HERMES_V98_V99_OPCODE_SIZES.get(opcode as usize)?);
        if size == 0 {
            return None;
        }
        let inst_end = cursor.checked_add(size)?;
        if inst_end > opcode_end {
            return None;
        }

        instructions.push(cursor..inst_end);

        if opcode == HERMES_V98_V99_UINT_SWITCH_IMM_OPCODE {
            let table_offset = usize::try_from(read_u32(bytes, cursor + 2)?).ok()?;
            let min_value = read_u32(bytes, cursor + 10)?;
            let max_value = read_u32(bytes, cursor + 14)?;
            if max_value < min_value {
                return None;
            }
            let entry_count = max_value.checked_sub(min_value)?.checked_add(1)?;
            let table_start = align4(cursor.checked_add(table_offset)?);
            let table_end = table_start.checked_add(entry_count as usize * 4)?;
            jump_tables.push(table_start..table_end);
        } else if opcode == HERMES_V98_V99_STRING_SWITCH_IMM_OPCODE {
            let table_offset = usize::try_from(read_u32(bytes, cursor + 6)?).ok()?;
            let entry_count = read_u32(bytes, cursor + 14)?;
            let table_start = align4(cursor.checked_add(table_offset)?);
            let table_end = table_start.checked_add(entry_count as usize * 8)?;
            jump_tables.push(table_start..table_end);
        }

        cursor = inst_end;
    }

    let post_opcode_segments = build_post_opcode_segments(opcode_end, body_end, jump_tables)?;

    Some(HermesFunctionBodyLayout {
        instructions,
        post_opcode_segments,
    })
}

fn build_post_opcode_segments(
    opcode_end: usize,
    body_end: usize,
    mut jump_tables: Vec<std::ops::Range<usize>>,
) -> Option<Vec<std::ops::Range<usize>>> {
    if opcode_end > body_end {
        return None;
    }

    let mut segments = Vec::new();
    let mut cursor = opcode_end;
    let post_opcode_start = align4(opcode_end).min(body_end);

    if cursor < post_opcode_start {
        segments.push(cursor..post_opcode_start);
        cursor = post_opcode_start;
    }

    jump_tables.sort_by_key(|range| range.start);
    for table in jump_tables {
        if table.start < post_opcode_start || table.end > body_end || table.start > table.end {
            return None;
        }
        if table.start < cursor {
            return None;
        }
        if cursor < table.start {
            segments.push(cursor..table.start);
        }
        segments.push(table.clone());
        cursor = table.end;
    }

    if cursor < body_end {
        segments.push(cursor..body_end);
    }

    Some(segments)
}

fn align4(value: usize) -> usize {
    (value + 3) & !3
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn append_prefix_suffix_diff(ops: &mut Vec<PatchOp>, old: &[u8], new: &[u8], old_base: usize) {
    append_resync_diff(ops, old, new, old_base);
}

fn append_resync_diff(ops: &mut Vec<PatchOp>, old: &[u8], new: &[u8], old_base: usize) {
    let prefix_len = common_prefix_len(old, new);
    let suffix_len = common_suffix_len(old, new, prefix_len);

    let old_mid_end = old.len() - suffix_len;
    let new_mid_end = new.len() - suffix_len;
    let old_mid = &old[prefix_len..old_mid_end];
    let new_mid = &new[prefix_len..new_mid_end];

    if let Some(anchor) = find_middle_anchor(old_mid, new_mid) {
        if prefix_len > 0 {
            push_op(
                ops,
                PatchOp::Copy {
                    offset: old_base,
                    len: prefix_len,
                },
            );
        }

        append_resync_diff(
            ops,
            &old_mid[..anchor.old_start],
            &new_mid[..anchor.new_start],
            old_base + prefix_len,
        );

        push_op(
            ops,
            PatchOp::Copy {
                offset: old_base + prefix_len + anchor.old_start,
                len: anchor.len,
            },
        );

        append_resync_diff(
            ops,
            &old_mid[anchor.old_start + anchor.len..],
            &new_mid[anchor.new_start + anchor.len..],
            old_base + prefix_len + anchor.old_start + anchor.len,
        );

        if suffix_len > 0 {
            push_op(
                ops,
                PatchOp::Copy {
                    offset: old_base + old_mid_end,
                    len: suffix_len,
                },
            );
        }
        return;
    }

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

fn unique_function_body_ranges(functions: &[crate::HermesFunction]) -> Vec<std::ops::Range<usize>> {
    let mut ranges = functions
        .iter()
        .map(|function| function.bytecode_offset as usize..function.body_end_offset as usize)
        .collect::<Vec<_>>();
    ranges.sort_by_key(|range| (range.start, range.end));
    ranges.dedup_by(|rhs, lhs| rhs.start == lhs.start && rhs.end == lhs.end);
    ranges
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

fn find_middle_anchor(old: &[u8], new: &[u8]) -> Option<MiddleAnchor> {
    let Some(step) = middle_anchor_step(old.len(), new.len()) else {
        return None;
    };

    let mut positions_by_key: HashMap<u32, Vec<usize>> = HashMap::new();
    for old_start in sampled_starts(old.len(), step) {
        let key = window_key(&old[old_start..old_start + RESYNC_ANCHOR_WINDOW]);
        let positions = positions_by_key.entry(key).or_default();
        if positions.len() < RESYNC_MAX_POSITIONS_PER_KEY {
            positions.push(old_start);
        }
    }

    let mut best_match = None;
    for new_start in sampled_starts(new.len(), step) {
        let key = window_key(&new[new_start..new_start + RESYNC_ANCHOR_WINDOW]);
        let Some(old_positions) = positions_by_key.get(&key) else {
            continue;
        };

        for &old_start in old_positions {
            let mut start_old = old_start;
            let mut start_new = new_start;
            let mut end_old = old_start + RESYNC_ANCHOR_WINDOW;
            let mut end_new = new_start + RESYNC_ANCHOR_WINDOW;

            while start_old > 0 && start_new > 0 && old[start_old - 1] == new[start_new - 1] {
                start_old -= 1;
                start_new -= 1;
            }

            while end_old < old.len() && end_new < new.len() && old[end_old] == new[end_new] {
                end_old += 1;
                end_new += 1;
            }

            let candidate = MiddleAnchor {
                old_start: start_old,
                new_start: start_new,
                len: end_old - start_old,
            };

            if candidate.len < RESYNC_MIN_MATCH_LEN {
                continue;
            }

            let replace_whole_region = candidate.old_start == 0
                && candidate.new_start == 0
                && candidate.len == old.len().min(new.len());
            if replace_whole_region {
                continue;
            }

            let should_replace_best = best_match
                .map(|best: MiddleAnchor| {
                    candidate.len > best.len
                        || (candidate.len == best.len
                            && candidate.old_start + candidate.new_start
                                < best.old_start + best.new_start)
                })
                .unwrap_or(true);

            if should_replace_best {
                best_match = Some(candidate);
            }
        }
    }

    best_match
}

fn window_key(window: &[u8]) -> u32 {
    debug_assert_eq!(window.len(), RESYNC_ANCHOR_WINDOW);
    u32::from_le_bytes(window.try_into().expect("window has fixed size"))
}

fn middle_anchor_step(old_len: usize, new_len: usize) -> Option<usize> {
    if old_len < RESYNC_MIN_MATCH_LEN
        || new_len < RESYNC_MIN_MATCH_LEN
        || old_len < RESYNC_ANCHOR_WINDOW
        || new_len < RESYNC_ANCHOR_WINDOW
    {
        return None;
    }

    let max_len = old_len.max(new_len);
    if max_len > RESYNC_MAX_REGION_LEN {
        return None;
    }

    if max_len <= RESYNC_FULL_SCAN_MAX_REGION_LEN {
        return Some(1);
    }

    Some((max_len / RESYNC_TARGET_SAMPLE_COUNT).max(1))
}

fn sampled_starts(len: usize, step: usize) -> Vec<usize> {
    debug_assert!(step > 0);
    debug_assert!(len >= RESYNC_ANCHOR_WINDOW);

    let last_start = len - RESYNC_ANCHOR_WINDOW;
    let mut starts = Vec::new();
    let mut cursor = 0usize;
    while cursor <= last_start {
        starts.push(cursor);
        cursor = cursor.saturating_add(step);
    }

    if starts.last().copied() != Some(last_start) {
        starts.push(last_start);
    }

    starts
}

#[cfg(test)]
mod tests {
    use super::{middle_anchor_step, sampled_starts, Patch, PatchOp};

    #[test]
    fn middle_anchor_uses_full_scan_for_small_regions() {
        assert_eq!(middle_anchor_step(64, 96), Some(1));
    }

    #[test]
    fn middle_anchor_uses_sampling_for_large_regions() {
        let step = middle_anchor_step(512 * 1024, 512 * 1024).unwrap();
        assert!(step > 1);
    }

    #[test]
    fn middle_anchor_is_disabled_for_oversized_regions() {
        assert_eq!(middle_anchor_step(8 * 1024 * 1024 + 1, 128), None);
        assert_eq!(middle_anchor_step(128, 8 * 1024 * 1024 + 1), None);
    }

    #[test]
    fn sampled_starts_include_region_end() {
        assert_eq!(sampled_starts(20, 7), vec![0, 7, 14, 16]);
    }

    #[test]
    fn patch_normalized_merges_adjacent_copy_ops() {
        let patch = Patch {
            ops: vec![
                PatchOp::Copy { offset: 10, len: 4 },
                PatchOp::Copy { offset: 14, len: 6 },
            ],
        }
        .normalized();

        assert_eq!(
            patch,
            Patch {
                ops: vec![PatchOp::Copy {
                    offset: 10,
                    len: 10
                }],
            }
        );
    }

    #[test]
    fn patch_normalized_merges_adjacent_insert_ops() {
        let patch = Patch {
            ops: vec![PatchOp::Insert(vec![1, 2]), PatchOp::Insert(vec![3, 4])],
        }
        .normalized();

        assert_eq!(
            patch,
            Patch {
                ops: vec![PatchOp::Insert(vec![1, 2, 3, 4])],
            }
        );
    }
}
