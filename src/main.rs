use std::error::Error;
use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{Clear, ClearType},
    event::{EnableBracketedPaste, DisableBracketedPaste},
};

use tokio::time::sleep;

const BASE_URL: &str = "http://192.168.0.1";
const DEFAULT_DMZ_IP: &str = "192.168.0.98";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Zitel Router Manager | Created by @Northvector ===\n");
    
    // Enable bracketed paste for better terminal handling
    execute!(io::stdout(), EnableBracketedPaste)?;
    
    // Authenticate and get both token and auth header
    let (token, auth_header) = authenticate().await?;
    println!("✓ Authenticated successfully\n");
    
    loop {
        println!("\n--- Menu ---");
        println!("1. Set DMZ");
        println!("2. Get Dashboard Data");
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
    
    // Disable bracketed paste on exit
    execute!(io::stdout(), DisableBracketedPaste)?;
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

async fn api_request(_token: &str, auth_header: &str, command: &str) -> Result<serde_json::Value, Box<dyn Error>> {
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
    execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
        
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
    let mut current_page = 1;
    const TOTAL_PAGES: u8 = 6; // Data Usage, Connection Info, Network Info, Cell Info, IP Config, System Info
    
    loop {
        println!("\n=== Index Data - Page {} of {} ===", current_page, TOTAL_PAGES);
        
        // Clear screen and move cursor to top-left using crossterm
        execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
        io::stdout().flush()?; // Ensure clear command is executed
        
        let response = api_request(token, auth_header, "get_index_data").await?;
        
        match current_page {
            1 => display_data_usage(&response),
            2 => display_connection_info(&response),
            3 => display_network_info(&response),
            4 => display_cell_info(&response),
            5 => display_ip_config(&response),
            6 => display_system_info(&response),
            _ => display_network_info(&response), // Default to first page
        }
        
        println!("\nNavigation: [N]ext, [P]revious, [Q]uit");
        print!("Enter command: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        match input.trim().to_lowercase().as_str() {
            "n" | "next" => {
                current_page = (current_page % TOTAL_PAGES) + 1;
            }
            "p" | "prev" | "previous" => {
                current_page = if current_page == 1 { TOTAL_PAGES } else { current_page - 1 };
            }
            "q" | "quit" => break,
            _ => println!("Invalid command"),
        }
        
        // Add a small delay to prevent excessive refresh
        sleep(Duration::from_millis(200)).await;
    }
    
    Ok(())
}

fn display_data_usage(json: &serde_json::Value) {
    println!("╔════════════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                                    DATA USAGE                                                ║");
    println!("╠════════════════════════════════════════════════════════════════════════════════════════════════╣");
    println!("║ Last Updated: {:70} ║", chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    println!("╚════════════════════════════════════════════════════════════════════════════════════════════════╝\n");
    
    // Data Usage Table
    println!("┌─────────────────────────────────────────────────────────────────────────────────────────────┐");
    println!("│ 📊 DATA USAGE                                                                               │");
    println!("├─────────────────────────────┬───────────────────────────────────────────────────────────────┤");
    if let Some(rx) = json["recieve"].as_str() {
        if let Ok(bytes) = rx.parse::<u64>() {
            println!("│ {:27} │ {:61} │", "Received", format_bytes(bytes));
        }
    }
    if let Some(tx) = json["sentt"].as_str() {
        if let Ok(bytes) = tx.parse::<u64>() {
            println!("│ {:27} │ {:61} │", "Sent", format_bytes(bytes));
        }
    }
    println!("└─────────────────────────────┴───────────────────────────────────────────────────────────────┘\n");
}

fn display_connection_info(json: &serde_json::Value) {
    // Connection Table
    println!("╔════════════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                                    CONNECTION INFO                                           ║");
    println!("╠════════════════════════════════════════════════════════════════════════════════════════════════╣");
    println!("║ Last Updated: {:70} ║", chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    println!("╚════════════════════════════════════════════════════════════════════════════════════════════════╝\n");
    
    println!("┌─────────────────────────────────────────────────────────────────────────────────────────────┐");
    println!("│ 🔗 CONNECTION                                                                               │");
    println!("├─────────────────────────────┬───────────────────────────────────────────────────────────────┤");
    print_table_row(json, "TYPE", "Type");
    print_table_row(json, "BAND", "Band");
    print_table_row(json, "CSQ", "Signal Quality");
    print_table_row(json, "RSRP", "RSRP");
    print_table_row(json, "RSRQ", "RSRQ");
    print_table_row(json, "SINR", "SINR");
    print_table_row(json, "RSSI", "RSSI");
    println!("└─────────────────────────────┴───────────────────────────────────────────────────────────────┘\n");
}

fn display_network_info(json: &serde_json::Value) {
    println!("╔════════════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                                    NETWORK INFORMATION                                     ║");
    println!("╠════════════════════════════════════════════════════════════════════════════════════════════════╣");
    println!("║ Last Updated: {:70} ║", chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    println!("╚════════════════════════════════════════════════════════════════════════════════════════════════╝\n");
    
    // Network Information Table
    println!("┌─────────────────────────────────────────────────────────────────────────────────────────────┐");
    println!("│ 📡 NETWORK INFORMATION                                                                      │");
    println!("├─────────────────────────────┬───────────────────────────────────────────────────────────────┤");
    print_table_row(json, "IMEI", "IMEI");
    print_table_row(json, "IMSI", "IMSI");
    print_table_row(json, "ICCID", "ICCID");
    print_table_row(json, "APN", "APN");
    print_table_row(json, "INTERNET", "Internet Status");
    println!("└─────────────────────────────┴───────────────────────────────────────────────────────────────┘\n");
}

fn display_cell_info(json: &serde_json::Value) {
    // Cell Information Table
    println!("╔════════════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                                    CELL INFO                                                 ║");
    println!("╠════════════════════════════════════════════════════════════════════════════════════════════════╣");
    println!("║ Last Updated: {:70} ║", chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    println!("╚════════════════════════════════════════════════════════════════════════════════════════════════╝\n");
    
    println!("┌─────────────────────────────────────────────────────────────────────────────────────────────┐");
    println!("│ 📍 CELL INFO                                                                                │");
    println!("├─────────────────────────────┬───────────────────────────────────────────────────────────────┤");
    print_table_row(json, "MCC", "MCC");
    print_table_row(json, "MNC", "MNC");
    print_table_row(json, "PCID", "PCI");
    print_table_row(json, "EARFCN", "EARFCN");
    print_table_row(json, "TAC", "TAC");
    print_table_row(json, "ENODE", "eNodeB");
    print_table_row(json, "CELL", "Cell ID");
    println!("└─────────────────────────────┴───────────────────────────────────────────────────────────────┘\n");
}

fn display_ip_config(json: &serde_json::Value) {
    // IP Configuration Table
    println!("╔════════════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                                    IP CONFIGURATION                                        ║");
    println!("╠════════════════════════════════════════════════════════════════════════════════════════════════╣");
    println!("║ Last Updated: {:70} ║", chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    println!("╚════════════════════════════════════════════════════════════════════════════════════════════════╝\n");
    
    println!("┌─────────────────────────────────────────────────────────────────────────────────────────────┐");
    println!("│ 🌐 IP CONFIGURATION                                                                         │");
    println!("├─────────────────────────────┬───────────────────────────────────────────────────────────────┤");
    print_table_row(json, "IPV4", "IPv4");
    print_table_row(json, "IPV6", "IPv6");
    print_table_row(json, "DNS1", "DNS1");
    print_table_row(json, "DNS2", "DNS2");
    print_table_row(json, "lanip", "LAN IP");
    print_table_row(json, "netmask", "Netmask");
    println!("└─────────────────────────────┴───────────────────────────────────────────────────────────────┘\n");
}

fn display_system_info(json: &serde_json::Value) {
    // System Information Table
    println!("╔════════════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                                    SYSTEM INFO                                               ║");
    println!("╠════════════════════════════════════════════════════════════════════════════════════════════════╣");
    println!("║ Last Updated: {:70} ║", chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    println!("╚════════════════════════════════════════════════════════════════════════════════════════════════╝\n");
    
    println!("┌─────────────────────────────────────────────────────────────────────────────────────────────┐");
    println!("│ ⚙️  SYSTEM                                                                                   │");
    println!("├─────────────────────────────┬───────────────────────────────────────────────────────────────┤");
    print_table_row(json, "model", "Model");
    print_table_row(json, "serial", "Serial");
    print_table_row(json, "hardv", "Hardware");
    print_table_row(json, "sofv", "Software");
    print_table_row(json, "SYSUP", "System Uptime (s)");
    print_table_row(json, "WANUP", "WAN Uptime (s)");
    print_table_row(json, "ram", "RAM (MB)");
    print_table_row(json, "cpu1", "CPU1 (%)");
    print_table_row(json, "cpu2", "CPU2 (%)");
    println!("└─────────────────────────────┴───────────────────────────────────────────────────────────────┘");
}

fn print_table_row(json: &serde_json::Value, key: &str, label: &str) {
    if let Some(value) = json[key].as_str() {
        if !value.is_empty() {
            println!("│ {:27} │ {:61} │", label, value);
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

    execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
    io::stdout().flush()?; // Ensure clear command is executed


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
                print_table_row(&response, &format!("type{}", i), "MCC");
                print_table_row(&response, &format!("band{}", i), "MNC");
                print_table_row(&response, &format!("pcid{}", i), "Band");
                print_table_row(&response, &format!("rsrq{}", i), "ARFCN");
                print_table_row(&response, &format!("rsrp{}", i), "PCI");
                print_table_row(&response, &format!("rsrppp{}", i), "Signal (dBm)");
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