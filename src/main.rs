use std::collections::HashMap;
use std::error::Error;
use std::io::{self, Write};
use std::time::Duration;

const BASE_URL: &str = "http://192.168.0.1";
const DEFAULT_DMZ_IP: &str = "192.168.0.98";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Leano Router API Client ===\n");
    
    // Authenticate and get both token and auth header
    let (token, auth_header) = authenticate().await?;
    println!("✓ Authenticated successfully\n");
    
    loop {
        println!("\n--- Menu ---");
        println!("1. Set DMZ");
        println!("2. Get Index Data (Auto-refresh table)");
        println!("3. Get Neighbor Cells");
        println!("4. Set Band Lock");
        println!("5. Exit");
        print!("\nSelect option: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        match input.trim() {
            "1" => set_dmz(&token, &auth_header).await?,
            "2" => get_index_data_loop(&token, &auth_header).await?,
            "3" => get_neighbour_cells(&token, &auth_header).await?,
            "4" => set_band_lock(&token, &auth_header).await?,
            "5" => {
                println!("Goodbye!");
                break;
            }
            _ => println!("Invalid option"),
        }
    }
    
    Ok(())
}

async fn authenticate() -> Result<(String, String), Box<dyn Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    
    let url = format!("{}/authenticate.leano", BASE_URL);
    let xml_data = "authenticate admin admin";
    
    let response = client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .body(xml_data)
        .send()
        .await?;
    
    let json: serde_json::Value = response.json().await?;
    
    if json["status"] == "success" {
        let token = json["token"].as_str().unwrap_or("").to_string();
        let auth_header = json["token"].as_str().unwrap_or("").to_string();
        Ok((token, auth_header))
    } else {
        Err("Authentication failed".into())
    }
}

async fn api_request(token: &str, auth_header: &str, command: &str) -> Result<serde_json::Value, Box<dyn Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    
    let url = format!("{}/api.leano", BASE_URL);
    
    let response = client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .header("Leano_Auth", auth_header)
        .header("Accept", "*/*")
        .header("X-Requested-With", "XMLHttpRequest")
        .body(command.to_string())
        .send()
        .await?;
    
    let json: serde_json::Value = response.json().await?;
    Ok(json)
}

