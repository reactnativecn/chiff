use chiff::{
    assess_structured_hermes, detect_input_format, diff_bytes, select_engine,
    select_engine_decision, EngineDecision, HermesForm, InputFormat, StructuredHermesSupport,
};
use napi::bindgen_prelude::{Buffer, Result};
use napi_derive::napi;

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
    let stats = diff_bytes(&old_input, &new_input).stats();

    Ok(DiffStatsResult {
        op_count: stats.op_count as u32,
        copy_op_count: stats.copy_op_count as u32,
        insert_op_count: stats.insert_op_count as u32,
        copied_bytes: stats.copied_bytes as u32,
        inserted_bytes: stats.inserted_bytes as u32,
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
