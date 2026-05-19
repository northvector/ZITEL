use std::error::Error;
use std::time::Duration;

use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::models::requests::{Request, Response};

const BASE_URL: &str = "http://192.168.0.1";

pub async fn authenticate() -> Result<(String, String), Box<dyn Error>> {
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    // Try admin/admin first
    let credentials = [("admin", "admin"), ("zitel", "zitel")];

    for (username, password) in credentials {
        let auth_body = format!("authenticate {} {}", username, password);

        let response = client
            .post(format!("{}/authenticate.leano", BASE_URL))
            .header(
                "Content-Type",
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .body(auth_body)
            .send()
            .await?;

        let json: Value = response.json().await?;

        if json["status"] == "success" {
            let token = json["token"].as_str().unwrap_or("").to_string();

            // If logged in with zitel/zitel, change password to admin/admin
            if username == "zitel" {
                let _ = client
                    .post(format!("{}/api.leano", BASE_URL))
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
        }
    }

    Err("Authentication failed".into())
}

pub async fn api_request(auth_header: &str, command: &str) -> Result<Value, Box<dyn Error>> {
    let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

    let response = client
        .post(format!("{}/api.leano", BASE_URL))
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

    Ok(response.json().await?)
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
                        "Set to {}: {}",
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
