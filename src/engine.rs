use crate::format::{detect_input_format, InputFormat};
use crate::hermes::can_use_structured_hermes;

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
                form: old_form,
                ..
            },
            InputFormat::HermesBytecode {
                version: new_version,
                form: new_form,
                ..
            },
        ) => {
            if old_version == new_version
                && old_form == new_form
                && can_use_structured_hermes(old)
                && can_use_structured_hermes(new)
            {
                EngineKind::Hermes
            } else {
                EngineKind::GenericBinary
            }
        }
        _ => EngineKind::GenericBinary,
    }
}
