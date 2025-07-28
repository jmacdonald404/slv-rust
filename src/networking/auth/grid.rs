#[derive(Debug, Clone, PartialEq)]
pub enum Grid {
    SecondLifeMain,
    SecondLifeBeta,
    OpenSimulator(String),
}

impl Grid {
    pub fn name(&self) -> &str {
        match self {
            Grid::SecondLifeMain => "Second Life Main Grid",
            Grid::SecondLifeBeta => "Second Life Beta Grid", 
            Grid::OpenSimulator(name) => name,
        }
    }
    
    pub fn login_uri(&self) -> &str {
        match self {
            Grid::SecondLifeMain => "https://login.agni.lindenlab.com/cgi-bin/login.cgi",
            Grid::SecondLifeBeta => "https://login.aditi.lindenlab.com/cgi-bin/login.cgi",
            Grid::OpenSimulator(_) => "http://localhost:8002/", // Default OpenSim
        }
    }
    
    pub fn default_simulator_port(&self) -> u16 {
        match self {
            Grid::SecondLifeMain | Grid::SecondLifeBeta => 9000,
            Grid::OpenSimulator(_) => 9000,
        }
    }
}

impl Default for Grid {
    fn default() -> Self {
        Grid::SecondLifeMain
    }
}

pub fn available_grids() -> Vec<Grid> {
    vec![
        Grid::SecondLifeMain,
        Grid::SecondLifeBeta,
        Grid::OpenSimulator("Local OpenSim".to_string()),
    ]
}