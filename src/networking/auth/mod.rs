//! Authentication module for SecondLife/OpenSimulator grids
//! 
//! This module provides secure authentication with SecondLife login servers
//! using XML-RPC protocol, session management, and grid configuration.

mod grid;
mod keychain;
mod login;
mod session;
mod types;
mod xmlrpc;

pub use grid::{Grid, available_grids};
pub use keychain::CredentialStore;
pub use login::{AuthenticationService, LoginCredentials};
pub use session::{SessionInfo, SessionManager};
pub use types::*;

// Re-export for convenience
pub use xmlrpc::{XmlRpcClient, LoginParameters};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_validation() {
        let credentials = LoginCredentials::new(
            "Test User".to_string(),
            "password".to_string()
        );
        assert!(credentials.validate().is_ok());

        let invalid_credentials = LoginCredentials::new(
            "".to_string(),
            "password".to_string()
        );
        assert!(invalid_credentials.validate().is_err());
    }

    #[test]
    fn test_name_splitting() {
        let credentials = LoginCredentials::new(
            "John Doe".to_string(),
            "password".to_string()
        );
        let (first, last) = credentials.split_name();
        assert_eq!(first, "John");
        assert_eq!(last, "Doe");

        let single_name = LoginCredentials::new(
            "John".to_string(),
            "password".to_string()
        );
        let (first, last) = single_name.split_name();
        assert_eq!(first, "John");
        assert_eq!(last, "Resident");
    }

    #[test]
    fn test_session_manager() {
        let mut manager = SessionManager::new();
        assert!(!manager.is_logged_in());
        assert!(manager.current_session().is_none());

        let session = SessionInfo {
            agent_id: uuid::Uuid::new_v4(),
            session_id: uuid::Uuid::new_v4(),
            secure_session_id: uuid::Uuid::new_v4(),
            first_name: "Test".to_string(),
            last_name: "User".to_string(),
            circuit_code: 12345,
            simulator_address: "127.0.0.1:9000".parse().unwrap(),
            look_at: crate::utils::math::Vector3::new(1.0, 0.0, 0.0).to_array(),
            start_location: "last".to_string(),
        };

        manager.start_session(session.clone());
        assert!(manager.is_logged_in());
        assert_eq!(manager.agent_id(), Some(session.agent_id));

        manager.end_session();
        assert!(!manager.is_logged_in());
    }
}

#[cfg(test)]
mod comprehensive_tests;