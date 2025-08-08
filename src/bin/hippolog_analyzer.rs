use anyhow::{Context, Result};
use std::env;
use std::path::Path;
use base64::prelude::*;

use slv_rust::utils::hippolog_parser::{HippologParser, LogEntry};

fn extract_message_name(entry: &LogEntry) -> Option<String> {
    // For LLUDP entries, try to extract message name from the decoded data
    if entry.entry_type == "LLUDP" {
        if let Some(message_obj) = entry.data.get("message") {
            if let Some(message_bytes) = message_obj.get("__bytes__") {
                if let Some(encoded_str) = message_bytes.as_str() {
                    if let Ok(decoded_bytes) = base64::prelude::Engine::decode(
                        &base64::prelude::BASE64_STANDARD, 
                        encoded_str
                    ) {
                        if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                            // Look for 'message':'MessageName' pattern
                            if let Some(start) = decoded_str.find("'message':'") {
                                let start_pos = start + "'message':'".len();
                                if let Some(end) = decoded_str[start_pos..].find("'") {
                                    return Some(decoded_str[start_pos..start_pos + end].to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[derive(Debug)]
struct PacketFlags {
    reliable: bool,
    resent: bool,
    zerocoded: bool,
    appended_acks: bool,
}

#[derive(Debug)]
struct PacketInfo {
    flags: Option<PacketFlags>,
    frequency: Option<String>,
    trust_level: Option<String>,
    packet_id: Option<u32>,
}

impl std::fmt::Display for PacketFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut flags = Vec::new();
        if self.reliable { flags.push("R"); }
        if self.resent { flags.push("RS"); }
        if self.zerocoded { flags.push("Z"); }
        if self.appended_acks { flags.push("A"); }
        
        if flags.is_empty() {
            write!(f, "-")
        } else {
            write!(f, "{}", flags.join("|"))
        }
    }
}

fn extract_packet_flags(entry: &LogEntry) -> Option<PacketFlags> {
    // For LLUDP entries, try to extract packet flags from raw packet data
    if entry.entry_type == "LLUDP" {
        // Look for 'extra' field that might contain raw packet data
        if let Some(extra_data) = entry.data.get("extra") {
            if let Some(encoded_str) = extra_data.get("__bytes__").and_then(|v| v.as_str()) {
                if let Ok(packet_data) = base64::prelude::Engine::decode(
                    &base64::prelude::BASE64_STANDARD, 
                    encoded_str
                ) {
                    if !packet_data.is_empty() {
                        return Some(parse_packet_flags(packet_data[0]));
                    }
                }
            }
        }
        
        // Try to extract from message data if available
        if let Some(message_obj) = entry.data.get("message") {
            // Look for send_flags field in the decoded message
            if let Some(message_bytes) = message_obj.get("__bytes__") {
                if let Some(encoded_str) = message_bytes.as_str() {
                    if let Ok(decoded_bytes) = base64::prelude::Engine::decode(
                        &base64::prelude::BASE64_STANDARD, 
                        encoded_str
                    ) {
                        if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                            // Look for 'send_flags':i128 pattern
                            if let Some(start) = decoded_str.find("'send_flags':i") {
                                let start_pos = start + "'send_flags':i".len();
                                if let Some(end) = decoded_str[start_pos..].find([',', '}']) {
                                    if let Ok(flags) = decoded_str[start_pos..start_pos + end].parse::<u8>() {
                                        return Some(parse_packet_flags(flags));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn parse_packet_flags(flags_byte: u8) -> PacketFlags {
    PacketFlags {
        reliable: (flags_byte & 0x40) != 0,      // ACK_FLAG
        resent: (flags_byte & 0x20) != 0,        // RESENT_FLAG  
        zerocoded: (flags_byte & 0x80) != 0,     // ZERO_CODE_FLAG
        appended_acks: (flags_byte & 0x10) != 0, // APPENDED_ACK_FLAG
    }
}

fn extract_packet_info(entry: &LogEntry) -> PacketInfo {
    let mut info = PacketInfo {
        flags: extract_packet_flags(entry),
        frequency: None,
        trust_level: None,
        packet_id: None,
    };
    
    if entry.entry_type == "LLUDP" {
        // Extract from decoded message data
        if let Some(message_obj) = entry.data.get("message") {
            if let Some(message_bytes) = message_obj.get("__bytes__") {
                if let Some(encoded_str) = message_bytes.as_str() {
                    if let Ok(decoded_bytes) = base64::prelude::Engine::decode(
                        &base64::prelude::BASE64_STANDARD, 
                        encoded_str
                    ) {
                        if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                            // Extract packet_id
                            if let Some(start) = decoded_str.find("'packet_id':i") {
                                let start_pos = start + "'packet_id':i".len();
                                if let Some(end) = decoded_str[start_pos..].find([',', '}']) {
                                    if let Ok(id) = decoded_str[start_pos..start_pos + end].parse::<u32>() {
                                        info.packet_id = Some(id);
                                    }
                                }
                            }
                            
                            // Try to determine frequency and trust from packet content
                            // This is heuristic-based since hippolog doesn't directly store these
                            if let Some(message_name) = extract_message_name(entry) {
                                info.frequency = Some(guess_frequency(&message_name));
                                info.trust_level = Some(guess_trust_level(&message_name, &entry.meta.method));
                            }
                        }
                    }
                }
            }
        }
    }
    
    info
}

fn guess_frequency(message_name: &str) -> String {
    // Based on common SecondLife message frequencies
    match message_name {
        "AgentUpdate" => "High".to_string(),
        "ViewerEffect" => "Medium".to_string(),
        "PacketAck" | "UseCircuitCode" | "CompleteAgentMovement" => "Fixed".to_string(),
        "RegionHandshake" | "AgentDataUpdate" | "AgentMovementComplete" => "Low".to_string(),
        _ => {
            // Guess based on message name patterns
            if message_name.contains("Update") && !message_name.contains("Agent") {
                "High".to_string()
            } else if message_name.contains("Request") || message_name.contains("Reply") {
                "Medium".to_string()
            } else {
                "Low".to_string()
            }
        }
    }
}

fn guess_trust_level(message_name: &str, direction: &str) -> String {
    // Based on common SecondLife trust patterns
    match message_name {
        "RegionHandshake" | "AgentDataUpdate" | "AgentMovementComplete" => "Trusted".to_string(),
        "UseCircuitCode" | "CompleteAgentMovement" | "PacketAck" => "NotTrusted".to_string(),
        _ => {
            // Server->Client messages are usually trusted, Client->Server usually not
            if direction == "IN" {
                "Trusted".to_string()
            } else {
                "NotTrusted".to_string()
            }
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <hippolog_file> [command]", args[0]);
        eprintln!("Commands:");
        eprintln!("  stats     - Show basic statistics (default)");
        eprintln!("  http      - Show HTTP entries");
        eprintln!("  lludp     - Show LLUDP entries");
        eprintln!("  eq        - Show EQ entries");
        eprintln!("  grep <pattern> [--case-sensitive] - Search for pattern in entries");
        eprintln!("  detail <index> [--pretty] [--decode-bytes] - Show full details for entry at index");
        eprintln!("  export    - Export back to hippolog format");
        eprintln!("  list      - Show packet headers (type, method, index) only");
        eprintln!("  flags     - Show packet flags analysis (reliable, resent, etc.)");
        eprintln!("  http-summary - Show HTTP request summaries (method, URL, status)");
        return Ok(());
    }

    let hippolog_path = &args[1];
    let command = args.get(2).map(String::as_str).unwrap_or("stats");

    println!("Parsing hippolog file: {}", hippolog_path);
    
    let parser = HippologParser::from_file(hippolog_path)
        .with_context(|| format!("Failed to parse hippolog file: {}", hippolog_path))?;

    match command {
        "stats" => {
            let stats = parser.stats();
            println!("{}", stats);
        }
        
        "http" => {
            let http_entries = parser.http_entries();
            println!("HTTP Entries ({}):", http_entries.len());
            for (i, entry) in http_entries.iter().enumerate().take(10) {
                println!("  {}: {} - {}", i + 1, entry.meta.method, entry.summary);
                if let Some(flow) = entry.data.get("flow") {
                    if let Some(url) = flow.get("request").and_then(|r| r.get("url")) {
                        println!("      URL: {}", url);
                    }
                }
            }
            if http_entries.len() > 10 {
                println!("  ... and {} more", http_entries.len() - 10);
            }
        }
        
        "lludp" => {
            let lludp_entries = parser.lludp_entries();
            println!("LLUDP Entries ({}):", lludp_entries.len());
            for (i, entry) in lludp_entries.iter().enumerate().take(10) {
                println!("  {}: {} - {}", i + 1, entry.meta.method, entry.summary);
            }
            if lludp_entries.len() > 10 {
                println!("  ... and {} more", lludp_entries.len() - 10);
            }
        }
        
        "eq" => {
            let eq_entries = parser.eq_entries();
            println!("EQ Entries ({}):", eq_entries.len());
            for (i, entry) in eq_entries.iter().enumerate().take(10) {
                println!("  {}: {} - {}", i + 1, entry.meta.method, entry.summary);
            }
            if eq_entries.len() > 10 {
                println!("  ... and {} more", eq_entries.len() - 10);
            }
        }
        
        "export" => {
            let export_data = parser.export_to_hippolog()
                .context("Failed to export hippolog data")?;
            
            let output_path = format!("{}.exported.hippolog", 
                Path::new(hippolog_path).file_stem().unwrap().to_string_lossy());
            
            std::fs::write(&output_path, &export_data)
                .with_context(|| format!("Failed to write exported file: {}", output_path))?;
            
            println!("Exported {} bytes to: {}", export_data.len(), output_path);
        }
        
        "grep" => {
            if args.len() < 4 {
                eprintln!("grep command requires a pattern: {} <file> grep <pattern> [--case-sensitive]", args[0]);
                return Ok(());
            }
            
            let pattern = &args[3];
            let case_sensitive = args.get(4).map(String::as_str) == Some("--case-sensitive");
            
            let matches = parser.grep(pattern, case_sensitive);
            println!("Found {} matches for '{}' (case {}sensitive):", 
                matches.len(), pattern, if case_sensitive { "" } else { "in" });
            
            if matches.len() == 1 {
                // Single match - show full details
                let m = &matches[0];
                println!("\n=== Single Match Found ===");
                println!("Entry #{} [{}] {} - {}", 
                    m.entry_index, 
                    m.entry.entry_type,
                    m.entry.meta.method,
                    m.entry.summary.chars().take(60).collect::<String>()
                );
                println!("Matches in: {}", 
                    m.match_locations.iter()
                        .map(|l| l.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                println!("\n=== Full Packet Details ===");
                match parser.get_packet_details(m.entry_index, true) {
                    Ok(details) => println!("{}", details),
                    Err(e) => eprintln!("Error getting packet details: {}", e),
                }
            } else if matches.is_empty() {
                println!("No matches found.");
            } else {
                // Multiple matches - show summary list
                for (i, m) in matches.iter().enumerate().take(20) {
                    println!("  {}: Entry #{} [{}] {} - {}", 
                        i + 1, 
                        m.entry_index, 
                        m.entry.entry_type,
                        m.entry.meta.method,
                        m.entry.summary.chars().take(60).collect::<String>()
                    );
                    println!("     Matches in: {}", 
                        m.match_locations.iter()
                            .map(|l| l.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                
                if matches.len() > 20 {
                    println!("  ... and {} more matches", matches.len() - 20);
                }
                
                println!("\nTo view full details of a specific match, use:");
                println!("  {} {} detail <entry_index> --pretty", args[0], args[1]);
            }
        }
        
        "detail" => {
            if args.len() < 4 {
                eprintln!("detail command requires an entry index: {} <file> detail <index> [--pretty] [--decode-bytes]", args[0]);
                return Ok(());
            }
            
            let entry_index: usize = args[3].parse()
                .with_context(|| format!("Invalid entry index: {}", args[3]))?;
            let pretty = args.iter().skip(4).any(|arg| arg == "--pretty");
            let decode_bytes = args.iter().skip(4).any(|arg| arg == "--decode-bytes");
            
            match parser.get_packet_details(entry_index, pretty) {
                Ok(details) => {
                    println!("{}", details);
                    
                    // Show comprehensive packet info if available
                    if let Some(entry) = parser.entries().get(entry_index) {
                        let packet_info = extract_packet_info(entry);
                        
                        println!("\n=== Packet Analysis ===");
                        
                        if let Some(flags) = &packet_info.flags {
                            println!("Reliable: {} (ACK required)", if flags.reliable { "Yes" } else { "No" });
                            println!("Resent: {}", if flags.resent { "Yes" } else { "No" });
                            println!("Zero-coded: {}", if flags.zerocoded { "Yes" } else { "No" });
                            println!("Appended ACKs: {}", if flags.appended_acks { "Yes" } else { "No" });
                            println!("Flags byte: 0x{:02x}", 
                                (if flags.reliable { 0x40 } else { 0 }) |
                                (if flags.resent { 0x20 } else { 0 }) |
                                (if flags.zerocoded { 0x80 } else { 0 }) |
                                (if flags.appended_acks { 0x10 } else { 0 })
                            );
                        } else {
                            println!("Flags: Not available");
                        }
                        
                        println!("Frequency: {}", packet_info.frequency.as_deref().unwrap_or("Unknown"));
                        println!("Trust Level: {}", packet_info.trust_level.as_deref().unwrap_or("Unknown"));
                        
                        if let Some(id) = packet_info.packet_id {
                            println!("Packet ID: {} (0x{:x})", id, id);
                        } else {
                            println!("Packet ID: Not available");
                        }
                        
                        // Show message name if available
                        if let Some(msg_name) = extract_message_name(entry) {
                            println!("Message Type: {}", msg_name);
                        }
                    }
                    
                    // Always show decoded bytes when using detail command
                    if decode_bytes {
                        println!("\n=== Additional Bytes Analysis ===");
                        // Could add more detailed bytes analysis here if needed
                        println!("(Bytes already decoded above in main output)");
                    }
                }
                Err(e) => {
                    eprintln!("Error getting packet details: {}", e);
                    let stats = parser.stats();
                    eprintln!("Valid entry indices are 0 to {}", stats.total_entries.saturating_sub(1));
                }
            }
        }
        
        "list" => {
            let entries = parser.entries();
            println!("Packet List ({} total):", entries.len());
            println!("Format: [Type] Direction [Flags] Freq/Trust - (MessageName) Summary");
            println!("Flags: R=Reliable, RS=Resent, Z=Zero-coded, A=Appended ACKs");
            println!();
            
            for (i, entry) in entries.iter().enumerate() {
                let message_name = extract_message_name(entry);
                let message_display = if let Some(name) = message_name {
                    format!("({}) ", name)
                } else {
                    String::new()
                };
                
                let packet_info = extract_packet_info(entry);
                
                let flags_display = if let Some(flags) = &packet_info.flags {
                    format!(" [{}]", flags)
                } else {
                    " [-]".to_string()
                };
                
                let freq_trust_display = format!(" {}/{}",
                    packet_info.frequency.as_deref().unwrap_or("?"),
                    packet_info.trust_level.as_deref().unwrap_or("?")
                );
                
                println!("{}: [{}] {}{}{} - {}{}", 
                    i + 1, 
                    entry.entry_type,
                    entry.meta.method,
                    flags_display,
                    freq_trust_display,
                    message_display,
                    entry.summary.chars().take(55).collect::<String>()
                );
            }
        }
        
        "flags" => {
            let entries = parser.entries();
            let lludp_entries: Vec<_> = entries.iter().enumerate()
                .filter(|(_, entry)| entry.entry_type == "LLUDP")
                .collect();
            
            println!("Packet Analysis ({} LLUDP packets):", lludp_entries.len());
            println!("Format: [Type] Dir [Flags] Freq/Trust ID - (MessageName) Summary");  
            println!("Flags: R=Reliable(ACK required), RS=Resent, Z=Zero-coded, A=Appended ACKs");
            println!();
            
            let mut reliable_count = 0;
            let mut resent_count = 0;
            let mut zerocoded_count = 0;
            let mut appended_acks_count = 0;
            let mut frequency_counts = std::collections::HashMap::new();
            let mut trust_counts = std::collections::HashMap::new();
            
            for (i, entry) in lludp_entries.iter() {
                let message_name = extract_message_name(entry);
                let message_display = if let Some(name) = message_name {
                    format!("({}) ", name)
                } else {
                    String::new()
                };
                
                let packet_info = extract_packet_info(entry);
                
                if let Some(flags) = &packet_info.flags {
                    if flags.reliable { reliable_count += 1; }
                    if flags.resent { resent_count += 1; }
                    if flags.zerocoded { zerocoded_count += 1; }
                    if flags.appended_acks { appended_acks_count += 1; }
                }
                
                // Count frequency and trust levels
                if let Some(freq) = &packet_info.frequency {
                    *frequency_counts.entry(freq.clone()).or_insert(0) += 1;
                }
                if let Some(trust) = &packet_info.trust_level {
                    *trust_counts.entry(trust.clone()).or_insert(0) += 1;
                }
                
                let flags_display = if let Some(flags) = &packet_info.flags {
                    format!(" [{}]", flags)
                } else {
                    " [-]".to_string()
                };
                
                let freq_trust_display = format!(" {}/{}",
                    packet_info.frequency.as_deref().unwrap_or("?"),
                    packet_info.trust_level.as_deref().unwrap_or("?")
                );
                
                let packet_id_display = if let Some(id) = packet_info.packet_id {
                    format!(" #{}", id)
                } else {
                    String::new()
                };
                
                println!("{}: [{}] {}{}{}{} - {}{}", 
                    i + 1,
                    entry.entry_type,
                    entry.meta.method,
                    flags_display,
                    freq_trust_display,
                    packet_id_display,
                    message_display,
                    entry.summary.chars().take(45).collect::<String>()
                );
            }
            
            println!();
            println!("=== Summary Statistics ===");
            println!("Reliable packets (need ACK): {} / {} ({:.1}%)", 
                reliable_count, lludp_entries.len(),
                reliable_count as f64 / lludp_entries.len() as f64 * 100.0);
            println!("Resent packets: {} / {} ({:.1}%)", 
                resent_count, lludp_entries.len(),
                resent_count as f64 / lludp_entries.len() as f64 * 100.0);
            println!("Zero-coded packets: {} / {} ({:.1}%)", 
                zerocoded_count, lludp_entries.len(),
                zerocoded_count as f64 / lludp_entries.len() as f64 * 100.0);
            println!("Packets with appended ACKs: {} / {} ({:.1}%)", 
                appended_acks_count, lludp_entries.len(),
                appended_acks_count as f64 / lludp_entries.len() as f64 * 100.0);
            
            println!();
            println!("=== Frequency Distribution ===");
            for (freq, count) in frequency_counts.iter() {
                println!("{}: {} packets ({:.1}%)", 
                    freq, count, 
                    *count as f64 / lludp_entries.len() as f64 * 100.0);
            }
            
            println!();
            println!("=== Trust Level Distribution ===");
            for (trust, count) in trust_counts.iter() {
                println!("{}: {} packets ({:.1}%)", 
                    trust, count, 
                    *count as f64 / lludp_entries.len() as f64 * 100.0);
            }
        }
        
        "http-summary" => {
            let http_entries = parser.http_entries();
            println!("HTTP Request Summary ({} entries):", http_entries.len());
            for (i, entry) in http_entries.iter().enumerate() {
                if let Some(flow) = entry.data.get("flow") {
                    let method = flow.get("request")
                        .and_then(|r| r.get("method"))
                        .and_then(|m| m.get("__bytes__"))
                        .and_then(|b| base64::prelude::Engine::decode(&base64::prelude::BASE64_STANDARD, b.as_str().unwrap_or("")).ok())
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                        .unwrap_or_else(|| entry.meta.method.clone());
                    
                    let host = flow.get("request")
                        .and_then(|r| r.get("host"))
                        .and_then(|h| h.as_str())
                        .unwrap_or("unknown");
                    
                    let path = flow.get("request")
                        .and_then(|r| r.get("path"))
                        .and_then(|p| p.get("__bytes__"))
                        .and_then(|b| base64::prelude::Engine::decode(&base64::prelude::BASE64_STANDARD, b.as_str().unwrap_or("")).ok())
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                        .unwrap_or_else(|| "/".to_string());
                    
                    let status = flow.get("response")
                        .and_then(|r| r.get("status_code"))
                        .and_then(|s| s.as_u64())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "???".to_string());
                    
                    println!("{}: {} {} {}{} -> {}", 
                        i + 1, 
                        method,
                        host,
                        path,
                        if path.len() > 50 { "..." } else { "" },
                        status
                    );
                    
                    // Show content type if available
                    if let Some(content_type) = flow.get("response")
                        .and_then(|r| r.get("headers"))
                        .and_then(|h| h.as_array())
                        .and_then(|headers| {
                            headers.iter().find(|header| {
                                header.as_array()
                                    .and_then(|h| h.first())
                                    .and_then(|k| k.get("__bytes__"))
                                    .and_then(|b| base64::prelude::Engine::decode(&base64::prelude::BASE64_STANDARD, b.as_str().unwrap_or("")).ok())
                                    .and_then(|bytes| String::from_utf8(bytes).ok())
                                    .map(|s| s.to_lowercase() == "content-type")
                                    .unwrap_or(false)
                            })
                        })
                        .and_then(|header| header.as_array())
                        .and_then(|h| h.get(1))
                        .and_then(|v| v.get("__bytes__"))
                        .and_then(|b| base64::prelude::Engine::decode(&base64::prelude::BASE64_STANDARD, b.as_str().unwrap_or("")).ok())
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                    {
                        if !content_type.is_empty() && content_type != "text/html; charset=utf-8" {
                            println!("    Content-Type: {}", content_type);
                        }
                    }
                }
            }
        }
        
        _ => {
            eprintln!("Unknown command: {}. Use 'stats', 'http', 'lludp', 'eq', 'grep', 'detail', 'list', 'flags', 'http-summary', or 'export'", command);
            return Ok(());
        }
    }

    Ok(())
}