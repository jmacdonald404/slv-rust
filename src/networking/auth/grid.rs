use serde::{Deserialize, Serialize};

/// SecondLife grid configurations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grid {
    /// Main SecondLife grid (Agni)
    SecondLife,
    /// SecondLife beta grid (Aditi)
    SecondLifeBeta,
    /// OpenSimulator grid
    OpenSimulator { 
        name: String, 
        login_uri: String 
    },
    /// Custom grid
    Custom { 
        name: String, 
        login_uri: String 
    },
}

impl Grid {
    pub fn name(&self) -> &str {
        match self {
            Grid::SecondLife => "Second Life",
            Grid::SecondLifeBeta => "Second Life Beta",
            Grid::OpenSimulator { name, .. } => name,
            Grid::Custom { name, .. } => name,
        }
    }

    pub fn login_uri(&self) -> &str {
        match self {
            Grid::SecondLife => "https://login.agni.lindenlab.com/cgi-bin/login.cgi",
            Grid::SecondLifeBeta => "https://login.aditi.lindenlab.com/cgi-bin/login.cgi",
            Grid::OpenSimulator { login_uri, .. } => login_uri,
            Grid::Custom { login_uri, .. } => login_uri,
        }
    }

    pub fn is_official(&self) -> bool {
        matches!(self, Grid::SecondLife | Grid::SecondLifeBeta)
    }

    /// Get the default grid (SecondLife main grid)
    pub fn default() -> Self {
        Grid::SecondLife
    }

    /// Parse grid from name string
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "secondlife" | "sl" | "agni" => Some(Grid::SecondLife),
            "secondlife-beta" | "sl-beta" | "aditi" => Some(Grid::SecondLifeBeta),
            _ => None,
        }
    }

    /// Create a custom OpenSimulator grid
    pub fn opensim(name: String, login_uri: String) -> Self {
        Grid::OpenSimulator { name, login_uri }
    }

    /// Create a custom grid
    pub fn custom(name: String, login_uri: String) -> Self {
        Grid::Custom { name, login_uri }
    }

    /// Get all predefined grids
    pub fn predefined() -> Vec<Self> {
        vec![
            Grid::SecondLife,
            Grid::SecondLifeBeta,
        ]
    }
}

/// Get all available grids (alias for Grid::predefined)
pub fn available_grids() -> Vec<Grid> {
    Grid::predefined()
}

impl Default for Grid {
    fn default() -> Self {
        Grid::SecondLife
    }
}

impl std::fmt::Display for Grid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_name() {
        assert_eq!(Grid::SecondLife.name(), "Second Life");
        assert_eq!(Grid::SecondLifeBeta.name(), "Second Life Beta");
    }

    #[test]
    fn test_grid_login_uri() {
        assert_eq!(
            Grid::SecondLife.login_uri(),
            "https://login.agni.lindenlab.com/cgi-bin/login.cgi"
        );
        assert_eq!(
            Grid::SecondLifeBeta.login_uri(),
            "https://login.aditi.lindenlab.com/cgi-bin/login.cgi"
        );
    }

    #[test]
    fn test_grid_from_name() {
        assert_eq!(Grid::from_name("secondlife"), Some(Grid::SecondLife));
        assert_eq!(Grid::from_name("sl"), Some(Grid::SecondLife));
        assert_eq!(Grid::from_name("agni"), Some(Grid::SecondLife));
        assert_eq!(Grid::from_name("unknown"), None);
    }

    #[test]
    fn test_custom_grid() {
        let grid = Grid::opensim(
            "OSGrid".to_string(),
            "http://login.osgrid.org/".to_string()
        );
        assert_eq!(grid.name(), "OSGrid");
        assert_eq!(grid.login_uri(), "http://login.osgrid.org/");
        assert!(!grid.is_official());
    }
}