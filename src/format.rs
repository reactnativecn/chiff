#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HermesForm {
    Execution,
    Delta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    Binary,
    TextUtf8,
    HermesBytecode { version: u32, form: HermesForm },
}

const HERMES_MAGIC: u64 = 0x1F19_03C1_03BC_1FC6;
const HERMES_DELTA_MAGIC: u64 = !HERMES_MAGIC;

pub fn detect_input_format(bytes: &[u8]) -> InputFormat {
    if let Some(format) = detect_hermes_bytecode(bytes) {
        return format;
    }
    if is_probably_utf8_text(bytes) {
        return InputFormat::TextUtf8;
    }
    InputFormat::Binary
}

fn detect_hermes_bytecode(bytes: &[u8]) -> Option<InputFormat> {
    if bytes.len() < 12 {
        return None;
    }

    let magic = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
    let form = match magic {
        HERMES_MAGIC => HermesForm::Execution,
        HERMES_DELTA_MAGIC => HermesForm::Delta,
        _ => return None,
    };

    let version = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    Some(InputFormat::HermesBytecode { version, form })
}

fn is_probably_utf8_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.contains(&0) {
        return false;
    }

    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return false,
    };

    let suspicious_controls = text
        .chars()
        .filter(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
        .count();

    suspicious_controls * 32 <= text.chars().count().max(1)
}
