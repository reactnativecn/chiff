use chiff::{detect_input_format, HermesForm, InputFormat};
use napi::bindgen_prelude::{Buffer, Result};
use napi_derive::napi;

#[napi(object)]
pub struct DetectFormatResult {
    pub kind: String,
    pub version: Option<u32>,
    pub form: Option<String>,
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
