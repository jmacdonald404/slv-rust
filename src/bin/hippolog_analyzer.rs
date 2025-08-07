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
            for (i, entry) in entries.iter().enumerate() {
                let message_name = extract_message_name(entry);
                let message_display = if let Some(name) = message_name {
                    format!("({}) ", name)
                } else {
                    String::new()
                };
                
                println!("{}: [{}] {} - {}{}", 
                    i + 1, 
                    entry.entry_type,
                    entry.meta.method,
                    message_display,
                    entry.summary.chars().take(80).collect::<String>()
                );
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
            eprintln!("Unknown command: {}. Use 'stats', 'http', 'lludp', 'eq', 'grep', 'detail', 'list', 'http-summary', or 'export'", command);
            return Ok(());
        }
    }

    Ok(())
}