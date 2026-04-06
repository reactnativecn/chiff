use crate::format::{detect_input_format, HermesForm, InputFormat};

const HERMES_HEADER_LEN: usize = 128;
const BYTECODE_ALIGNMENT: u32 = 4;
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
const STRING_KIND_ENTRY_SIZE: u32 = 4;
const IDENTIFIER_HASH_SIZE: u32 = 4;
const SMALL_STRING_TABLE_ENTRY_SIZE: u32 = 4;
const OVERFLOW_STRING_TABLE_ENTRY_SIZE: u32 = 8;
const OBJ_SHAPE_TABLE_ENTRY_SIZE: u32 = 8;
const BIG_INT_TABLE_ENTRY_SIZE: u32 = 8;
const REG_EXP_TABLE_ENTRY_SIZE: u32 = 8;
const U32_PAIR_SIZE: u32 = 8;

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
pub struct HermesArtifact {
    pub header: HermesHeader,
    pub payload_len: usize,
    pub section_layout: HermesSectionLayout,
}

pub fn parse_artifact(bytes: &[u8]) -> Option<HermesArtifact> {
    let header = parse_header(bytes)?;
    let section_layout = parse_section_layout_from_header(bytes, &header)?;

    Some(HermesArtifact {
        header,
        payload_len: bytes.len(),
        section_layout,
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
