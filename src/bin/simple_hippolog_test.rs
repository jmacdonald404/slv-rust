use anyhow::Result;
use std::fs::File;
use std::io::Read;

fn main() -> Result<()> {
    println!("Testing basic hippolog parsing...");
    
    // Test decompression first
    let file = File::open("oa.hippolog")?;
    let mut decoder = flate2::read::GzDecoder::new(file);
    let mut decompressed_data = Vec::new();
    let bytes_read = decoder.read_to_end(&mut decompressed_data)?;
    
    println!("Successfully decompressed {} bytes", bytes_read);
    println!("First 200 chars: {}", 
        String::from_utf8_lossy(&decompressed_data[..200.min(decompressed_data.len())]));
    
    // Find the structure
    let data_str = String::from_utf8_lossy(&decompressed_data);
    let lines: Vec<&str> = data_str.lines().take(5).collect();
    println!("First few lines:");
    for (i, line) in lines.iter().enumerate() {
        println!("  {}: {}", i + 1, &line[..100.min(line.len())]);
    }
    
    // Count entries (simple approach)
    let entry_count = data_str.matches("'type':").count();
    println!("Estimated entries: {}", entry_count);
    
    Ok(())
}