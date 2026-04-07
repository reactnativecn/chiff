use crate::format::{detect_input_format, HermesForm, InputFormat};
use std::collections::HashMap;

const HERMES_HEADER_LEN: usize = 128;
const BYTECODE_ALIGNMENT: u32 = 4;
const SMALL_FUNCTION_OFFSET_MASK: u32 = (1 << 25) - 1;
const SMALL_FUNCTION_BYTECODE_SIZE_MASK: u32 = (1 << 14) - 1;
const SMALL_FUNCTION_INFO_OFFSET_MASK: u32 = (1 << 24) - 1;
const OVERFLOWED_FUNCTION_HEADER_OFFSET_MASK: u32 = (1 << 24) - 1;
const SMALL_FUNCTION_NAME_SHIFT: u32 = 14;
const SMALL_FUNCTION_NAME_MASK: u32 = 0xFF;
const FUNCTION_HEADER_HAS_EXCEPTION_HANDLER_MASK: u8 = 1 << 3;
const FUNCTION_HEADER_HAS_DEBUG_INFO_MASK: u8 = 1 << 4;
const FUNCTION_HEADER_OVERFLOWED_MASK: u8 = 1 << 5;
const FILE_LENGTH_OFFSET: usize = 32;
const GLOBAL_CODE_INDEX_OFFSET: usize = 36;
const FUNCTION_COUNT_OFFSET: usize = 40;
const STRING_KIND_COUNT_OFFSET: usize = 44;
const IDENTIFIER_COUNT_OFFSET: usize = 48;
const STRING_COUNT_OFFSET: usize = 52;
const OVERFLOW_STRING_COUNT_OFFSET: usize = 56;
const STRING_STORAGE_SIZE_OFFSET: usize = 60;
const BIG_INT_COUNT_OFFSET: usize = 64;
const BIG_INT_STORAGE_SIZE_OFFSET: usize = 68;
const REG_EXP_COUNT_OFFSET: usize = 72;
const REG_EXP_STORAGE_SIZE_OFFSET: usize = 76;
const LITERAL_VALUE_BUFFER_SIZE_OFFSET: usize = 80;
const OBJ_KEY_BUFFER_SIZE_OFFSET: usize = 84;
const OBJ_SHAPE_TABLE_COUNT_OFFSET: usize = 88;
const NUM_STRING_SWITCH_IMMS_OFFSET: usize = 92;
const SEGMENT_ID_OFFSET: usize = 96;
const CJS_MODULE_COUNT_OFFSET: usize = 100;
const FUNCTION_SOURCE_COUNT_OFFSET: usize = 104;
const DEBUG_INFO_OFFSET_OFFSET: usize = 108;
const OPTIONS_FLAGS_OFFSET: usize = 112;

const SMALL_FUNC_HEADER_SIZE: u32 = 12;
const LARGE_FUNC_HEADER_SIZE: u32 = 36;
const LARGE_FUNC_HEADER_OFFSET_OFFSET: usize = 0;
const LARGE_FUNC_HEADER_BYTECODE_SIZE_OFFSET: usize = 12;
const LARGE_FUNC_HEADER_FLAGS_OFFSET: usize = 35;
const EXCEPTION_HANDLER_TABLE_HEADER_SIZE: u32 = 4;
const EXCEPTION_HANDLER_INFO_SIZE: u32 = 12;
const DEBUG_OFFSETS_SIZE: u32 = 4;
const STRING_KIND_ENTRY_SIZE: u32 = 4;
const IDENTIFIER_HASH_SIZE: u32 = 4;
const SMALL_STRING_TABLE_ENTRY_SIZE: u32 = 4;
const OVERFLOW_STRING_TABLE_ENTRY_SIZE: u32 = 8;
const OBJ_SHAPE_TABLE_ENTRY_SIZE: u32 = 8;
const BIG_INT_TABLE_ENTRY_SIZE: u32 = 8;
const REG_EXP_TABLE_ENTRY_SIZE: u32 = 8;
const U32_PAIR_SIZE: u32 = 8;
const DEBUG_INFO_HEADER_SIZE: u32 = 16;
const DEBUG_INFO_FILENAME_ENTRY_SIZE: u32 = 8;
const DEBUG_FILE_REGION_ENTRY_SIZE: u32 = 12;
pub const SUPPORTED_STRUCTURED_HERMES_VERSIONS: &[u32] = &[98, 99];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredHermesSupport {
    NotHermes,
    InvalidHeader,
    UnsupportedVersion { version: u32, form: HermesForm },
    Supported { version: u32, form: HermesForm },
}

