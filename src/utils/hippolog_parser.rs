use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize, Deserializer};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Parser for Hippolyzer .hippolog files
/// 
/// These files contain gzip-compressed Python literal data representing
/// SecondLife protocol message logs. The format is:
/// gzip(str(list_of_dicts))
///
/// Reference implementation: hippolyzer/lib/proxy/message_logger.py
#[derive(Debug, Clone)]
pub struct HippologParser {
    entries: Vec<LogEntry>,
}

/// Represents a log entry from a hippolog file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub region_name: String,
    pub agent_id: Option<String>,
    pub summary: String,
    pub meta: LogEntryMeta,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryMeta {
    #[serde(rename = "RegionName")]
    pub region_name: String,
    #[serde(rename = "AgentID")]
    pub agent_id: Option<String>,
    #[serde(rename = "SessionID")]
    pub session_id: Option<String>,
    #[serde(rename = "AgentLocal")]
    pub agent_local: Option<u32>,
    #[serde(rename = "Method")]
    pub method: String,
    #[serde(rename = "Type")]
    pub entry_type: String,
    #[serde(rename = "SelectedLocal")]
    pub selected_local: Option<u32>,
    #[serde(rename = "SelectedFull")]
    pub selected_full: Option<String>,
    #[serde(rename = "Synthetic", default)]
    pub synthetic: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}


/// Represents a match found during grep search
#[derive(Debug, Clone)]
pub struct HippologMatch<'a> {
    pub entry_index: usize,
    pub entry: &'a LogEntry,
    pub match_locations: Vec<HippologMatchLocation>,
}

/// Locations where a match was found in a log entry
#[derive(Debug, Clone, PartialEq)]
pub enum HippologMatchLocation {
    Summary,
    RegionName,
    EntryType,
    Method,
    AgentId,
    Data,
}

impl std::fmt::Display for HippologMatchLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Summary => write!(f, "summary"),
            Self::RegionName => write!(f, "region"),
            Self::EntryType => write!(f, "type"),
            Self::Method => write!(f, "method"),
            Self::AgentId => write!(f, "agent_id"),
            Self::Data => write!(f, "data"),
        }
    }
}

