use anyhow::Result;
use std::fs::File;
use std::io::Read;
use std::process::{Command, Stdio};
use std::io::Write;

fn main() -> Result<()> {
    println!("=== Hippolog Debug Tool ===");
    
    // Step 1: Decompress the file
    println!("Step 1: Decompressing oa.hippolog...");
    let file = File::open("oa.hippolog")?;
    let mut decoder = flate2::read::GzDecoder::new(file);
    let mut decompressed_data = String::new();
    decoder.read_to_string(&mut decompressed_data)?;
    
    println!("Decompressed {} characters", decompressed_data.len());
    
    // Step 2: Show structure
    println!("\nStep 2: Analyzing structure...");
    println!("First 200 characters:");
    println!("{}", &decompressed_data[..200.min(decompressed_data.len())]);
    
    println!("\nLast 200 characters:");
    let start = decompressed_data.len().saturating_sub(200);
    println!("{}", &decompressed_data[start..]);
    
    // Step 3: Try Python parsing
    println!("\nStep 3: Testing Python parsing...");
    
    let python_script = r#"
import ast
import json
import sys

try:
    data = sys.stdin.read()
    print(f"Python received {len(data)} characters", file=sys.stderr)
    
    # Parse with ast.literal_eval
    parsed = ast.literal_eval(data)
    print(f"Successfully parsed {len(parsed)} entries", file=sys.stderr)
    
    # Convert first entry to JSON for testing
    first_entry_json = json.dumps(parsed[0])
    print("First entry JSON:")
    print(first_entry_json)
    
    print(f"\nEntry types found: {set(entry.get('type', 'unknown') for entry in parsed)}", file=sys.stderr)
    
except Exception as e:
    print(f"Python parsing error: {e}", file=sys.stderr)
    sys.exit(1)
"#;
    
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(python_script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(decompressed_data.as_bytes())?;
    }
    
    let output = child.wait_with_output()?;
    
    println!("Python stdout:");
    println!("{}", String::from_utf8_lossy(&output.stdout));
    
    println!("Python stderr:");
    println!("{}", String::from_utf8_lossy(&output.stderr));
    
    println!("Exit status: {}", output.status);
    
    Ok(())
}