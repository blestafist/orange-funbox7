use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;
use serde_json::{Value, json};

const API_CONTENT_TYPE: &str = "application/x-sah-ws-4-call+json";
const APP_NAME: &str = "funboxctl";

pub struct RouterClient {
    http: Client,
    endpoint: String,
    referer: String,
    context_id: Option<String>,
}

impl RouterClient {
    fn new(host: &str) -> Result<Self> {
        let host = host.trim();
        if host.is_empty()
            || host.contains('/')
            || host.contains('@')
            || host.contains('#')
            || host.contains('?')
        {
            bail!("Некорректный адрес роутера: укажи только имя хоста или IP, без URL и пути");
        }

        let base = format!("http://{host}");
        let http = Client::builder()
            // The web UI issues a session cookie on login. Some services return
            // Permission denied without it, even when the contextID is valid.
            .cookie_store(true)
            .user_agent("funbox7-cli/0.1")
            .timeout(Duration::from_secs(12))
            .build()
            .context("не удалось создать HTTP-клиент")?;

        Ok(Self {
            http,
            endpoint: format!("{base}/ws"),
            referer: format!("{base}/"),
            context_id: None,
        })
    }

    fn request(&self, body: &Value, authorization: &str) -> Result<Value> {
        let response = self
            .http
            .post(&self.endpoint)
            .header(reqwest::header::CONTENT_TYPE, API_CONTENT_TYPE)
            .header(reqwest::header::ACCEPT, "*/*")
            .header(reqwest::header::REFERER, &self.referer)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .body(body.to_string())
            .send()
            .with_context(|| format!("не удалось подключиться к {}", self.endpoint))?;

        let status = response.status();
        let text = response
            .text()
            .context("не удалось прочитать ответ роутера")?;
        if !status.is_success() {
            bail!("роутер вернул HTTP {status}: {}", short_body(&text));
        }

        serde_json::from_str(&text)
            .with_context(|| format!("роутер вернул не-JSON ответ: {}", short_body(&text)))
    }

    pub fn login(host: &str, username: &str, password: &str) -> Result<Self> {
        let mut client = Self::new(host)?;
        let body = json!({
            "service": "sah.Device.Information",
            "method": "createContext",
            "parameters": {
                "applicationName": APP_NAME,
                "username": username,
                "password": password
            }
        });
        let response = client.request(&body, "X-Sah-Login")?;
        check_api_errors(&response, "вход")?;

        let context_id = response
            .get("data")
            .and_then(|data| data.get("contextID"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("роутер не выдал сессию; проверь логин и пароль"))?;
        client.context_id = Some(context_id.to_owned());
        Ok(client)
    }

    pub fn call(&self, service: &str, method: &str, parameters: Value) -> Result<Value> {
        let context_id = self
            .context_id
            .as_deref()
            .ok_or_else(|| anyhow!("нет активной сессии"))?;
        let body = json!({
            "service": service,
            "method": method,
            "parameters": parameters
        });
        let authorization = format!("X-Sah {context_id}");
        let response = self.request(&body, &authorization)?;
        check_api_errors(&response, &format!("{service}.{method}"))?;
        if let Some(status) = response.get("status").filter(|value| !value.is_null()) {
            return Ok(status.clone());
        }
        if let Some(data) = response.get("data") {
            return Ok(data.clone());
        }
        response
            .get("status")
            .cloned()
            .ok_or_else(|| anyhow!("в ответе {service}.{method} нет поля status/data"))
    }

    pub fn logout(&mut self) {
        let Some(context_id) = self.context_id.take() else {
            return;
        };
        let body = json!({
            "service": "sah.Device.Information",
            "method": "releaseContext",
            "parameters": { "applicationName": APP_NAME }
        });
        let authorization = format!("X-Sah-Logout {context_id}");
        // Releasing the session is best-effort and must not mask command errors.
        let _ = self.request(&body, &authorization);
    }
}

fn check_api_errors(response: &Value, operation: &str) -> Result<()> {
    let Some(errors) = response.get("errors").and_then(Value::as_array) else {
        return Ok(());
    };
    if errors.is_empty() {
        return Ok(());
    }

    let details = errors
        .iter()
        .map(|error| {
            let description = error
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("ошибка API");
            let info = error
                .get("info")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if info.is_empty() {
                description.to_owned()
            } else {
                format!("{description} ({info})")
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    bail!("ошибка роутера при {operation}: {details}");
}

fn short_body(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut result = compact.chars().take(240).collect::<String>();
    if compact.chars().count() > 240 {
        result.push('…');
    }
    result
}
