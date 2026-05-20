use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::models::requests::{Request, Response};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Config {
    base_url: String,
}

fn get_config_path() -> Result<PathBuf, Box<dyn Error>> {
    let exe_path = std::env::current_exe()?;

    let exe_dir = exe_path
        .parent()
        .ok_or("Failed to get executable directory")?;

    Ok(exe_dir.join("config.json"))
}

fn load_config() -> Result<Config, Box<dyn Error>> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        let default_config = Config {
            base_url: "http://192.168.0.1".to_string(),
        };

        let json = serde_json::to_string_pretty(&default_config)?;

        fs::write(&config_path, json)?;

        return Ok(default_config);
    }

    let content = fs::read_to_string(&config_path)?;

    let config: Config = serde_json::from_str(&content)?;

    Ok(config)
}

fn build_client(timeout_secs: u64) -> Result<Client, Box<dyn Error>> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .danger_accept_invalid_certs(true)
        .build()?)
}

async fn parse_response(
    response: reqwest::Response,
    context: &str,
) -> Result<Value, Box<dyn Error>> {
    let status = response.status();

    let text = response.text().await?;

    if text.trim().is_empty() {
        return Err(format!("{} returned empty response", context).into());
    }

    // Some modem firmwares return HTML login pages
    if text.trim_start().starts_with("<!DOCTYPE") || text.trim_start().starts_with("<html") {
        return Err(format!("{} returned HTML instead of JSON:\n{}", context, text).into());
    }

    if !status.is_success() {
        return Err(format!("{} failed with HTTP {}:\n{}", context, status, text).into());
    }

    match serde_json::from_str::<Value>(&text) {
        Ok(json) => Ok(json),
        Err(e) => Err(format!(
            "Invalid JSON from {}\n\nError: {}\n\nResponse:\n{}",
            context, e, text
        )
        .into()),
    }
}

pub async fn authenticate() -> Result<(String, String), Box<dyn Error>> {
    let config = load_config()?;

    let client = build_client(10)?;

    // Try admin/admin first
    let credentials = [("admin", "admin"), ("zitel", "zitel")];

    for (username, password) in credentials {
        let auth_body = format!("authenticate {} {}", username, password);

        let response = client
            .post(format!("{}/authenticate.leano", config.base_url))
            .header(
                "Content-Type",
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .body(auth_body)
            .send()
            .await?;

        let json = match parse_response(response, "Authentication request").await {
            Ok(j) => j,
            Err(e) => {
                println!("Authentication attempt failed for {}: {}", username, e);

                continue;
            }
        };

        if json["status"] == "success" {
            let token = json["token"].as_str().unwrap_or("").to_string();

            if token.is_empty() {
                return Err("Authentication succeeded but token missing".into());
            }

            println!("Authenticated successfully with {}", username);

            // If zitel/zitel works, switch to admin/admin
            if username == "zitel" {
                println!("Changing credentials from zitel/zitel to admin/admin...");

                let _ = client
                    .post(format!("{}/api.leano", config.base_url))
                    .header(
                        "Content-Type",
                        "application/x-www-form-urlencoded; charset=UTF-8",
                    )
                    .header("Leano_Auth", &token)
                    .header("Accept", "*/*")
                    .header("X-Requested-With", "XMLHttpRequest")
                    .body("setdigest admin admin")
                    .send()
                    .await;
            }

            return Ok((token.clone(), token));
        } else {
            println!("Authentication rejected for {}", username);
        }
    }

    Err("All authentication methods failed".into())
}

pub async fn api_request(auth_header: &str, command: &str) -> Result<Value, Box<dyn Error>> {
    let config = load_config()?;

    let client = build_client(30)?;

    let response = client
        .post(format!("{}/api.leano", config.base_url))
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .header("Leano_Auth", auth_header)
        .header("Accept", "*/*")
        .header("X-Requested-With", "XMLHttpRequest")
        .body(command.to_string())
        .send()
        .await?;

    parse_response(response, &format!("API command '{}'", command)).await
}

pub async fn run_handlers(
    auth_header: String,
    mut rx: mpsc::UnboundedReceiver<(Request, mpsc::UnboundedSender<Response>)>,
) {
    while let Some((request, resp_tx)) = rx.recv().await {
        match request {
            Request::RefreshDashboard => {
                let result = api_request(&auth_header, "get_index_data").await;

                let (data, error) = match result {
                    Ok(d) => (d, None),
                    Err(e) => (Value::Null, Some(e.to_string())),
                };

                let _ = resp_tx.send(Response::DashboardData { data, error });
            }

            Request::FetchNeighbors => {
                let result = api_request(&auth_header, "get_neighbour_cell").await;

                let (data, error) = match result {
                    Ok(d) => (d, None),
                    Err(e) => (Value::Null, Some(e.to_string())),
                };

                let _ = resp_tx.send(Response::NeighborData { data, error });
            }

            Request::SetBandLock { earfcn } => {
                let result = api_request(&auth_header, &format!("set_band_lock {}", earfcn)).await;

                let msg = match result {
                    Ok(resp) => format!(
                        "Set band lock to {}\n{}",
                        earfcn,
                        serde_json::to_string_pretty(&resp).unwrap_or_default()
                    ),
                    Err(e) => format!("Error: {}", e),
                };

                let _ = resp_tx.send(Response::BandLockResult { result: msg });
            }

            Request::SetDmz { ip } => {
                let result = api_request(&auth_header, &format!("set_dmz 1 tcpudp {}", ip)).await;

                let msg = match result {
                    Ok(resp) => serde_json::to_string_pretty(&resp).unwrap_or_default(),
                    Err(e) => format!("Error: {}", e),
                };

                let _ = resp_tx.send(Response::DmzResult(msg));
            }
        }
    }
}