impl HippologParser {
    /// Create a new empty parser
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Parse a hippolog file from a path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref())
            .with_context(|| format!("Failed to open hippolog file: {}", path.as_ref().display()))?;
        
        Self::from_reader(file)
    }

    /// Parse a hippolog file from any reader
    pub fn from_reader<R: Read>(reader: R) -> Result<Self> {
        let mut decoder = flate2::read::GzDecoder::new(reader);
        let mut decompressed_data = String::new();
        decoder.read_to_string(&mut decompressed_data)
            .context("Failed to decompress hippolog file")?;

        Self::from_python_literal(&decompressed_data)
    }

    /// Parse from decompressed Python literal string
    pub fn from_python_literal(data: &str) -> Result<Self> {
        // Try the simple conversion first
        if let Ok(json_data) = Self::python_to_json(data) {
            if let Ok(entries) = serde_json::from_str::<Vec<LogEntry>>(&json_data) {
                return Ok(Self { entries });
            }
        }
        
        // If that fails, try a more robust approach using Python subprocess
        Self::parse_with_python_fallback(data)
    }
    
    /// Fallback parsing using Python's ast.literal_eval via subprocess
    fn parse_with_python_fallback(data: &str) -> Result<Self> {
        use std::process::{Command, Stdio};
        use std::io::Write;
        
        let mut child = Command::new("python3")
            .arg("-c")
            .arg("
import ast
import json
import sys
import base64

def convert_bytes(obj):
    '''Convert bytes objects to base64 strings for JSON serialization'''
    if isinstance(obj, bytes):
        return {'__bytes__': base64.b64encode(obj).decode('ascii')}
    elif isinstance(obj, dict):
        return {k: convert_bytes(v) for k, v in obj.items()}
    elif isinstance(obj, list):
        return [convert_bytes(item) for item in obj]
    elif isinstance(obj, tuple):
        return [convert_bytes(item) for item in obj]
    else:
        return obj

data = sys.stdin.read()
try:
    parsed = ast.literal_eval(data)
    converted = convert_bytes(parsed)
    print(json.dumps(converted))
except Exception as e:
    print(f'ERROR: {e}', file=sys.stderr)
    sys.exit(1)
")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn Python process for parsing")?;
        
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(data.as_bytes())
                .context("Failed to write data to Python process")?;
        }
        
        let output = child.wait_with_output()
            .context("Failed to read output from Python process")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Python parsing failed: {}", stderr));
        }
        
        let json_data = String::from_utf8(output.stdout)
            .context("Python output was not valid UTF-8")?;
        
        let entries: Vec<LogEntry> = serde_json::from_str(&json_data)
            .context("Failed to parse Python-converted JSON")?;
        
        Ok(Self { entries })
    }

    /// Convert Python literal to JSON (simplified approach)
    /// This handles the most common cases but may need extension for edge cases
    fn python_to_json(python_data: &str) -> Result<String> {
        let mut json_data = python_data.to_string();

        // Replace Python-specific literals with JSON equivalents
        json_data = json_data.replace("True", "true");
        json_data = json_data.replace("False", "false");
        json_data = json_data.replace("None", "null");

        // Handle Python single quotes (basic approach - doesn't handle nested quotes)
        json_data = Self::fix_quotes(&json_data);

        Ok(json_data)
    }

    /// Fix Python single quotes to JSON double quotes
    /// This is a more robust implementation that handles escaping
    fn fix_quotes(data: &str) -> String {
        let mut result = String::with_capacity(data.len() * 2);
        let mut chars = data.chars().peekable();
        let mut in_string = false;
        let mut string_delimiter = '"';
        let mut escaped = false;

        while let Some(ch) = chars.next() {
            if escaped {
                // Previous character was a backslash
                result.push(ch);
                escaped = false;
                continue;
            }
            
            match ch {
                '\\' => {
                    result.push(ch);
                    escaped = true;
                }
                '\'' | '"' if !in_string => {
                    // Start of a string
                    in_string = true;
                    string_delimiter = ch;
                    result.push('"'); // Always use double quotes in JSON
                }
                ch if in_string && ch == string_delimiter => {
                    // End of string
                    in_string = false;
                    result.push('"');
                }
                '"' if in_string && string_delimiter == '\'' => {
                    // Double quote inside single-quoted string - escape it
                    result.push('\\');
                    result.push('"');
                }
                ch => {
                    result.push(ch);
                }
            }
        }

        result
    }

    /// Get all log entries
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// Filter entries by type
    pub fn filter_by_type(&self, entry_type: &str) -> Vec<&LogEntry> {
        self.entries.iter()
            .filter(|entry| entry.entry_type == entry_type)
            .collect()
    }

    /// Get HTTP entries only
    pub fn http_entries(&self) -> Vec<&LogEntry> {
        self.filter_by_type("HTTP")
    }

    /// Get LLUDP entries only
    pub fn lludp_entries(&self) -> Vec<&LogEntry> {
        self.filter_by_type("LLUDP")
    }

    /// Get EQ entries only
    pub fn eq_entries(&self) -> Vec<&LogEntry> {
        self.filter_by_type("EQ")
    }

    /// Get entries by region name
    pub fn filter_by_region(&self, region_name: &str) -> Vec<&LogEntry> {
        self.entries.iter()
            .filter(|entry| entry.region_name == region_name)
            .collect()
    }

    /// Get detailed packet information for a specific entry
    pub fn get_packet_details(&self, entry_index: usize, beautify: bool) -> Result<String> {
        if entry_index >= self.entries.len() {
            return Err(anyhow::anyhow!("Entry index {} out of range", entry_index));
        }
        
        let entry = &self.entries[entry_index];
        let mut details = String::new();
        
        details.push_str(&format!("=== Entry #{} ===\n", entry_index));
        details.push_str(&format!("Type: {}\n", entry.entry_type));
        details.push_str(&format!("Method: {}\n", entry.meta.method));
        details.push_str(&format!("Region: {}\n", entry.region_name));
        if let Some(agent_id) = &entry.agent_id {
            details.push_str(&format!("Agent ID: {}\n", agent_id));
        }
        details.push_str(&format!("Summary: {}\n", entry.summary));
        details.push_str("\n=== Metadata ===\n");
        details.push_str(&format!("Session ID: {:?}\n", entry.meta.session_id));
        details.push_str(&format!("Agent Local: {:?}\n", entry.meta.agent_local));
        details.push_str(&format!("Synthetic: {}\n", entry.meta.synthetic));
        
        details.push_str("\n=== Data ===\n");
        if beautify {
            details.push_str(&serde_json::to_string_pretty(&entry.data)?);
        } else {
            details.push_str(&serde_json::to_string(&entry.data)?);
        }
        
        // Add decoded bytes section if there are any
        let decoded_bytes = Self::extract_and_decode_bytes(&entry.data);
        if !decoded_bytes.is_empty() {
            details.push_str("\n=== Decoded Bytes ===\n");
            for (path, decoded) in decoded_bytes {
                details.push_str(&format!("{}: {}\n", path, decoded));
            }
        }
        
        Ok(details)
    }
    
    /// Extract and decode base64-encoded bytes from the data structure
    fn extract_and_decode_bytes(value: &Value) -> Vec<(String, String)> {
        let mut decoded_bytes = Vec::new();
        Self::extract_bytes_recursive(value, String::new(), &mut decoded_bytes);
        decoded_bytes
    }
    
    /// Recursively search for __bytes__ encoded data and decode it
    fn extract_bytes_recursive(value: &Value, path: String, results: &mut Vec<(String, String)>) {
        match value {
            Value::Object(map) => {
                // Check if this is a bytes-encoded object
                if let Some(Value::String(b64_data)) = map.get("__bytes__") {
                    match general_purpose::STANDARD.decode(b64_data) {
                        Ok(bytes_data) => {
                            // Try to decode as different types
                            let decoded = Self::analyze_bytes_data(&bytes_data);
                            results.push((path, decoded));
                        }
                        Err(e) => {
                            results.push((path, format!("Base64 decode error: {}", e)));
                        }
                    }
                } else {
                    // Recursively search nested objects
                    for (key, nested_value) in map {
                        let new_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{}.{}", path, key)
                        };
                        Self::extract_bytes_recursive(nested_value, new_path, results);
                    }
                }
            }
            Value::Array(arr) => {
                // Search array elements
                for (index, item) in arr.iter().enumerate() {
                    let new_path = format!("{}[{}]", path, index);
                    Self::extract_bytes_recursive(item, new_path, results);
                }
            }
            _ => {
                // Other types don't contain nested data to search
            }
        }
    }
    
    /// Analyze bytes data and provide multiple interpretations
    fn analyze_bytes_data(bytes_data: &[u8]) -> String {
        let mut analyses = Vec::new();
        
        // Basic info
        analyses.push(format!("Length: {} bytes", bytes_data.len()));
        
        // Try UTF-8 decoding
        if let Ok(utf8_str) = String::from_utf8(bytes_data.to_vec()) {
            if utf8_str.chars().all(|c| c.is_ascii_graphic() || c.is_ascii_whitespace()) {
                analyses.push(format!("UTF-8: \"{}\"", utf8_str));
            } else if utf8_str.len() <= 200 {
                // Show partial UTF-8 with escape sequences for non-printable chars
                let escaped = utf8_str.chars()
                    .map(|c| if c.is_control() { format!("\\x{:02x}", c as u8) } else { c.to_string() })
                    .collect::<String>();
                analyses.push(format!("UTF-8: \"{}\"", escaped));
            } else {
                analyses.push(format!("UTF-8: {} chars (truncated: \"{}...\")", 
                    utf8_str.chars().count(),
                    utf8_str.chars().take(50).collect::<String>()
                ));
            }
        } else {
            analyses.push("UTF-8: Invalid".to_string());
        }
        
        // Hex dump (first 32 bytes)
        let hex_dump = bytes_data.iter()
            .take(32)
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");
        let hex_suffix = if bytes_data.len() > 32 { "..." } else { "" };
        analyses.push(format!("Hex: {}{}", hex_dump, hex_suffix));
        
        // Try to detect common patterns
        if bytes_data.len() >= 4 {
            // Check for common binary signatures
            let first_four = &bytes_data[0..4];
            match first_four {
                [0xFF, 0xD8, 0xFF, _] => analyses.push("Detected: JPEG image".to_string()),
                [0x89, 0x50, 0x4E, 0x47] => analyses.push("Detected: PNG image".to_string()),
                [0x50, 0x4B, 0x03, 0x04] | [0x50, 0x4B, 0x05, 0x06] => analyses.push("Detected: ZIP archive".to_string()),
                [0x00, 0x00, 0x00, _] if bytes_data.len() >= 8 => {
                    // Could be integer data
                    let as_u32 = u32::from_le_bytes([bytes_data[0], bytes_data[1], bytes_data[2], bytes_data[3]]);
                    analyses.push(format!("As LE u32: {}", as_u32));
                },
                _ => {}
            }
        }
        
        // Check if all bytes are printable ASCII
        if bytes_data.iter().all(|&b| b >= 32 && b <= 126) {
            analyses.push("Note: All bytes are printable ASCII".to_string());
        }
        
        analyses.join(" | ")
    }

    /// Get a compact summary of an entry for listing purposes
    pub fn get_entry_summary(&self, entry_index: usize) -> Result<String> {
        if entry_index >= self.entries.len() {
            return Err(anyhow::anyhow!("Entry index {} out of range", entry_index));
        }
        
        let entry = &self.entries[entry_index];
        let timestamp = entry.data.get("flow")
            .and_then(|flow| flow.get("timestamp"))
            .and_then(|t| t.as_str())
            .or_else(|| entry.data.get("timestamp").and_then(|t| t.as_str()))
            .unwrap_or("unknown");
        
        Ok(format!("#{}: [{}] {} {} - {} | Region: {} | Time: {}", 
            entry_index,
            entry.entry_type,
            entry.meta.method,
            entry.agent_id.as_deref().unwrap_or("no-agent"),
            entry.summary.chars().take(50).collect::<String>(),
            entry.region_name,
            timestamp
        ))
    }

    /// Search through log entries (grep-like functionality)
    pub fn grep(&self, pattern: &str, case_sensitive: bool) -> Vec<HippologMatch> {
        let pattern_lower = if case_sensitive { pattern.to_string() } else { pattern.to_lowercase() };
        let mut matches = Vec::new();

        for (index, entry) in self.entries.iter().enumerate() {
            let mut entry_matches = Vec::new();
            
            // Search in summary
            if self.text_contains(&entry.summary, &pattern_lower, case_sensitive) {
                entry_matches.push(HippologMatchLocation::Summary);
            }
            
            // Search in region name
            if self.text_contains(&entry.region_name, &pattern_lower, case_sensitive) {
                entry_matches.push(HippologMatchLocation::RegionName);
            }
            
            // Search in entry type
            if self.text_contains(&entry.entry_type, &pattern_lower, case_sensitive) {
                entry_matches.push(HippologMatchLocation::EntryType);
            }
            
            // Search in method
            if self.text_contains(&entry.meta.method, &pattern_lower, case_sensitive) {
                entry_matches.push(HippologMatchLocation::Method);
            }
            
            // Search in agent ID
            if let Some(agent_id) = &entry.agent_id {
                if self.text_contains(agent_id, &pattern_lower, case_sensitive) {
                    entry_matches.push(HippologMatchLocation::AgentId);
                }
            }
            
            // Search in data content (convert to JSON string for searching)
            let data_str = serde_json::to_string(&entry.data).unwrap_or_default();
            if self.text_contains(&data_str, &pattern_lower, case_sensitive) {
                entry_matches.push(HippologMatchLocation::Data);
            }
            
            if !entry_matches.is_empty() {
                matches.push(HippologMatch {
                    entry_index: index,
                    entry,
                    match_locations: entry_matches,
                });
            }
        }
        
        matches
    }
    
    /// Helper function for case-sensitive/insensitive text matching
    fn text_contains(&self, text: &str, pattern: &str, case_sensitive: bool) -> bool {
        if case_sensitive {
            text.contains(pattern)
        } else {
            text.to_lowercase().contains(pattern)
        }
    }

    /// Get summary statistics
    pub fn stats(&self) -> HippologStats {
        let mut stats = HippologStats::default();
        
        for entry in &self.entries {
            stats.total_entries += 1;
            match entry.entry_type.as_str() {
                "HTTP" => stats.http_entries += 1,
                "LLUDP" => stats.lludp_entries += 1,
                "EQ" => stats.eq_entries += 1,
                _ => stats.other_entries += 1,
            }
            
            if !entry.region_name.is_empty() {
                *stats.regions.entry(entry.region_name.clone()).or_insert(0) += 1;
            }
        }
        
        stats
    }

    /// Export entries back to hippolog format
    pub fn export_to_hippolog(&self) -> Result<Vec<u8>> {
        // Convert back to Python literal format
        let json_str = serde_json::to_string(&self.entries)
            .context("Failed to serialize entries to JSON")?;
        
        let python_str = Self::json_to_python(&json_str)?;
        
        // Compress with gzip
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(python_str.as_bytes())
            .context("Failed to write to gzip encoder")?;
        
        encoder.finish().context("Failed to finish gzip compression")
    }

    /// Convert JSON back to Python literal format
    fn json_to_python(json_data: &str) -> Result<String> {
        let mut python_data = json_data.to_string();
        
        python_data = python_data.replace("true", "True");
        python_data = python_data.replace("false", "False");
        python_data = python_data.replace("null", "None");
        
        Ok(python_data)
    }
}

