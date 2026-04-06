pub mod corpus;
pub mod engine;
pub mod format;
pub mod hermes;
pub mod patch;

pub use corpus::{
    analyze_directory_pair, CorpusAnalysis, CorpusEntryAnalysis, CorpusEntryStatus, CorpusSummary,
};
pub use engine::{select_engine, select_engine_decision, EngineDecision, EngineKind, EngineReason};
pub use format::{detect_input_format, HermesForm, InputFormat};
pub use hermes::{
    assess_structured_hermes, can_use_structured_hermes, parse_artifact, parse_debug_info_layout,
    parse_function_layout, parse_header, parse_section_layout, supports_structured_hermes_version,
    HermesArtifact, HermesDebugDataStream, HermesDebugFileRegion, HermesDebugInfoHeader,
    HermesDebugInfoLayout, HermesFunction, HermesFunctionInfoBlock, HermesFunctionLayout,
    HermesHeader, HermesSection, HermesSectionKind, HermesSectionLayout, StructuredHermesSupport,
    SUPPORTED_STRUCTURED_HERMES_VERSIONS,
};
pub use patch::{
    analyze_diff, apply_patch, diff_bytes, DiffAnalysis, Patch, PatchError, PatchOp, PatchStats,
};
