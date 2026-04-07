use chiff::{
    analyze_diff, analyze_directory_pair, assess_structured_hermes, build_hpatch_approximate_plan,
    build_hpatch_compatible_plan, detect_input_format, select_engine, select_engine_decision,
    CorpusEntryStatus, EngineDecision, HermesForm, HpatchCompatiblePlan,
    HpatchCoverSelectionPolicy, InputFormat, StructuredHermesSupport,
};
use napi::bindgen_prelude::{Buffer, Result};
use napi_derive::napi;
use std::path::Path;

#[napi(object)]
pub struct DetectFormatResult {
    pub kind: String,
    pub version: Option<u32>,
    pub form: Option<String>,
}

#[napi(object)]
pub struct DiffStatsResult {
    pub op_count: u32,
    pub copy_op_count: u32,
    pub insert_op_count: u32,
    pub copied_bytes: u32,
    pub inserted_bytes: u32,
}

#[napi(object)]
pub struct AnalyzeDiffResult {
    pub engine_kind: String,
    pub engine_reason: String,
    pub old_structured_hermes_support: String,
    pub new_structured_hermes_support: String,
    pub op_count: u32,
    pub copy_op_count: u32,
    pub insert_op_count: u32,
    pub copied_bytes: u32,
    pub inserted_bytes: u32,
}

#[napi(object)]
pub struct EngineDecisionResult {
    pub kind: String,
    pub reason: String,
}

#[napi(object)]
pub struct StructuredHermesSupportResult {
    pub status: String,
    pub version: Option<u32>,
    pub form: Option<String>,
}

#[napi(object)]
pub struct CountEntryResult {
    pub key: String,
    pub count: u32,
}

#[napi(object)]
pub struct CorpusSummaryResult {
    pub paired: u32,
    pub copy_ops: u32,
    pub insert_ops: u32,
    pub copied_bytes: u32,
    pub inserted_bytes: u32,
    pub reason_counts: Vec<CountEntryResult>,
    pub old_support_counts: Vec<CountEntryResult>,
    pub new_support_counts: Vec<CountEntryResult>,
}

#[napi(object)]
pub struct CorpusEntryResult {
    pub relative_path: String,
    pub status: String,
    pub old_format: String,
    pub new_format: String,
    pub engine_kind: String,
    pub engine_reason: String,
    pub old_structured_hermes_support: String,
    pub new_structured_hermes_support: String,
    pub op_count: u32,
    pub copy_op_count: u32,
    pub insert_op_count: u32,
    pub copied_bytes: u32,
    pub inserted_bytes: u32,
}

#[napi(object)]
pub struct DirectoryAnalysisResult {
    pub entries: Vec<CorpusEntryResult>,
    pub summary: CorpusSummaryResult,
}

#[napi(object)]
pub struct HpatchCoverResult {
    pub old_pos: String,
    pub new_pos: String,
    pub len: String,
}

#[napi(object)]
pub struct HpatchCompatiblePlanResult {
    pub output_mode: String,
    pub cover_policy: String,
    pub old_size: String,
    pub new_size: String,
    pub cover_count: u32,
    pub covered_bytes: String,
    pub uncovered_new_bytes: String,
    pub covers: Vec<HpatchCoverResult>,
}

fn engine_decision_result(decision: EngineDecision) -> EngineDecisionResult {
    EngineDecisionResult {
        kind: String::from(decision.kind.as_str()),
        reason: String::from(decision.reason.as_str()),
    }
}

fn structured_hermes_support_result(
    support: StructuredHermesSupport,
) -> StructuredHermesSupportResult {
    match support {
        StructuredHermesSupport::NotHermes => StructuredHermesSupportResult {
            status: String::from("not_hermes"),
            version: None,
            form: None,
        },
        StructuredHermesSupport::InvalidHeader => StructuredHermesSupportResult {
            status: String::from("invalid_header"),
            version: None,
            form: None,
        },
        StructuredHermesSupport::UnsupportedVersion { version, form } => {
            StructuredHermesSupportResult {
                status: String::from("unsupported_version"),
                version: Some(version),
                form: Some(String::from(match form {
                    HermesForm::Execution => "execution",
                    HermesForm::Delta => "delta",
                })),
            }
        }
        StructuredHermesSupport::Supported { version, form } => StructuredHermesSupportResult {
            status: String::from("supported"),
            version: Some(version),
            form: Some(String::from(match form {
                HermesForm::Execution => "execution",
                HermesForm::Delta => "delta",
            })),
        },
    }
}