async fn set_dmz(token: &str, auth_header: &str) -> Result<(), Box<dyn Error>> {
    print!("Enter DMZ IP address (press Enter for default {}): ", DEFAULT_DMZ_IP);
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    let ip = input.trim();
    let ip = if ip.is_empty() { DEFAULT_DMZ_IP } else { ip };
    
    let command = format!("set_dmz 1 tcpudp {}", ip);
    let response = api_request(token, auth_header, &command).await?;
    
    println!("\nResponse: {}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

async fn get_index_data_loop(token: &str, auth_header: &str) -> Result<(), Box<dyn Error>> {
    println!("\n=== Index Data (Press Ctrl+C to stop) ===\n");
    
    loop {
        let response = api_request(token, auth_header, "get_index_data").await?;
        
        // Clear screen (works on Unix and Windows 10+)
        print!("\x1B[2J\x1B[1;1H");
        
        println!("╔══════════════════════════════════════════════════════════════════════╗");
        println!("║                        ROUTER STATUS                                 ║");
        println!("╚══════════════════════════════════════════════════════════════════════╝\n");
        
        // Network Information
        println!("📡 NETWORK INFORMATION:");
        print_field(&response, "IMEI", "IMEI");
        print_field(&response, "IMSI", "IMSI");
        print_field(&response, "ICCID", "ICCID");
        print_field(&response, "APN", "APN");
        print_field(&response, "INTERNET", "Status");
        println!();
        
        // Connection Details
        println!("🔗 CONNECTION:");
        print_field(&response, "TYPE", "Type");
        print_field(&response, "BAND", "Band");
        print_field(&response, "CSQ", "Signal Quality");
        print_field(&response, "RSRP", "RSRP (dBm)");
        print_field(&response, "RSRQ", "RSRQ (dB)");
        print_field(&response, "SINR", "SINR (dB)");
        print_field(&response, "RSSI", "RSSI (dBm)");
        println!();
        
        // Cell Information
        println!("📍 CELL INFO:");
        print_field(&response, "MCC", "MCC");
        print_field(&response, "MNC", "MNC");
        print_field(&response, "PCID", "PCI");
        print_field(&response, "EARFCN", "EARFCN");
        print_field(&response, "TAC", "TAC");
        print_field(&response, "ENODE", "eNodeB");
        print_field(&response, "CELL", "Cell ID");
        println!();
        
        // IP Information
        println!("🌐 IP CONFIG:");
        print_field(&response, "IPV4", "IPv4");
        print_field(&response, "IPV6", "IPv6");
        print_field(&response, "DNS1", "DNS1");
        print_field(&response, "DNS2", "DNS2");
        print_field(&response, "lanip", "LAN IP");
        print_field(&response, "netmask", "Netmask");
        println!();
        
        // Data Usage
        println!("📊 DATA USAGE:");
        if let Some(rx) = response["recieve"].as_str() {
            if let Ok(bytes) = rx.parse::<u64>() {
                println!("  Received: {}", format_bytes(bytes));
            }
        }
        if let Some(tx) = response["sentt"].as_str() {
            if let Ok(bytes) = tx.parse::<u64>() {
                println!("  Sent: {}", format_bytes(bytes));
            }
        }
        println!();
        
        // System Information
        println!("⚙️  SYSTEM:");
        print_field(&response, "model", "Model");
        print_field(&response, "serial", "Serial");
        print_field(&response, "hardv", "Hardware");
        print_field(&response, "sofv", "Software");
        print_field(&response, "SYSUP", "System Uptime (s)");
        print_field(&response, "WANUP", "WAN Uptime (s)");
        print_field(&response, "ram", "RAM (MB)");
        print_field(&response, "cpu1", "CPU1 (%)");
        print_field(&response, "cpu2", "CPU2 (%)");
        println!();
        
        println!("Last updated: {}", chrono::Local::now().format("%H:%M:%S"));
        println!("\n(Refreshing in 5 seconds... Press Ctrl+C to stop)");
        
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

fn print_field(json: &serde_json::Value, key: &str, label: &str) {
    if let Some(value) = json[key].as_str() {
        if !value.is_empty() {
            println!("  {}: {}", label, value);
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

async fn get_neighbour_cells(token: &str, auth_header: &str) -> Result<(), Box<dyn Error>> {
    println!("\nFetching neighbor cells (this may take a moment)...");
    
    let response = api_request(token, auth_header, "get_neighbour_cell").await?;
    
    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                      NEIGHBOR CELLS                                  ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝\n");
    
    if let Some(length) = response["lenghtt"].as_str() {
        println!("Found {} neighbor cell(s)\n", length);
        
        if let Ok(count) = length.parse::<usize>() {
            for i in 1..=count {
                println!("─── Cell {} ───", i);
                print_field(&response, &format!("type{}", i), "MCC");
                print_field(&response, &format!("band{}", i), "MNC");
                print_field(&response, &format!("pcid{}", i), "Band");
                print_field(&response, &format!("rsrq{}", i), "ARFCN");
                print_field(&response, &format!("rsrp{}", i), "PCI");
                print_field(&response, &format!("rsrppp{}", i), "Signal (dBm)");
                println!();
            }
        }
    }
    
    Ok(())
}

async fn set_band_lock(token: &str, auth_header: &str) -> Result<(), Box<dyn Error>> {
    print!("Enter EARFCN to lock (e.g., 42490): ");
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    let earfcn = input.trim();
    
    if earfcn.is_empty() {
        println!("EARFCN cannot be empty");
        return Ok(());
    }
    
    let command = format!("set_band_lock {}", earfcn);
    let response = api_request(token, auth_header, &command).await?;
    
    println!("\nResponse: {}", serde_json::to_string_pretty(&response)?);
    
    if response["status"] == "success" {
        println!("✓ Band locked successfully to EARFCN {}", earfcn);
    }
    
    Ok(())
}