#[derive(Debug, Default)]
pub struct HippologStats {
    pub total_entries: usize,
    pub http_entries: usize,
    pub lludp_entries: usize,
    pub eq_entries: usize,
    pub other_entries: usize,
    pub regions: HashMap<String, usize>,
}

impl std::fmt::Display for HippologStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Hippolog Statistics:")?;
        writeln!(f, "  Total entries: {}", self.total_entries)?;
        writeln!(f, "  HTTP entries: {}", self.http_entries)?;
        writeln!(f, "  LLUDP entries: {}", self.lludp_entries)?;
        writeln!(f, "  EQ entries: {}", self.eq_entries)?;
        if self.other_entries > 0 {
            writeln!(f, "  Other entries: {}", self.other_entries)?;
        }
        if !self.regions.is_empty() {
            writeln!(f, "  Regions:")?;
            for (region, count) in &self.regions {
                if !region.is_empty() {
                    writeln!(f, "    {}: {}", region, count)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_to_json_conversion() {
        let python_data = r#"[{'type': 'HTTP', 'region_name': '', 'synthetic': True, 'agent_id': None}]"#;
        let json_data = HippologParser::python_to_json(python_data).unwrap();
        let expected = r#"[{"type": "HTTP", "region_name": "", "synthetic": true, "agent_id": null}]"#;
        assert_eq!(json_data, expected);
    }

    #[test]
    fn test_quote_fixing() {
        let input = "{'key': 'value', \"other\": \"test\"}";
        let output = HippologParser::fix_quotes(input);
        let expected = "{\"key\": \"value\", \"other\": \"test\"}";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_grep_functionality() {
        use super::*;
        
        // Create a test parser with some mock entries
        let mut parser = HippologParser::new();
        // Note: This would need actual LogEntry instances for a full test
        // For now, just test the helper function
        assert!(parser.text_contains("Hello World", "world", false));
        assert!(!parser.text_contains("Hello World", "world", true));
        assert!(parser.text_contains("Hello World", "World", true));
    }
}