use anyhow::{anyhow, Result};
use reqwest::{Client, header};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

const BASE_URL: &str = "http://192.168.0.1";

#[derive(Debug, Deserialize)]
struct AuthResponse {
    status: String,
    code: String,
    token: String,
}

#[derive(Debug, Deserialize)]
struct GenericResponse {
    status: String,
    code: String,
}

#[derive(Debug, Deserialize)]
struct IndexData {
    IMEI: String,
    IMSI: String,
    CSQ: String,
    IPV4: String,
    INTERNET: String,
    SYSUP: String,
    WANUP: String,
    recieve: String,
    sentt: String,
    cpu1: String,
    cpu2: String,
    ram: String,
    status: String,
    code: String,
}

struct LeanoClient {
    client: Client,
    token: String,
}

impl LeanoClient {
    async fn authenticate() -> Result<Self> {
        let client = Client::new();

        let res = client
            .post(format!("{BASE_URL}/authenticate.leano"))
            .header(header::CONTENT_TYPE, "application/xml")
            .body("authenticate admin admin")
            .send()
            .await?
            .json::<AuthResponse>()
            .await?;

        if res.status != "success" {
            return Err(anyhow!("Authentication failed"));
        }

        Ok(Self {
            client,
            token: res.token,
        })
    }

    async fn post_command(&self, command: &str) -> Result<serde_json::Value> {
        let res = self
            .client
            .post(format!("{BASE_URL}/api.leano"))
            .header("Leano_Auth", &self.token)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(command.to_string())
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        Ok(res)
    }

    // 1) SET DMZ
    async fn set_dmz(&self, ip: Option<&str>) -> Result<()> {
        let ip = ip.unwrap_or("192.168.0.98");
        let cmd = format!("set_dmz 1 tcpudp {}", ip);
        let res = self.post_command(&cmd).await?;

        if res["status"] == "success" {
            println!("DMZ enabled for {}", ip);
            Ok(())
        } else {
            Err(anyhow!("Failed to set DMZ"))
        }
    }

    // 2) GET INDEX DATA (AUTO REFRESH)
    async fn monitor_index_data(&self) -> Result<()> {
        loop {
            let res = self.post_command("get_index_data").await?;
            let data: IndexData = serde_json::from_value(res)?;

            print!("\x1B[2J\x1B[1;1H"); // clear screen

            println!("=== MODEM STATUS ===");
            println!("IMEI      : {}", data.IMEI);
            println!("IP        : {}", data.IPV4);
            println!("CSQ       : {}", data.CSQ);
            println!("Internet  : {}", data.INTERNET);
            println!("CPU       : {}% / {}%", data.cpu1, data.cpu2);
            println!("RAM       : {} KB", data.ram);
            println!("Uptime    : {} sec", data.SYSUP);
            println!("RX Bytes  : {}", data.recieve);
            println!("TX Bytes  : {}", data.sentt);

            sleep(Duration::from_secs(3)).await;
        }
    }

    // 3) GET NEIGHBOUR CELL
    async fn get_neighbour_cell(&self) -> Result<()> {
        let res = self.post_command("get_neighbour_cell").await?;
        println!("{}", serde_json::to_string_pretty(&res)?);
        Ok(())
    }

    // 3) SET BAND LOCK
    async fn set_band_lock(&self, earfcn: u32) -> Result<()> {
        let cmd = format!("set_band_lock {}", earfcn);
        let res: GenericResponse = serde_json::from_value(self.post_command(&cmd).await?)?;

        if res.status == "success" {
            println!("Band lock set to EARFCN {}", earfcn);
            Ok(())
        } else {
            Err(anyhow!("Failed to set band lock"))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let leano = LeanoClient::authenticate().await?;

    // Examples:
    leano.set_dmz(None).await?;
    // leano.monitor_index_data().await?;
    // leano.get_neighbour_cell().await?;
    // leano.set_band_lock(42490).await?;

    Ok(())
}
