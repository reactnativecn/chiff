use crate::format::{detect_input_format, HermesForm, InputFormat};

const HERMES_HEADER_LEN: usize = 128;
const FILE_LENGTH_OFFSET: usize = 32;
const FUNCTION_COUNT_OFFSET: usize = 40;
const STRING_COUNT_OFFSET: usize = 52;
const DEBUG_INFO_OFFSET_OFFSET: usize = 108;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesHeader {
    pub version: u32,
    pub form: HermesForm,
    pub file_length: u32,
    pub function_count: u32,
    pub string_count: u32,
    pub debug_info_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HermesArtifact {
    pub header: HermesHeader,
    pub payload_len: usize,
}

pub fn parse_artifact(bytes: &[u8]) -> Option<HermesArtifact> {
    let header = parse_header(bytes)?;

    Some(HermesArtifact {
        header,
        payload_len: bytes.len(),
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
            function_count: read_u32(bytes, FUNCTION_COUNT_OFFSET)?,
            string_count: read_u32(bytes, STRING_COUNT_OFFSET)?,
            debug_info_offset: read_u32(bytes, DEBUG_INFO_OFFSET_OFFSET)?,
        }),
        _ => None,
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}
