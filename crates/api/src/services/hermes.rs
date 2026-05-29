use std::fs;
use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

fn err<E: std::fmt::Display>(status: StatusCode, e: E) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": e.to_string() })))
}

#[derive(Deserialize)]
pub struct PatchChecksumRequest {
    pub checksum: String,
}

pub async fn patch_wasm_checksum(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PatchChecksumRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let checksum = req.checksum.trim().to_ascii_lowercase();
    if checksum.len() != 64 || !checksum.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "checksum must be 64 lowercase hex characters",
        ));
    }

    let path = state.hermes_config_path.as_str();
    if !Path::new(path).exists() {
        return Err(err(
            StatusCode::NOT_FOUND,
            format!("hermes config not found at {path}"),
        ));
    }

    let text = fs::read_to_string(path)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("read {path}: {e}")))?;

    let re = Regex::new(r"wasm_checksum_hex\s*=\s*'[^']*'")
        .expect("static regex");

    let previous = re
        .find(&text)
        .map(|m| m.as_str().to_string());

    let new_line = format!("wasm_checksum_hex = '{checksum}'");
    let new_text = re.replacen(&text, 1, new_line.as_str()).to_string();

    if new_text == text {
        return Err(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "wasm_checksum_hex line not found in hermes config",
        ));
    }

    fs::write(path, new_text)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("write {path}: {e}")))?;

    Ok(Json(json!({
        "patched": true,
        "path": path,
        "checksum": checksum,
        "previous": previous,
    })))
}
