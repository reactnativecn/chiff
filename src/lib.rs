pub mod engine;
pub mod format;
pub mod hermes;
pub mod patch;

pub use engine::{select_engine, EngineKind};
pub use format::{detect_input_format, HermesForm, InputFormat};
pub use hermes::{
    can_use_structured_hermes, parse_artifact, parse_debug_info_layout, parse_function_layout,
    parse_header, parse_section_layout, supports_structured_hermes_version, HermesArtifact,
    HermesDebugDataStream, HermesDebugFileRegion, HermesDebugInfoHeader, HermesDebugInfoLayout,
    HermesFunction, HermesFunctionInfoBlock, HermesFunctionLayout, HermesHeader, HermesSection,
    HermesSectionKind, HermesSectionLayout, SUPPORTED_STRUCTURED_HERMES_VERSIONS,
};
pub use patch::{apply_patch, diff_bytes, Patch, PatchError, PatchOp, PatchStats};
