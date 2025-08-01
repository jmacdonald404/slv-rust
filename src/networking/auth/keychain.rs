//! Secure credential storage using the system keychain/keyring
//! 
//! # Security Notice
//! 
//! This module stores login credentials in the system's secure credential store:
//! - Windows: Windows Credential Manager
//! - macOS: macOS Keychain
//! - Linux: Secret Service (libsecret/GNOME Keyring)
//! 
//! ## Security Properties:
//! - Credentials are stored only locally on the user's machine
//! - Uses OS-level encryption and access controls
//! - Credentials are never written to disk in plaintext
//! - No credentials are sent over the network by this module
//! - Debug logs do not contain sensitive information
//! 
//! ## Important Security Notes:
//! - This module does NOT store credentials in version control
//! - This module does NOT write credentials to log files
//! - This module does NOT expose credentials in debug output
//! - Credentials are only accessible to the current user account
//! - The keyring service name "slv-rust" identifies stored credentials

use keyring::{Entry, Result as KeyringResult};
use tracing::{debug, warn, error};
use super::LoginCredentials;

pub struct CredentialStore {
    service_name: String,
}

impl CredentialStore {
    /// Create a new credential store
    /// 
    /// Uses "slv-rust" as the service name for keyring entries
    pub fn new() -> Self {
        Self {
            service_name: "slv-rust".to_string(),
        }
    }

    /// Store login credentials securely in the system keychain
    /// 
    /// # Security Notes:
    /// - Credentials are stored using the system's secure credential store
    /// - Only the current user can access these credentials
    /// - Passwords are encrypted by the OS keyring service
    /// - This method does NOT log sensitive information
    /// 
    /// # Storage Format:
    /// - Username: stored as "{grid_name}_username"
    /// - Password: stored as "{grid_name}_password"
    pub fn store_credentials(&self, credentials: &LoginCredentials) -> KeyringResult<()> {
        let username_key = format!("{}_username", credentials.grid.name().to_lowercase());
        let password_key = format!("{}_password", credentials.grid.name().to_lowercase());
        
        let username_entry = Entry::new(&self.service_name, &username_key)?;
        let password_entry = Entry::new(&self.service_name, &password_key)?;
        
        username_entry.set_password(&credentials.username)?;
        password_entry.set_password(&credentials.password)?;
        
        debug!("Successfully stored credentials for grid {}", credentials.grid.name());
        
        Ok(())
    }

    pub fn load_credentials(&self, grid_name: &str) -> KeyringResult<Option<LoginCredentials>> {
        let username_key = format!("{}_username", grid_name.to_lowercase());
        let password_key = format!("{}_password", grid_name.to_lowercase());
        
        let username_entry = Entry::new(&self.service_name, &username_key)?;
        let password_entry = Entry::new(&self.service_name, &password_key)?;
        
        match (username_entry.get_password(), password_entry.get_password()) {
            (Ok(username), Ok(password)) => {
                debug!("Successfully loaded credentials for grid {}", grid_name);
                let grid = super::Grid::from_name(grid_name).unwrap_or_default();
                Ok(Some(LoginCredentials::new(username, password).with_grid(grid)))
            }
            (Err(e1), Err(e2)) => {
                debug!("No stored credentials found for grid {}: username error: {}, password error: {}", 
                       grid_name, e1, e2);
                Ok(None)
            }
            (Ok(_), Err(e)) => {
                warn!("Found username but not password for grid {}: {}", grid_name, e);
                Ok(None)
            }
            (Err(e), Ok(_)) => {
                warn!("Found password but not username for grid {}: {}", grid_name, e);
                Ok(None)
            }
        }
    }

    pub fn delete_credentials(&self, grid_name: &str) -> KeyringResult<()> {
        let username_key = format!("{}_username", grid_name.to_lowercase());
        let password_key = format!("{}_password", grid_name.to_lowercase());
        
        let username_entry = Entry::new(&self.service_name, &username_key)?;
        let password_entry = Entry::new(&self.service_name, &password_key)?;
        
        let username_result = username_entry.delete_credential();
        let password_result = password_entry.delete_credential();
        
        match (username_result, password_result) {
            (Ok(()), Ok(())) => {
                debug!("Successfully deleted credentials for grid {}", grid_name);
                Ok(())
            }
            (Err(e1), Err(e2)) => {
                error!("Failed to delete credentials for grid {}: username error: {}, password error: {}", 
                       grid_name, e1, e2);
                Err(e1) // Return the first error
            }
            (Ok(()), Err(e)) => {
                warn!("Deleted username but failed to delete password for grid {}: {}", grid_name, e);
                Err(e)
            }
            (Err(e), Ok(())) => {
                warn!("Deleted password but failed to delete username for grid {}: {}", grid_name, e);
                Err(e)
            }
        }
    }

    pub fn has_stored_credentials(&self, grid_name: &str) -> bool {
        match self.load_credentials(grid_name) {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                debug!("Error checking for stored credentials for grid {}: {}", grid_name, e);
                false
            }
        }
    }
}

impl Default for CredentialStore {
    fn default() -> Self {
        Self::new()
    }
}