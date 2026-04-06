pub mod engine;
pub mod format;
pub mod hermes;
pub mod patch;

pub use engine::{select_engine, EngineKind};
pub use format::{detect_input_format, HermesForm, InputFormat};
pub use hermes::{
    parse_artifact, parse_function_layout, parse_header, parse_section_layout, HermesArtifact,
    HermesFunction, HermesFunctionInfoBlock, HermesFunctionLayout, HermesHeader, HermesSection,
    HermesSectionKind, HermesSectionLayout,
};
pub use patch::{apply_patch, diff_bytes, Patch, PatchError, PatchOp, PatchStats};
