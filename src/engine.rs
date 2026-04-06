use crate::format::{detect_input_format, InputFormat};
use crate::hermes::assess_structured_hermes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    GenericBinary,
    Text,
    Hermes,
}

impl EngineKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GenericBinary => "generic_binary",
            Self::Text => "text",
            Self::Hermes => "hermes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineReason {
    TextPair,
    HermesStructured,
    HermesVersionMismatch,
    HermesFormMismatch,
    HermesOldInvalidHeader,
    HermesOldUnsupportedVersion,
    HermesNewInvalidHeader,
    HermesNewUnsupportedVersion,
    MixedFormats,
}

impl EngineReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TextPair => "text_pair",
            Self::HermesStructured => "hermes_structured",
            Self::HermesVersionMismatch => "hermes_version_mismatch",
            Self::HermesFormMismatch => "hermes_form_mismatch",
            Self::HermesOldInvalidHeader => "hermes_old_invalid_header",
            Self::HermesOldUnsupportedVersion => "hermes_old_unsupported_version",
            Self::HermesNewInvalidHeader => "hermes_new_invalid_header",
            Self::HermesNewUnsupportedVersion => "hermes_new_unsupported_version",
            Self::MixedFormats => "mixed_formats",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineDecision {
    pub kind: EngineKind,
    pub reason: EngineReason,
}

pub fn select_engine(old: &[u8], new: &[u8]) -> EngineKind {
    select_engine_decision(old, new).kind
}

pub fn select_engine_decision(old: &[u8], new: &[u8]) -> EngineDecision {
    match (detect_input_format(old), detect_input_format(new)) {
        (InputFormat::TextUtf8, InputFormat::TextUtf8) => EngineDecision {
            kind: EngineKind::Text,
            reason: EngineReason::TextPair,
        },
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
            if old_version != new_version {
                return EngineDecision {
                    kind: EngineKind::GenericBinary,
                    reason: EngineReason::HermesVersionMismatch,
                };
            }

            if old_form != new_form {
                return EngineDecision {
                    kind: EngineKind::GenericBinary,
                    reason: EngineReason::HermesFormMismatch,
                };
            }

            match (assess_structured_hermes(old), assess_structured_hermes(new)) {
                (
                    crate::StructuredHermesSupport::Supported { .. },
                    crate::StructuredHermesSupport::Supported { .. },
                ) => EngineDecision {
                    kind: EngineKind::Hermes,
                    reason: EngineReason::HermesStructured,
                },
                (crate::StructuredHermesSupport::InvalidHeader, _) => EngineDecision {
                    kind: EngineKind::GenericBinary,
                    reason: EngineReason::HermesOldInvalidHeader,
                },
                (crate::StructuredHermesSupport::UnsupportedVersion { .. }, _) => EngineDecision {
                    kind: EngineKind::GenericBinary,
                    reason: EngineReason::HermesOldUnsupportedVersion,
                },
                (_, crate::StructuredHermesSupport::InvalidHeader) => EngineDecision {
                    kind: EngineKind::GenericBinary,
                    reason: EngineReason::HermesNewInvalidHeader,
                },
                (_, crate::StructuredHermesSupport::UnsupportedVersion { .. }) => EngineDecision {
                    kind: EngineKind::GenericBinary,
                    reason: EngineReason::HermesNewUnsupportedVersion,
                },
                _ => EngineDecision {
                    kind: EngineKind::GenericBinary,
                    reason: EngineReason::MixedFormats,
                },
            }
        }
        _ => EngineDecision {
            kind: EngineKind::GenericBinary,
            reason: EngineReason::MixedFormats,
        },
    }
}