impl StructuredHermesSupport {
    pub fn is_supported(self) -> bool {
        matches!(self, Self::Supported { .. })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotHermes => "not_hermes",
            Self::InvalidHeader => "invalid_header",
            Self::UnsupportedVersion { .. } => "unsupported_version",
            Self::Supported { .. } => "supported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesHeader {
    pub version: u32,
    pub form: HermesForm,
    pub file_length: u32,
    pub global_code_index: u32,
    pub function_count: u32,
    pub string_kind_count: u32,
    pub identifier_count: u32,
    pub string_count: u32,
    pub overflow_string_count: u32,
    pub string_storage_size: u32,
    pub big_int_count: u32,
    pub big_int_storage_size: u32,
    pub reg_exp_count: u32,
    pub reg_exp_storage_size: u32,
    pub literal_value_buffer_size: u32,
    pub obj_key_buffer_size: u32,
    pub obj_shape_table_count: u32,
    pub num_string_switch_imms: u32,
    pub segment_id: u32,
    pub cjs_module_count: u32,
    pub function_source_count: u32,
    pub debug_info_offset: u32,
    pub options_flags: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HermesSectionKind {
    FunctionHeaders,
    StringKinds,
    IdentifierHashes,
    SmallStringTable,
    OverflowStringTable,
    StringStorage,
    LiteralValueBuffer,
    ObjectKeyBuffer,
    ObjectShapeTable,
    BigIntTable,
    BigIntStorage,
    RegExpTable,
    RegExpStorage,
    CjsModuleTable,
    FunctionSourceTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesSection {
    pub kind: HermesSectionKind,
    pub offset: u32,
    pub len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesSectionLayout {
    pub sections: Vec<HermesSection>,
    pub structured_end_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesFunction {
    pub index: u32,
    pub header_offset: u32,
    pub bytecode_offset: u32,
    pub bytecode_size: u32,
    pub body_end_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesFunctionLayout {
    pub functions: Vec<HermesFunction>,
    pub info_blocks: Vec<HermesFunctionInfoBlock>,
    pub bytecode_region_start: u32,
    pub bytecode_region_end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesFunctionInfoBlock {
    pub index: u32,
    pub offset: u32,
    pub large_header_end_offset: u32,
    pub exception_table_offset: Option<u32>,
    pub exception_table_end_offset: Option<u32>,
    pub debug_offsets_offset: Option<u32>,
    pub debug_offsets_end_offset: Option<u32>,
    pub payload_end_offset: u32,
    pub end_offset: u32,
    pub has_exception_handlers: bool,
    pub has_debug_offsets: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesDebugInfoHeader {
    pub filename_count: u32,
    pub filename_storage_size: u32,
    pub file_region_count: u32,
    pub debug_data_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesDebugFileRegion {
    pub from_address: u32,
    pub filename_id: u32,
    pub source_mapping_url_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesDebugDataStream {
    pub function_index: u32,
    pub offset: u32,
    pub end_offset: u32,
    pub segments: Vec<std::ops::Range<u32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesDebugInfoLayout {
    pub header: HermesDebugInfoHeader,
    pub file_regions: Vec<HermesDebugFileRegion>,
    pub streams: Vec<HermesDebugDataStream>,
    pub header_offset: u32,
    pub header_end_offset: u32,
    pub filename_table_offset: u32,
    pub filename_table_end_offset: u32,
    pub filename_storage_offset: u32,
    pub filename_storage_end_offset: u32,
    pub file_regions_offset: u32,
    pub file_regions_end_offset: u32,
    pub debug_data_offset: u32,
    pub debug_data_end_offset: u32,
    pub end_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesArtifact {
    pub header: HermesHeader,
    pub payload_len: usize,
    pub section_layout: HermesSectionLayout,
    pub function_layout: Option<HermesFunctionLayout>,
    pub debug_info_layout: Option<HermesDebugInfoLayout>,
}

#[derive(Debug, Clone, Copy)]
struct RawFunctionInfoBlock {
    function_index: u32,
    offset: u32,
    large_header_end_offset: u32,
    has_exception_handlers: bool,
    has_debug_offsets: bool,
}

pub fn supports_structured_hermes_version(version: u32) -> bool {
    SUPPORTED_STRUCTURED_HERMES_VERSIONS.contains(&version)
}

pub fn assess_structured_hermes(bytes: &[u8]) -> StructuredHermesSupport {
    let InputFormat::HermesBytecode { version, form } = detect_input_format(bytes) else {
        return StructuredHermesSupport::NotHermes;
    };

    if parse_header(bytes).is_none() {
        return StructuredHermesSupport::InvalidHeader;
    }

    if supports_structured_hermes_version(version) {
        StructuredHermesSupport::Supported { version, form }
    } else {
        StructuredHermesSupport::UnsupportedVersion { version, form }
    }
}

pub fn can_use_structured_hermes(bytes: &[u8]) -> bool {
    assess_structured_hermes(bytes).is_supported()
}

pub fn parse_artifact(bytes: &[u8]) -> Option<HermesArtifact> {
    let header = parse_header(bytes)?;
    let section_layout = parse_section_layout_from_header(bytes, &header)?;
    let function_layout = parse_function_layout_from_parts(bytes, &header, &section_layout);
    let debug_info_layout = parse_debug_info_layout_from_header(bytes, &header);

    Some(HermesArtifact {
        header,
        payload_len: bytes.len(),
        section_layout,
        function_layout,
        debug_info_layout,
    })
}

pub fn parse_header(bytes: &[u8]) -> Option<HermesHeader> {
    if bytes.len() < HERMES_HEADER_LEN {
        return None;
    }

    match detect_input_format(bytes) {
        InputFormat::HermesBytecode { version, form } => Some(HermesHeader {
            version,
            form,
            file_length: read_u32(bytes, FILE_LENGTH_OFFSET)?,
            global_code_index: read_u32(bytes, GLOBAL_CODE_INDEX_OFFSET)?,
            function_count: read_u32(bytes, FUNCTION_COUNT_OFFSET)?,
            string_kind_count: read_u32(bytes, STRING_KIND_COUNT_OFFSET)?,
            identifier_count: read_u32(bytes, IDENTIFIER_COUNT_OFFSET)?,
            string_count: read_u32(bytes, STRING_COUNT_OFFSET)?,
            overflow_string_count: read_u32(bytes, OVERFLOW_STRING_COUNT_OFFSET)?,
            string_storage_size: read_u32(bytes, STRING_STORAGE_SIZE_OFFSET)?,
            big_int_count: read_u32(bytes, BIG_INT_COUNT_OFFSET)?,
            big_int_storage_size: read_u32(bytes, BIG_INT_STORAGE_SIZE_OFFSET)?,
            reg_exp_count: read_u32(bytes, REG_EXP_COUNT_OFFSET)?,
            reg_exp_storage_size: read_u32(bytes, REG_EXP_STORAGE_SIZE_OFFSET)?,
            literal_value_buffer_size: read_u32(bytes, LITERAL_VALUE_BUFFER_SIZE_OFFSET)?,
            obj_key_buffer_size: read_u32(bytes, OBJ_KEY_BUFFER_SIZE_OFFSET)?,
            obj_shape_table_count: read_u32(bytes, OBJ_SHAPE_TABLE_COUNT_OFFSET)?,
            num_string_switch_imms: read_u32(bytes, NUM_STRING_SWITCH_IMMS_OFFSET)?,
            segment_id: read_u32(bytes, SEGMENT_ID_OFFSET)?,
            cjs_module_count: read_u32(bytes, CJS_MODULE_COUNT_OFFSET)?,
            function_source_count: read_u32(bytes, FUNCTION_SOURCE_COUNT_OFFSET)?,
            debug_info_offset: read_u32(bytes, DEBUG_INFO_OFFSET_OFFSET)?,
            options_flags: *bytes.get(OPTIONS_FLAGS_OFFSET)?,
        }),
        _ => None,
    }
}

pub fn parse_section_layout(bytes: &[u8]) -> Option<HermesSectionLayout> {
    let header = parse_header(bytes)?;
    parse_section_layout_from_header(bytes, &header)
}

pub fn parse_function_layout(bytes: &[u8]) -> Option<HermesFunctionLayout> {
    let header = parse_header(bytes)?;
    let section_layout = parse_section_layout_from_header(bytes, &header)?;
    parse_function_layout_from_parts(bytes, &header, &section_layout)
}

pub fn parse_debug_info_layout(bytes: &[u8]) -> Option<HermesDebugInfoLayout> {
    let header = parse_header(bytes)?;
    parse_debug_info_layout_from_header(bytes, &header)
}

fn parse_section_layout_from_header(
    bytes: &[u8],
    header: &HermesHeader,
) -> Option<HermesSectionLayout> {
    validate_artifact_bounds(bytes, header)?;

    let mut sections = Vec::new();
    let mut cursor = HERMES_HEADER_LEN as u32;

    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::FunctionHeaders,
        multiply_u32(header.function_count, SMALL_FUNC_HEADER_SIZE)?,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::StringKinds,
        multiply_u32(header.string_kind_count, STRING_KIND_ENTRY_SIZE)?,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::IdentifierHashes,
        multiply_u32(header.identifier_count, IDENTIFIER_HASH_SIZE)?,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::SmallStringTable,
        multiply_u32(header.string_count, SMALL_STRING_TABLE_ENTRY_SIZE)?,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::OverflowStringTable,
        multiply_u32(
            header.overflow_string_count,
            OVERFLOW_STRING_TABLE_ENTRY_SIZE,
        )?,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::StringStorage,
        header.string_storage_size,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::LiteralValueBuffer,
        header.literal_value_buffer_size,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::ObjectKeyBuffer,
        header.obj_key_buffer_size,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::ObjectShapeTable,
        multiply_u32(header.obj_shape_table_count, OBJ_SHAPE_TABLE_ENTRY_SIZE)?,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::BigIntTable,
        multiply_u32(header.big_int_count, BIG_INT_TABLE_ENTRY_SIZE)?,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::BigIntStorage,
        header.big_int_storage_size,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::RegExpTable,
        multiply_u32(header.reg_exp_count, REG_EXP_TABLE_ENTRY_SIZE)?,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::RegExpStorage,
        header.reg_exp_storage_size,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::CjsModuleTable,
        multiply_u32(header.cjs_module_count, U32_PAIR_SIZE)?,
        header,
        bytes,
    )?;
    push_section(
        &mut sections,
        &mut cursor,
        HermesSectionKind::FunctionSourceTable,
        multiply_u32(header.function_source_count, U32_PAIR_SIZE)?,
        header,
        bytes,
    )?;

    if header.debug_info_offset < cursor {
        return None;
    }

    Some(HermesSectionLayout {
        sections,
        structured_end_offset: cursor,
    })
}

fn parse_function_layout_from_parts(
    bytes: &[u8],
    header: &HermesHeader,
    section_layout: &HermesSectionLayout,
) -> Option<HermesFunctionLayout> {
    let Some(function_headers_section) = section_layout
        .sections
        .iter()
        .find(|section| section.kind == HermesSectionKind::FunctionHeaders)
    else {
        if header.function_count == 0 {
            return Some(HermesFunctionLayout {
                functions: Vec::new(),
                info_blocks: Vec::new(),
                bytecode_region_start: section_layout.structured_end_offset,
                bytecode_region_end: section_layout.structured_end_offset,
            });
        }

        return None;
    };

    let expected_len = multiply_u32(header.function_count, SMALL_FUNC_HEADER_SIZE)?;
    if function_headers_section.len != expected_len {
        return None;
    }

    let mut functions = Vec::with_capacity(header.function_count as usize);
    let mut bytecode_region_end = header.debug_info_offset;
    let mut raw_info_blocks = Vec::new();

    for index in 0..header.function_count {
        let header_offset =
            function_headers_section.offset + index.checked_mul(SMALL_FUNC_HEADER_SIZE)?;
        let raw_header = bytes.get(header_offset as usize..header_offset as usize + 12)?;
        let flags = *raw_header.get(11)?;

        let word1 = u32::from_le_bytes(raw_header.get(0..4)?.try_into().ok()?);
        let word2 = u32::from_le_bytes(raw_header.get(4..8)?.try_into().ok()?);
        let word3 = u32::from_le_bytes(raw_header.get(8..12)?.try_into().ok()?);
        let (bytecode_offset, bytecode_size) = if flags & FUNCTION_HEADER_OVERFLOWED_MASK != 0 {
            let large_header_offset = decode_large_header_offset(word1, word2);
            if large_header_offset < section_layout.structured_end_offset
                || large_header_offset % BYTECODE_ALIGNMENT != 0
            {
                return None;
            }

            let large_header_end = large_header_offset.checked_add(LARGE_FUNC_HEADER_SIZE)?;
            if large_header_end > header.debug_info_offset {
                return None;
            }

            bytecode_region_end = bytecode_region_end.min(large_header_offset);
            let large_flags =
                *bytes.get(large_header_offset as usize + LARGE_FUNC_HEADER_FLAGS_OFFSET)?;
            let has_exception_handlers =
                large_flags & FUNCTION_HEADER_HAS_EXCEPTION_HANDLER_MASK != 0;
            let has_debug_offsets = large_flags & FUNCTION_HEADER_HAS_DEBUG_INFO_MASK != 0;
            raw_info_blocks.push(RawFunctionInfoBlock {
                function_index: index,
                offset: large_header_offset,
                large_header_end_offset: large_header_offset.checked_add(LARGE_FUNC_HEADER_SIZE)?,
                has_exception_handlers,
                has_debug_offsets,
            });

            (
                read_u32_at_u32(bytes, large_header_offset, LARGE_FUNC_HEADER_OFFSET_OFFSET)?,
                read_u32_at_u32(
                    bytes,
                    large_header_offset,
                    LARGE_FUNC_HEADER_BYTECODE_SIZE_OFFSET,
                )?,
            )
        } else {
            let has_exception_handlers = flags & FUNCTION_HEADER_HAS_EXCEPTION_HANDLER_MASK != 0;
            let has_debug_offsets = flags & FUNCTION_HEADER_HAS_DEBUG_INFO_MASK != 0;
            if has_exception_handlers || has_debug_offsets {
                let info_offset = word3 & SMALL_FUNCTION_INFO_OFFSET_MASK;
                if info_offset < section_layout.structured_end_offset
                    || info_offset >= header.debug_info_offset
                    || info_offset % BYTECODE_ALIGNMENT != 0
                {
                    return None;
                }
                bytecode_region_end = bytecode_region_end.min(info_offset);
                raw_info_blocks.push(RawFunctionInfoBlock {
                    function_index: index,
                    offset: info_offset,
                    large_header_end_offset: info_offset,
                    has_exception_handlers,
                    has_debug_offsets,
                });
            }

            (
                word1 & SMALL_FUNCTION_OFFSET_MASK,
                word2 & SMALL_FUNCTION_BYTECODE_SIZE_MASK,
            )
        };

        if bytecode_offset < section_layout.structured_end_offset {
            return None;
        }

        let opcode_end = bytecode_offset.checked_add(bytecode_size)?;
        if opcode_end > header.debug_info_offset {
            return None;
        }

        functions.push(HermesFunction {
            index,
            header_offset,
            bytecode_offset,
            bytecode_size,
            body_end_offset: 0,
        });
    }

    let bytecode_region_start = functions
        .iter()
        .map(|function| function.bytecode_offset)
        .min()
        .unwrap_or(section_layout.structured_end_offset);
    let bytecode_region_end = if functions.is_empty() {
        section_layout.structured_end_offset
    } else {
        bytecode_region_end
    };
    raw_info_blocks.sort_by_key(|block| block.offset);
    let info_blocks = build_info_blocks(bytes, header, &raw_info_blocks)?;

    let body_end_by_offset = build_function_body_end_offsets(&functions, bytecode_region_end)?;
    for function in &mut functions {
        let body_end_offset = *body_end_by_offset.get(&function.bytecode_offset)?;
        if function
            .bytecode_offset
            .checked_add(function.bytecode_size)?
            > body_end_offset
        {
            return None;
        }
        function.body_end_offset = body_end_offset;
    }

    Some(HermesFunctionLayout {
        functions,
        info_blocks,
        bytecode_region_start,
        bytecode_region_end,
    })
}

fn parse_debug_info_layout_from_header(
    bytes: &[u8],
    header: &HermesHeader,
) -> Option<HermesDebugInfoLayout> {
    validate_artifact_bounds(bytes, header)?;

    let header_offset = header.debug_info_offset;
    let header_end_offset = header_offset.checked_add(DEBUG_INFO_HEADER_SIZE)?;
    if header_end_offset > header.file_length {
        return None;
    }

    let debug_header = HermesDebugInfoHeader {
        filename_count: read_u32_at_u32(bytes, header_offset, 0)?,
        filename_storage_size: read_u32_at_u32(bytes, header_offset, 4)?,
        file_region_count: read_u32_at_u32(bytes, header_offset, 8)?,
        debug_data_size: read_u32_at_u32(bytes, header_offset, 12)?,
    };

    let filename_table_offset = header_end_offset;
    let filename_table_end_offset = filename_table_offset.checked_add(multiply_u32(
        debug_header.filename_count,
        DEBUG_INFO_FILENAME_ENTRY_SIZE,
    )?)?;
    let filename_storage_offset = filename_table_end_offset;
    let filename_storage_end_offset =
        filename_storage_offset.checked_add(debug_header.filename_storage_size)?;
    let file_regions_offset = filename_storage_end_offset;
    let file_regions_end_offset = file_regions_offset.checked_add(multiply_u32(
        debug_header.file_region_count,
        DEBUG_FILE_REGION_ENTRY_SIZE,
    )?)?;
    let debug_data_offset = file_regions_end_offset;
    let debug_data_end_offset = debug_data_offset.checked_add(debug_header.debug_data_size)?;

    if debug_data_end_offset > header.file_length {
        return None;
    }

    let mut file_regions = Vec::with_capacity(debug_header.file_region_count as usize);
    let mut previous_from_address = 0u32;
    for index in 0..debug_header.file_region_count {
        let region_offset =
            file_regions_offset.checked_add(index.checked_mul(DEBUG_FILE_REGION_ENTRY_SIZE)?)?;
        let region = HermesDebugFileRegion {
            from_address: read_u32_at_u32(bytes, region_offset, 0)?,
            filename_id: read_u32_at_u32(bytes, region_offset, 4)?,
            source_mapping_url_id: read_u32_at_u32(bytes, region_offset, 8)?,
        };
        if index > 0 && region.from_address < previous_from_address {
            return None;
        }
        if region.from_address > debug_header.debug_data_size {
            return None;
        }
        previous_from_address = region.from_address;
        file_regions.push(region);
    }

    let streams = parse_debug_data_streams(
        bytes.get(debug_data_offset as usize..debug_data_end_offset as usize)?,
        debug_data_offset,
    )?;

    Some(HermesDebugInfoLayout {
        header: debug_header,
        file_regions,
        streams,
        header_offset,
        header_end_offset,
        filename_table_offset,
        filename_table_end_offset,
        filename_storage_offset,
        filename_storage_end_offset,
        file_regions_offset,
        file_regions_end_offset,
        debug_data_offset,
        debug_data_end_offset,
        end_offset: header.file_length,
    })
}

fn parse_debug_data_streams(
    debug_data: &[u8],
    debug_data_offset: u32,
) -> Option<Vec<HermesDebugDataStream>> {
    let mut streams = Vec::new();
    let mut relative_offset = 0usize;

    while relative_offset < debug_data.len() {
        let stream_start = relative_offset;
        let mut segments = Vec::new();

        let (function_index, function_index_range) =
            read_signed_leb128_with_range(debug_data, &mut relative_offset)?;
        segments.push(to_absolute_u32_range(
            debug_data_offset,
            function_index_range,
        )?);
        let function_index = u32::try_from(function_index).ok()?;

        for _ in 0..3 {
            let (_, range) = read_signed_leb128_with_range(debug_data, &mut relative_offset)?;
            segments.push(to_absolute_u32_range(debug_data_offset, range)?);
        }

        loop {
            let (address_delta, address_delta_range) =
                read_signed_leb128_with_range(debug_data, &mut relative_offset)?;
            segments.push(to_absolute_u32_range(
                debug_data_offset,
                address_delta_range,
            )?);
            if address_delta == -1 {
                break;
            }

            let (line_delta, line_delta_range) =
                read_signed_leb128_with_range(debug_data, &mut relative_offset)?;
            segments.push(to_absolute_u32_range(debug_data_offset, line_delta_range)?);
            if (line_delta & 1) == 0 {
                continue;
            }

            let (_, range) = read_signed_leb128_with_range(debug_data, &mut relative_offset)?;
            segments.push(to_absolute_u32_range(debug_data_offset, range)?);
            if (line_delta & 2) != 0 {
                let (_, range) = read_signed_leb128_with_range(debug_data, &mut relative_offset)?;
                segments.push(to_absolute_u32_range(debug_data_offset, range)?);
            }
            if (line_delta & 4) != 0 {
                let (_, range) = read_signed_leb128_with_range(debug_data, &mut relative_offset)?;
                segments.push(to_absolute_u32_range(debug_data_offset, range)?);
            }
        }

        let offset = debug_data_offset.checked_add(u32::try_from(stream_start).ok()?)?;
        let end_offset = debug_data_offset.checked_add(u32::try_from(relative_offset).ok()?)?;
        streams.push(HermesDebugDataStream {
            function_index,
            offset,
            end_offset,
            segments,
        });
    }

    Some(streams)
}

fn build_function_body_end_offsets(
    functions: &[HermesFunction],
    bytecode_region_end: u32,
) -> Option<HashMap<u32, u32>> {
    let mut unique_offsets = functions
        .iter()
        .map(|function| function.bytecode_offset)
        .collect::<Vec<_>>();
    unique_offsets.sort_unstable();
    unique_offsets.dedup();

    let mut body_end_by_offset = HashMap::with_capacity(unique_offsets.len());
    for (index, start_offset) in unique_offsets.iter().copied().enumerate() {
        let end_offset = unique_offsets
            .get(index + 1)
            .copied()
            .unwrap_or(bytecode_region_end);
        if end_offset < start_offset {
            return None;
        }
        body_end_by_offset.insert(start_offset, end_offset);
    }

    Some(body_end_by_offset)
}

fn build_info_blocks(
    bytes: &[u8],
    header: &HermesHeader,
    raw_info_blocks: &[RawFunctionInfoBlock],
) -> Option<Vec<HermesFunctionInfoBlock>> {
    let mut info_blocks = Vec::with_capacity(raw_info_blocks.len());
    let mut previous_offset = 0u32;

    for (index, raw_info_block) in raw_info_blocks.iter().enumerate() {
        let offset = raw_info_block.offset;
        if offset < previous_offset {
            return None;
        }
        previous_offset = offset;

        let end_offset = raw_info_blocks
            .get(index + 1)
            .map(|next| next.offset)
            .unwrap_or(header.debug_info_offset);
        let parsed_info = parse_function_info_payload(
            bytes,
            offset,
            raw_info_block.large_header_end_offset,
            end_offset,
            raw_info_block.has_exception_handlers,
            raw_info_block.has_debug_offsets,
        )?;

        info_blocks.push(HermesFunctionInfoBlock {
            index: raw_info_block.function_index,
            offset,
            large_header_end_offset: parsed_info.large_header_end_offset,
            exception_table_offset: parsed_info.exception_table_offset,
            exception_table_end_offset: parsed_info.exception_table_end_offset,
            debug_offsets_offset: parsed_info.debug_offsets_offset,
            debug_offsets_end_offset: parsed_info.debug_offsets_end_offset,
            payload_end_offset: parsed_info.payload_end_offset,
            end_offset,
            has_exception_handlers: raw_info_block.has_exception_handlers,
            has_debug_offsets: raw_info_block.has_debug_offsets,
        });
    }

    Some(info_blocks)
}

struct ParsedFunctionInfoPayload {
    large_header_end_offset: u32,
    exception_table_offset: Option<u32>,
    exception_table_end_offset: Option<u32>,
    debug_offsets_offset: Option<u32>,
    debug_offsets_end_offset: Option<u32>,
    payload_end_offset: u32,
}

fn parse_function_info_payload(
    bytes: &[u8],
    info_offset: u32,
    payload_start_offset: u32,
    max_end_offset: u32,
    has_exception_handlers: bool,
    has_debug_offsets: bool,
) -> Option<ParsedFunctionInfoPayload> {
    let large_header_end_offset = payload_start_offset;
    let mut cursor = payload_start_offset;
    let mut exception_table_offset = None;
    let mut exception_table_end_offset = None;
    let mut debug_offsets_offset = None;
    let mut debug_offsets_end_offset = None;

    if large_header_end_offset < info_offset || large_header_end_offset > max_end_offset {
        return None;
    }

    if has_exception_handlers {
        cursor = align_to(cursor, BYTECODE_ALIGNMENT)?;
        exception_table_offset = Some(cursor);
        let count = read_u32_at_u32(bytes, cursor, 0)?;
        let table_len = EXCEPTION_HANDLER_TABLE_HEADER_SIZE
            .checked_add(multiply_u32(count, EXCEPTION_HANDLER_INFO_SIZE)?)?;
        cursor = cursor.checked_add(table_len)?;
        exception_table_end_offset = Some(cursor);
    }

    if has_debug_offsets {
        cursor = align_to(cursor, BYTECODE_ALIGNMENT)?;
        debug_offsets_offset = Some(cursor);
        cursor = cursor.checked_add(DEBUG_OFFSETS_SIZE)?;
        debug_offsets_end_offset = Some(cursor);
    }

    if cursor > max_end_offset {
        return None;
    }

    Some(ParsedFunctionInfoPayload {
        large_header_end_offset,
        exception_table_offset,
        exception_table_end_offset,
        debug_offsets_offset,
        debug_offsets_end_offset,
        payload_end_offset: cursor,
    })
}

fn push_section(
    sections: &mut Vec<HermesSection>,
    cursor: &mut u32,
    kind: HermesSectionKind,
    len: u32,
    header: &HermesHeader,
    bytes: &[u8],
) -> Option<()> {
    let offset = align_to(*cursor, BYTECODE_ALIGNMENT)?;
    let end = offset.checked_add(len)?;

    if end > header.file_length || end as usize > bytes.len() {
        return None;
    }

    if len > 0 {
        sections.push(HermesSection { kind, offset, len });
    }

    *cursor = end;
    Some(())
}

fn align_to(value: u32, alignment: u32) -> Option<u32> {
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

fn multiply_u32(lhs: u32, rhs: u32) -> Option<u32> {
    lhs.checked_mul(rhs)
}

fn validate_artifact_bounds(bytes: &[u8], header: &HermesHeader) -> Option<()> {
    if header.file_length < HERMES_HEADER_LEN as u32 {
        return None;
    }

    if header.debug_info_offset > header.file_length {
        return None;
    }

    if header.file_length as usize > bytes.len() {
        return None;
    }

    Some(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u32_at_u32(bytes: &[u8], base_offset: u32, relative_offset: usize) -> Option<u32> {
    let offset = usize::try_from(base_offset)
        .ok()?
        .checked_add(relative_offset)?;
    read_u32(bytes, offset)
}

fn read_signed_leb128(bytes: &[u8], offset: &mut usize) -> Option<i64> {
    let mut result = 0i64;
    let mut shift = 0u32;

    loop {
        let byte = i64::from(*bytes.get(*offset)?);
        *offset = offset.checked_add(1)?;

        result |= (byte & 0x7f) << shift;
        shift = shift.checked_add(7)?;

        if (byte & 0x80) == 0 {
            if shift < 64 && (byte & 0x40) != 0 {
                result |= (!0i64) << shift;
            }
            return Some(result);
        }

        if shift >= 64 {
            return None;
        }
    }
}

fn read_signed_leb128_with_range(
    bytes: &[u8],
    offset: &mut usize,
) -> Option<(i64, std::ops::Range<usize>)> {
    let start = *offset;
    let value = read_signed_leb128(bytes, offset)?;
    Some((value, start..*offset))
}

fn to_absolute_u32_range(
    base_offset: u32,
    range: std::ops::Range<usize>,
) -> Option<std::ops::Range<u32>> {
    Some(
        base_offset.checked_add(u32::try_from(range.start).ok()?)?
            ..base_offset.checked_add(u32::try_from(range.end).ok()?)?,
    )
}

fn decode_large_header_offset(word1: u32, word2: u32) -> u32 {
    let low = word1 & OVERFLOWED_FUNCTION_HEADER_OFFSET_MASK;
    let high = (word2 >> SMALL_FUNCTION_NAME_SHIFT) & SMALL_FUNCTION_NAME_MASK;
    (high << 24) | low
}
