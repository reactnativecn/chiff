use crate::format::{detect_input_format, InputFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    GenericBinary,
    Text,
    Hermes,
}

pub fn select_engine(old: &[u8], new: &[u8]) -> EngineKind {
    match (detect_input_format(old), detect_input_format(new)) {
        (InputFormat::TextUtf8, InputFormat::TextUtf8) => EngineKind::Text,
        (
            InputFormat::HermesBytecode {
                version: old_version,
                ..
            },
            InputFormat::HermesBytecode {
                version: new_version,
                ..
            },
        ) => {
            if old_version == new_version {
                EngineKind::Hermes
            } else {
                EngineKind::GenericBinary
            }
        }
        _ => EngineKind::GenericBinary,
    }
}