fn build_analyze_diff_result(old_input: &[u8], new_input: &[u8]) -> AnalyzeDiffResult {
    let analysis = analyze_diff(old_input, new_input);
    AnalyzeDiffResult {
        engine_kind: String::from(analysis.engine_decision.kind.as_str()),
        engine_reason: String::from(analysis.engine_decision.reason.as_str()),
        old_structured_hermes_support: String::from(
            analysis.old_structured_hermes_support.as_str(),
        ),
        new_structured_hermes_support: String::from(
            analysis.new_structured_hermes_support.as_str(),
        ),
        op_count: analysis.stats.op_count as u32,
        copy_op_count: analysis.stats.copy_op_count as u32,
        insert_op_count: analysis.stats.insert_op_count as u32,
        copied_bytes: analysis.stats.copied_bytes as u32,
        inserted_bytes: analysis.stats.inserted_bytes as u32,
    }
}

fn hpatch_plan_result(
    plan: HpatchCompatiblePlan,
    policy: HpatchCoverSelectionPolicy,
) -> HpatchCompatiblePlanResult {
    let stats = plan.stats();

    HpatchCompatiblePlanResult {
        output_mode: String::from(plan.output_mode().as_str()),
        cover_policy: String::from(policy.as_str()),
        old_size: plan.old_size.to_string(),
        new_size: plan.new_size.to_string(),
        cover_count: stats.cover_count as u32,
        covered_bytes: stats.covered_bytes.to_string(),
        uncovered_new_bytes: stats.uncovered_new_bytes.to_string(),
        covers: plan
            .covers
            .into_iter()
            .map(|cover| HpatchCoverResult {
                old_pos: cover.old_pos.to_string(),
                new_pos: cover.new_pos.to_string(),
                len: cover.len.to_string(),
            })
            .collect(),
    }
}

fn input_format_name(format: InputFormat) -> String {
    match format {
        InputFormat::Binary => String::from("binary"),
        InputFormat::TextUtf8 => String::from("text_utf8"),
        InputFormat::HermesBytecode { version, form } => format!(
            "hermes_bytecode:{}@{}",
            match form {
                HermesForm::Execution => "execution",
                HermesForm::Delta => "delta",
            },
            version
        ),
    }
}

fn corpus_entry_status_name(status: CorpusEntryStatus) -> String {
    String::from(status.as_str())
}

fn count_entries(counts: impl IntoIterator<Item = (String, usize)>) -> Vec<CountEntryResult> {
    counts
        .into_iter()
        .map(|(key, count)| CountEntryResult {
            key,
            count: count as u32,
        })
        .collect()
}

#[napi]
pub fn detect_format(input: Buffer) -> Result<DetectFormatResult> {
    let result = match detect_input_format(&input) {
        InputFormat::Binary => DetectFormatResult {
            kind: String::from("binary"),
            version: None,
            form: None,
        },
        InputFormat::TextUtf8 => DetectFormatResult {
            kind: String::from("text_utf8"),
            version: None,
            form: None,
        },
        InputFormat::HermesBytecode { version, form } => DetectFormatResult {
            kind: String::from("hermes_bytecode"),
            version: Some(version),
            form: Some(String::from(match form {
                HermesForm::Execution => "execution",
                HermesForm::Delta => "delta",
            })),
        },
    };

    Ok(result)
}

#[napi]
pub fn diff_stats(old_input: Buffer, new_input: Buffer) -> Result<DiffStatsResult> {
    let stats = analyze_diff(&old_input, &new_input).stats;

    Ok(DiffStatsResult {
        op_count: stats.op_count as u32,
        copy_op_count: stats.copy_op_count as u32,
        insert_op_count: stats.insert_op_count as u32,
        copied_bytes: stats.copied_bytes as u32,
        inserted_bytes: stats.inserted_bytes as u32,
    })
}

#[napi]
pub fn analyze_diff_result(old_input: Buffer, new_input: Buffer) -> Result<AnalyzeDiffResult> {
    Ok(build_analyze_diff_result(&old_input, &new_input))
}

