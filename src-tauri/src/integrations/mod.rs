//! Optional local integrations. Neither is required for the launcher, and
//! neither is allowed to run an action by itself.

use serde::Serialize;
use url::Url;

use crate::error::{AppError, AppResult};
use crate::security;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaStatus {
    pub enabled: bool,
    pub endpoint: String,
    pub reachable: bool,
    pub models: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub text: String,
    pub executed: bool,
    pub notice: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub configured: bool,
    pub reachable: bool,
    pub message: String,
}

fn http_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(4)))
        .build();
    config.into()
}

pub fn validate_endpoint(raw: &str) -> AppResult<Url> {
    let url = security::validate_web_url(raw.trim().trim_end_matches('/'))?;
    Ok(url)
}

pub fn ollama_status(enabled: bool, endpoint: &str) -> OllamaStatus {
    if !enabled {
        return OllamaStatus {
            enabled: false,
            endpoint: endpoint.to_string(),
            reachable: false,
            models: vec![],
            message: "Local AI is off. Search and launching do not use it.".into(),
        };
    }
    let url = match validate_endpoint(endpoint) {
        Ok(u) => u,
        Err(e) => {
            return OllamaStatus {
                enabled: true,
                endpoint: endpoint.to_string(),
                reachable: false,
                models: vec![],
                message: e.message,
            }
        }
    };
    let tags = format!("{}/api/tags", url.as_str().trim_end_matches('/'));
    match http_agent().get(&tags).call() {
        Ok(mut response) => {
            let body: serde_json::Value = response
                .body_mut()
                .read_json()
                .unwrap_or(serde_json::Value::Null);
            let models = body
                .get("models")
                .and_then(|m| m.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(str::to_owned))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let message = if models.is_empty() {
                "Connected, but no model is installed in Ollama. PulseLauncher will not download one.".into()
            } else {
                format!("Connected. {} model(s) already installed.", models.len())
            };
            OllamaStatus {
                enabled: true,
                endpoint: url.to_string(),
                reachable: true,
                models,
                message,
            }
        }
        Err(e) => OllamaStatus {
            enabled: true,
            endpoint: url.to_string(),
            reachable: false,
            models: vec![],
            message: format!("Could not reach Ollama: {e}. The launcher keeps working without it."),
        },
    }
}

/// Ask a local model for an explanation. The returned text is never executed.
pub fn ollama_suggest(endpoint: &str, model: &str, prompt: &str) -> AppResult<Suggestion> {
    let url = validate_endpoint(endpoint)?;
    let model = model.trim();
    if model.is_empty() {
        return Err(AppError::invalid("Choose an installed model first. The connection test lists models already on this computer."));
    }
    if model.len() > 120 || model.chars().any(|c| c.is_control()) {
        return Err(AppError::invalid("Model name is invalid"));
    }
    let prompt = prompt.trim();
    if prompt.is_empty() || prompt.chars().count() > 2000 {
        return Err(AppError::invalid(
            "Enter a question of at most 2000 characters",
        ));
    }
    let chat = format!("{}/api/chat", url.as_str().trim_end_matches('/'));
    let payload = serde_json::json!({
        "model": model,
        "stream": false,
        "messages": [{
            "role": "user",
            "content": format!(
                "You assist a person using a local application launcher. Explain the request or propose a command. Do not claim that anything was executed. Request: {prompt}"
            )
        }]
    });
    let mut response = http_agent().post(&chat).send_json(payload).map_err(|e| {
        AppError::new(
            crate::error::ErrorCode::Integration,
            format!("Ollama request failed: {e}"),
        )
    })?;
    let body: serde_json::Value = response.body_mut().read_json().map_err(|e| {
        AppError::new(
            crate::error::ErrorCode::Integration,
            format!("Ollama returned an unreadable response: {e}"),
        )
    })?;
    let text = body
        .pointer("/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(8000)
        .collect::<String>();
    if text.trim().is_empty() {
        return Err(AppError::new(
            crate::error::ErrorCode::Integration,
            "Ollama responded without any text.",
        ));
    }
    Ok(Suggestion {
        text,
        executed: false,
        notice: "This is a suggestion from your local model. PulseLauncher did not run it.".into(),
    })
}

fn loopback(url: &Url) -> bool {
    matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    )
}

/// Report whether a user-configured PulseFlow address answers. This does not
/// start a PulseFlow workflow and does not invent an API.
pub fn pulseflow_probe(raw: &str) -> Probe {
    let raw = raw.trim();
    if raw.is_empty() {
        return Probe {
            configured: false,
            reachable: false,
            message: "No PulseFlow address is configured. Launch PulseFlow like any other registered program. Set an address here only if your PulseFlow build documents a local endpoint.".into(),
        };
    }
    let url = match security::validate_web_url(raw) {
        Ok(u) => u,
        Err(e) => {
            return Probe {
                configured: true,
                reachable: false,
                message: e.message,
            }
        }
    };
    if !loopback(&url) {
        return Probe {
            configured: true,
            reachable: false,
            message:
                "PulseFlow integration only contacts a loopback address (localhost or 127.0.0.1)."
                    .into(),
        };
    }
    match http_agent().get(url.as_str()).call() {
        Ok(response) => Probe {
            configured: true,
            reachable: true,
            message: format!(
                "The address responded with HTTP {}. PulseLauncher did not start a workflow; this only checks that something is listening.",
                response.status()
            ),
        },
        Err(e) => Probe {
            configured: true,
            reachable: false,
            message: format!("Nothing responded at that address: {e}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_ollama_makes_no_request() {
        let status = ollama_status(false, "http://127.0.0.1:1");
        assert!(!status.reachable);
        assert!(status.message.contains("off"));
    }

    #[test]
    fn rejects_remote_pulseflow_and_bad_endpoints() {
        let probe = pulseflow_probe("https://example.com/api");
        assert!(!probe.reachable);
        assert!(probe.message.contains("loopback"));
        assert!(pulseflow_probe("").message.contains("No PulseFlow"));
        assert!(validate_endpoint("javascript:alert(1)").is_err());
        assert!(ollama_suggest("http://127.0.0.1:11434", "", "hi").is_err());
    }
}
