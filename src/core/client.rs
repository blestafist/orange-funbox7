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
            bail!(
                "Invalid router address: provide only a hostname or IP, without a URL scheme or path"
            );
        }

        let base = format!("http://{host}");
        let http = Client::builder()
            // The web UI issues a session cookie on login. Some services return
            // Permission denied without it, even when the contextID is valid.
            .cookie_store(true)
            .user_agent("funbox7-cli/0.1")
            .timeout(Duration::from_secs(12))
            .build()
            .context("failed to create HTTP client")?;

        Ok(Self {
            http,
            endpoint: format!("{base}/ws"),
            referer: format!("{base}/"),
            context_id: None,
        })
    }

    fn request(&self, body: &Value, authorization: &str) -> Result<Value> {
        self.request_with_privacy(body, authorization, false)
    }

    fn request_sensitive(&self, body: &Value, authorization: &str) -> Result<Value> {
        self.request_with_privacy(body, authorization, true)
    }

    fn request_with_privacy(
        &self,
        body: &Value,
        authorization: &str,
        sensitive: bool,
    ) -> Result<Value> {
        self.post_ws(body, Some(authorization), None, sensitive)
    }

    /// Posts one `/ws` call. The web UI sends `X-Context` on authenticated
    /// calls and makes anonymous calls (no `Authorization`) before login.
    fn post_ws(
        &self,
        body: &Value,
        authorization: Option<&str>,
        x_context: Option<&str>,
        sensitive: bool,
    ) -> Result<Value> {
        let mut request = self
            .http
            .post(&self.endpoint)
            .header(reqwest::header::CONTENT_TYPE, API_CONTENT_TYPE)
            .header(reqwest::header::ACCEPT, "*/*")
            .header(reqwest::header::REFERER, &self.referer);
        if let Some(authorization) = authorization {
            request = request.header(reqwest::header::AUTHORIZATION, authorization);
        }
        if let Some(context_id) = x_context {
            request = request.header("X-Context", context_id);
        }
        let response = request
            .body(body.to_string())
            .send()
            .with_context(|| format!("failed to connect to {}", self.endpoint))?;

        let status = response.status();
        let text = response.text().context("failed to read router response")?;
        if !status.is_success() {
            if sensitive {
                bail!(
                    "router returned HTTP {status}; response body is hidden because the request contains credentials"
                );
            }
            bail!("router returned HTTP {status}: {}", short_body(&text));
        }

        serde_json::from_str(&text).with_context(|| {
            if sensitive {
                "router returned a non-JSON response; content is hidden because the request contains credentials"
                    .to_owned()
            } else {
                format!("router returned a non-JSON response: {}", short_body(&text))
            }
        })
    }

    /// Anonymous calls the web UI makes before login. They set the session
    /// cookie that the router requires for authenticated calls.
    fn warm_up(&self) -> Result<()> {
        for (service, method) in [("DeviceInfo", "get"), ("UserInterface", "getLanguage")] {
            let body = json!({ "service": service, "method": method, "parameters": {} });
            self.post_ws(&body, None, None, false)?;
        }
        Ok(())
    }

    pub fn login(host: &str, username: &str, password: &str) -> Result<Self> {
        let mut client = Self::new(host)?;
        client.warm_up()?;
        let body = json!({
            "service": "sah.Device.Information",
            "method": "createContext",
            "parameters": {
                "applicationName": APP_NAME,
                "username": username,
                "password": password
            }
        });
        let response = client.request_sensitive(&body, "X-Sah-Login")?;
        check_api_errors_with_privacy(&response, "login", true)?;

        let context_id = response
            .get("data")
            .and_then(|data| data.get("contextID"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow!("router did not provide a session; check the username and password")
            })?;
        client.context_id = Some(context_id.to_owned());
        Ok(client)
    }

    pub fn call(&self, service: &str, method: &str, parameters: Value) -> Result<Value> {
        self.call_with_privacy(service, method, parameters, false)
    }

    pub fn call_sensitive(&self, service: &str, method: &str, parameters: Value) -> Result<Value> {
        self.call_with_privacy(service, method, parameters, true)
    }

    /// For NMC-style services that report success as `status: true` and put the
    /// payload in `data`. Returns `data`, or an error when `errors` is present.
    pub fn call_nmc_data(&self, service: &str, method: &str, parameters: Value) -> Result<Value> {
        let context_id = self
            .context_id
            .as_deref()
            .ok_or_else(|| anyhow!("no active session"))?;
        let body = json!({
            "service": service,
            "method": method,
            "parameters": parameters
        });
        let authorization = format!("X-Sah {context_id}");
        let response = self.post_ws(&body, Some(&authorization), Some(context_id), false)?;
        let operation = format!("{service}.{method}");
        check_api_errors_with_privacy(&response, &operation, false)?;
        response
            .get("data")
            .cloned()
            .ok_or_else(|| anyhow!("response from {operation} is missing the data field"))
    }

    pub fn call_data(&self, service: &str, method: &str, parameters: Value) -> Result<Value> {
        self.call_data_with_privacy(service, method, parameters, false)
    }

    pub fn call_data_sensitive(
        &self,
        service: &str,
        method: &str,
        parameters: Value,
    ) -> Result<Value> {
        self.call_data_with_privacy(service, method, parameters, true)
    }

    fn call_data_with_privacy(
        &self,
        service: &str,
        method: &str,
        parameters: Value,
        sensitive: bool,
    ) -> Result<Value> {
        let context_id = self
            .context_id
            .as_deref()
            .ok_or_else(|| anyhow!("no active session"))?;
        let body = json!({
            "service": service,
            "method": method,
            "parameters": parameters
        });
        let authorization = format!("X-Sah {context_id}");
        let response = self.post_ws(&body, Some(&authorization), Some(context_id), sensitive)?;
        let operation = format!("{service}.{method}");
        check_api_errors_with_privacy(&response, &operation, sensitive)?;
        response_data_with_privacy(&response, &operation, sensitive)
    }

    fn call_with_privacy(
        &self,
        service: &str,
        method: &str,
        parameters: Value,
        sensitive: bool,
    ) -> Result<Value> {
        let context_id = self
            .context_id
            .as_deref()
            .ok_or_else(|| anyhow!("no active session"))?;
        let body = json!({
            "service": service,
            "method": method,
            "parameters": parameters
        });
        let authorization = format!("X-Sah {context_id}");
        let response = self.post_ws(&body, Some(&authorization), Some(context_id), sensitive)?;
        check_api_errors_with_privacy(&response, &format!("{service}.{method}"), sensitive)?;
        if let Some(status) = response.get("status").filter(|value| !value.is_null()) {
            return Ok(status.clone());
        }
        if let Some(data) = response.get("data") {
            return Ok(data.clone());
        }
        response.get("status").cloned().ok_or_else(|| {
            anyhow!("response from {service}.{method} is missing the status/data field")
        })
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

fn check_api_errors_with_privacy(response: &Value, operation: &str, sensitive: bool) -> Result<()> {
    let Some(errors) = response.get("errors").and_then(Value::as_array) else {
        return Ok(());
    };
    if errors.is_empty() {
        return Ok(());
    }

    if sensitive {
        bail!(
            "router error during {operation}; details are hidden because the request contains credentials"
        );
    }

    let details = errors
        .iter()
        .map(|error| {
            let description = error
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("API error");
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
    bail!("router error during {operation}: {details}");
}

fn response_data_with_privacy(response: &Value, operation: &str, sensitive: bool) -> Result<Value> {
    let status = response
        .get("status")
        .ok_or_else(|| anyhow!("response from {operation} is missing the status field"))?;
    if status != 0 {
        if sensitive {
            bail!(
                "router error during {operation}; details are hidden because the response contains personal data"
            );
        }
        bail!("router error during {operation}: status {status}");
    }
    response
        .get("data")
        .cloned()
        .ok_or_else(|| anyhow!("response from {operation} is missing the data field"))
}

fn short_body(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut result = compact.chars().take(240).collect::<String>();
    if compact.chars().count() > 240 {
        result.push('…');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{check_api_errors_with_privacy, response_data_with_privacy};

    #[test]
    fn semantic_response_data_requires_success_status_and_payload() {
        let response = serde_json::json!({
            "status": 0,
            "data": { "languageId": "pl" }
        });
        assert_eq!(
            response_data_with_privacy(&response, "getLanguage", false).unwrap(),
            serde_json::json!({ "languageId": "pl" })
        );
        assert!(
            response_data_with_privacy(&serde_json::json!({ "status": 1 }), "getLanguage", false)
                .is_err()
        );
        assert!(
            response_data_with_privacy(&serde_json::json!({ "data": {} }), "getLanguage", false)
                .is_err()
        );
        assert!(
            response_data_with_privacy(&serde_json::json!({ "status": 0 }), "getLanguage", false)
                .is_err()
        );
    }

    #[test]
    fn sensitive_response_status_is_not_echoed() {
        let response = serde_json::json!({
            "status": "phone-number-is-private",
            "data": {}
        });
        let error = response_data_with_privacy(&response, "CallLog.browse", true)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("phone-number-is-private"));
        assert!(error.contains("details are hidden"));
    }

    #[test]
    fn sensitive_api_errors_do_not_echo_provider_credentials() {
        let response = serde_json::json!({
            "errors": [{
                "description": "invalid credentials",
                "info": "password=provider-secret"
            }]
        });

        let error = check_api_errors_with_privacy(&response, "DynDNS.addHost", true)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("provider-secret"));
        assert!(error.contains("details are hidden"));
    }

    #[test]
    fn ordinary_api_errors_retain_diagnostic_details() {
        let response = serde_json::json!({
            "errors": [{ "description": "denied", "info": "policy" }]
        });

        let error = check_api_errors_with_privacy(&response, "Firewall.get", false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("denied (policy)"));
    }
}