#[napi]
pub fn hpatch_compatible_plan_result(
    old_input: Buffer,
    new_input: Buffer,
) -> Result<HpatchCompatiblePlanResult> {
    let analysis = analyze_diff(&old_input, &new_input);
    let plan = build_hpatch_compatible_plan(old_input.len(), &analysis.patch)
        .map_err(|error| napi::Error::from_reason(format!("{error:?}")))?;

    Ok(hpatch_plan_result(
        plan,
        HpatchCoverSelectionPolicy::ChiffStructured,
    ))
}

#[napi]
pub fn hpatch_approximate_plan_result(
    old_input: Buffer,
    new_input: Buffer,
) -> Result<HpatchCompatiblePlanResult> {
    Ok(hpatch_plan_result(
        build_hpatch_approximate_plan(&old_input, &new_input),
        HpatchCoverSelectionPolicy::ChiffApproximate,
    ))
}

#[napi]
pub fn analyze_directory_pair_result(
    old_root: String,
    new_root: String,
) -> Result<DirectoryAnalysisResult> {
    let corpus = analyze_directory_pair(Path::new(&old_root), Path::new(&new_root))
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;

    let entries = corpus
        .entries
        .into_iter()
        .map(|entry| {
            let (
                engine_kind,
                engine_reason,
                old_support,
                new_support,
                op_count,
                copy_op_count,
                insert_op_count,
                copied_bytes,
                inserted_bytes,
            ) = match entry.diff_analysis {
                Some(analysis) => (
                    String::from(analysis.engine_decision.kind.as_str()),
                    String::from(analysis.engine_decision.reason.as_str()),
                    String::from(analysis.old_structured_hermes_support.as_str()),
                    String::from(analysis.new_structured_hermes_support.as_str()),
                    analysis.stats.op_count as u32,
                    analysis.stats.copy_op_count as u32,
                    analysis.stats.insert_op_count as u32,
                    analysis.stats.copied_bytes as u32,
                    analysis.stats.inserted_bytes as u32,
                ),
                None => (
                    String::from("-"),
                    String::from("-"),
                    String::from("-"),
                    String::from("-"),
                    0,
                    0,
                    0,
                    0,
                    0,
                ),
            };

            CorpusEntryResult {
                relative_path: entry.relative_path.display().to_string(),
                status: corpus_entry_status_name(entry.status),
                old_format: entry
                    .old_format
                    .map(input_format_name)
                    .unwrap_or_else(|| String::from("-")),
                new_format: entry
                    .new_format
                    .map(input_format_name)
                    .unwrap_or_else(|| String::from("-")),
                engine_kind,
                engine_reason,
                old_structured_hermes_support: old_support,
                new_structured_hermes_support: new_support,
                op_count,
                copy_op_count,
                insert_op_count,
                copied_bytes,
                inserted_bytes,
            }
        })
        .collect();

    Ok(DirectoryAnalysisResult {
        entries,
        summary: CorpusSummaryResult {
            paired: corpus.summary.paired as u32,
            copy_ops: corpus.summary.copy_ops as u32,
            insert_ops: corpus.summary.insert_ops as u32,
            copied_bytes: corpus.summary.copied_bytes as u32,
            inserted_bytes: corpus.summary.inserted_bytes as u32,
            reason_counts: count_entries(corpus.summary.reason_counts),
            old_support_counts: count_entries(corpus.summary.old_support_counts),
            new_support_counts: count_entries(corpus.summary.new_support_counts),
        },
    })
}

#[napi]
pub fn select_engine_name(old_input: Buffer, new_input: Buffer) -> Result<String> {
    Ok(String::from(select_engine(&old_input, &new_input).as_str()))
}

#[napi]
pub fn structured_hermes_compatible(input: Buffer) -> Result<bool> {
    Ok(assess_structured_hermes(&input).is_supported())
}

#[napi]
pub fn select_engine_decision_result(
    old_input: Buffer,
    new_input: Buffer,
) -> Result<EngineDecisionResult> {
    Ok(engine_decision_result(select_engine_decision(
        &old_input, &new_input,
    )))
}

#[napi]
pub fn structured_hermes_support(input: Buffer) -> Result<StructuredHermesSupportResult> {
    Ok(structured_hermes_support_result(assess_structured_hermes(
        &input,
    )))
}
