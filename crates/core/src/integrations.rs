use std::time::Duration;

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::{OllamaReport, PulseFlowReport, Suggestion};
use crate::security::{redact, validate_http_endpoint};

pub fn test_ollama(enabled: bool, endpoint: &str, model: &str) -> OllamaReport {
    if !enabled {
        return OllamaReport {
            enabled: false,
            reachable: false,
            endpoint: endpoint.to_string(),
            models: Vec::new(),
            configured_model_present: None,
            message: "Local AI is off. Search and launch do not use a model.".into(),
        };
    }
    let endpoint = match validate_http_endpoint(endpoint) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            return OllamaReport {
                enabled: true,
                reachable: false,
                endpoint: endpoint.to_string(),
                models: Vec::new(),
                configured_model_present: None,
                message: err.to_string(),
            }
        }
    };
    match ureq::get(&format!("{endpoint}/api/tags"))
        .timeout(Duration::from_secs(3))
        .call()
    {
        Ok(response) => {
            let body = response.into_string().unwrap_or_default();
            let parsed: Result<TagsResponse, _> = serde_json::from_str(&body);
            let models: Vec<String> = parsed
                .map(|tags| tags.models.into_iter().map(|model| model.name).collect())
                .unwrap_or_default();
            let configured = if model.trim().is_empty() {
                None
            } else {
                Some(models.iter().any(|name| name == model))
            };
            let message = match configured {
                Some(false) => "Ollama responded, but the configured model is not installed.".into(),
                Some(true) => "Ollama is reachable and the configured model is installed. Suggestions still require approval.".into(),
                None => "Ollama is reachable. Set a model before asking for a suggestion.".into(),
            };
            OllamaReport {
                enabled: true,
                reachable: true,
                endpoint,
                models,
                configured_model_present: configured,
                message,
            }
        }
        Err(err) => OllamaReport {
            enabled: true,
            reachable: false,
            endpoint,
            models: Vec::new(),
            configured_model_present: None,
            message: format!("Ollama is not reachable: {err}"),
        },
    }
}

pub fn suggest_with_ollama(endpoint: &str, model: &str, prompt: &str) -> AppResult<Suggestion> {
    let endpoint = validate_http_endpoint(endpoint)?;
    let model = model.trim();
    if model.is_empty() {
        return Err(AppError::new(
            "OLLAMA",
            "Set an Ollama model before asking.",
        ));
    }
    if prompt.trim().is_empty() || prompt.len() > 4000 {
        return Err(AppError::new(
            "OLLAMA",
            "Prompt must be between 1 and 4000 characters.",
        ));
    }
    let response = ureq::post(&format!("{endpoint}/api/generate"))
        .timeout(Duration::from_secs(30))
        .send_json(serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        }))
        .map_err(|err| AppError::new("OLLAMA", format!("Ollama request failed: {err}")))?;
    let body = response
        .into_string()
        .map_err(|err| AppError::new("OLLAMA", err.to_string()))?;
    let parsed: GenerateResponse = serde_json::from_str(&body)
        .map_err(|_| AppError::new("OLLAMA", "Ollama returned an unexpected response."))?;
    let mut text = redact(&parsed.response);
    if text.len() > 8000 {
        text.truncate(8000);
    }
    Ok(Suggestion {
        text,
        executed: false,
        model: model.to_string(),
        message: "Suggestion only. PulseLauncher did not run this text.".into(),
    })
}

pub fn test_pulseflow(executable: &str, endpoint: &str) -> PulseFlowReport {
    if executable.trim().is_empty() && endpoint.trim().is_empty() {
        return PulseFlowReport {
            configured: false,
            executable: String::new(),
            endpoint: String::new(),
            reachable: None,
            http_status: None,
            message: "PulseFlow is not configured. Register its executable or set a localhost endpoint that the installed PulseFlow build actually exposes.".into(),
        };
    }
    let mut report = PulseFlowReport {
        configured: true,
        executable: executable.trim().to_string(),
        endpoint: endpoint.trim().to_string(),
        reachable: None,
        http_status: None,
        message: String::new(),
    };
    if !report.executable.is_empty() && !std::path::Path::new(&report.executable).is_file() {
        report.message = "The PulseFlow executable path does not exist.".into();
        return report;
    }
    if report.endpoint.is_empty() {
        report.message = "PulseFlow executable is set. No API endpoint is configured, so PulseLauncher will only launch the program.".into();
        return report;
    }
    let endpoint = match validate_http_endpoint(&report.endpoint) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            report.message = err.to_string();
            report.reachable = Some(false);
            return report;
        }
    };
    report.endpoint = endpoint.clone();
    match ureq::get(&endpoint).timeout(Duration::from_secs(3)).call() {
        Ok(response) => {
            let status = response.status();
            report.reachable = Some(true);
            report.http_status = Some(status);
            report.message = format!(
                "Endpoint responded with HTTP {status}. PulseLauncher did not start a PulseFlow workflow."
            );
        }
        Err(ureq::Error::Status(status, _)) => {
            report.reachable = Some(true);
            report.http_status = Some(status);
            report.message = format!(
                "Endpoint responded with HTTP {status}. PulseLauncher did not start a PulseFlow workflow."
            );
        }
        Err(err) => {
            report.reachable = Some(false);
            report.message = format!("PulseFlow endpoint is not reachable: {err}");
        }
    }
    report
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
}

#[derive(Deserialize)]
struct GenerateResponse {
    #[serde(default)]
    response: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn serve(body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = body.to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let payload = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: application/json\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(payload.as_bytes());
        });
        format!("http://127.0.0.1:{port}")
    }

    #[test]
    fn ollama_disabled_and_live_probe_do_not_execute_commands() {
        let disabled = test_ollama(false, "http://127.0.0.1:9", "");
        assert!(!disabled.reachable);
        let endpoint = serve(r#"{"models":[{"name":"llama3.2:latest"}]}"#);
        let report = test_ollama(true, &endpoint, "missing");
        assert!(report.reachable);
        assert_eq!(report.configured_model_present, Some(false));
        assert!(
            test_ollama(true, "file:///tmp/x", "")
                .message
                .contains("Scheme")
                || !test_ollama(true, "file:///tmp/x", "").reachable
        );
    }

    #[test]
    fn pulseflow_without_config_does_not_invent_an_endpoint() {
        let report = test_pulseflow("", "");
        assert!(!report.configured);
        assert!(report.endpoint.is_empty());
        assert!(report.message.contains("not configured"));
    }
}
