// Temporary test module to debug keychain issues
use super::{CredentialStore, LoginCredentials, Grid};

pub fn test_keychain() {
    println!("=== KEYCHAIN DEBUG TEST ===");
    
    let store = CredentialStore::new();
    println!("CredentialStore created successfully");
    
    let test_credentials = LoginCredentials::new(
        "TestUser".to_string(),
        "TestPassword".to_string(),
    ).with_grid(Grid::SecondLife);
    
    println!("Test credentials created");
    
    match store.store_credentials(&test_credentials) {
        Ok(()) => {
            println!("✅ Successfully stored test credentials");
            
            // Try to retrieve them
            match store.load_credentials("second life") {
                Ok(Some(loaded)) => {
                    println!("✅ Successfully loaded credentials: username={}", loaded.username);
                }
                Ok(None) => {
                    println!("❌ No credentials found when loading");
                }
                Err(e) => {
                    println!("❌ Error loading credentials: {}", e);
                }
            }
            
            // Clean up
            if store.delete_stored_credentials("second life") {
                println!("✅ Successfully deleted test credentials");
            } else {
                println!("❌ Failed to delete test credentials");
            }
        }
        Err(e) => {
            println!("❌ Error storing credentials: {}", e);
        }
    }
    
    println!("=== KEYCHAIN DEBUG TEST COMPLETE ===");
}