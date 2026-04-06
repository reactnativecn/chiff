use chiff::{
    can_use_structured_hermes, detect_input_format, diff_bytes, select_engine, EngineKind,
    HermesForm, InputFormat,
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

fn engine_kind_name(kind: EngineKind) -> String {
    String::from(match kind {
        EngineKind::GenericBinary => "generic_binary",
        EngineKind::Text => "text",
        EngineKind::Hermes => "hermes",
    })
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
    Ok(engine_kind_name(select_engine(&old_input, &new_input)))
}

#[napi]
pub fn structured_hermes_compatible(input: Buffer) -> Result<bool> {
    Ok(can_use_structured_hermes(&input))
}
