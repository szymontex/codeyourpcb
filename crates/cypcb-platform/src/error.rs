use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Dialog error: {0}")]
    Dialog(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Operation not supported: {0}")]
    NotSupported(String),

    #[error("Web API error: {0}")]
    Web(String),
}

#[cfg(wasm)]
impl From<wasm_bindgen::JsValue> for PlatformError {
    fn from(value: wasm_bindgen::JsValue) -> Self {
        PlatformError::Web(format!("{:?}", value))
    }
}
