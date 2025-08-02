//! State machine for region crossing operations
//!
//! Implements a finite state machine to track the complex process of
//! moving between regions in Second Life.

use crate::networking::{NetworkResult, NetworkError};
use super::SimulatorInfo;
use tracing::{debug, info};

/// States in the region crossing process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionCrossingState {
    /// Not currently crossing regions
    Idle,
    /// Connecting to the new simulator
    Connecting,
    /// Moving the agent to the new region
    MovingAgent,
    /// Successfully connected to new region
    Connected,
    /// Region crossing failed
    Failed,
}

impl Default for RegionCrossingState {
    fn default() -> Self {
        RegionCrossingState::Idle
    }
}

impl std::fmt::Display for RegionCrossingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegionCrossingState::Idle => write!(f, "Idle"),
            RegionCrossingState::Connecting => write!(f, "Connecting"),
            RegionCrossingState::MovingAgent => write!(f, "MovingAgent"),
            RegionCrossingState::Connected => write!(f, "Connected"),
            RegionCrossingState::Failed => write!(f, "Failed"),
        }
    }
}

/// Events that drive the region crossing state machine
#[derive(Debug, Clone)]
pub enum RegionCrossingEvent {
    /// Initiate a crossing to a new region
    InitiateCrossing {
        simulator_info: SimulatorInfo,
    },
    /// EnableSimulator packet received
    EnableSimulatorReceived {
        simulator_info: SimulatorInfo,
    },
    /// Connection to new simulator established
    ConnectionEstablished {
        region_handle: u64,
    },
    /// Agent movement to new region completed
    MovementCompleted {
        region_handle: u64,
    },
    /// Region crossing failed
    CrossingFailed {
        region_handle: u64,
        error: String,
    },
}

/// Region crossing state machine
#[derive(Debug)]
pub struct RegionCrossingStateMachine {
    /// Current state
    current_state: RegionCrossingState,
    /// Previous state for rollback
    previous_state: Option<RegionCrossingState>,
    /// Number of state transitions
    transition_count: u64,
    /// Time when current state was entered
    state_entered_at: std::time::Instant,
}

impl RegionCrossingStateMachine {
    /// Create a new state machine in idle state
    pub fn new() -> Self {
        Self {
            current_state: RegionCrossingState::Idle,
            previous_state: None,
            transition_count: 0,
            state_entered_at: std::time::Instant::now(),
        }
    }
    
    /// Get the current state
    pub fn current_state(&self) -> RegionCrossingState {
        self.current_state
    }
    
    /// Get the previous state
    pub fn previous_state(&self) -> Option<RegionCrossingState> {
        self.previous_state
    }
    
    /// Get the time spent in current state
    pub fn time_in_current_state(&self) -> std::time::Duration {
        self.state_entered_at.elapsed()
    }
    
    /// Get the total number of state transitions
    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }
    
    /// Check if a state transition is valid
    pub fn can_transition_to(&self, new_state: RegionCrossingState) -> bool {
        use RegionCrossingState::*;
        
        match (self.current_state, new_state) {
            // From Idle
            (Idle, Connecting) => true,
            (Idle, Failed) => true,
            
            // From Connecting
            (Connecting, MovingAgent) => true,
            (Connecting, Failed) => true,
            (Connecting, Idle) => true, // Abort crossing
            
            // From MovingAgent
            (MovingAgent, Connected) => true,
            (MovingAgent, Failed) => true,
            (MovingAgent, Idle) => true, // Abort crossing
            
            // From Connected
            (Connected, Idle) => true, // Normal operation
            (Connected, Connecting) => true, // New crossing
            (Connected, Failed) => true,
            
            // From Failed
            (Failed, Idle) => true, // Reset
            (Failed, Connecting) => true, // Retry
            
            // Self-transitions (allowed for updates)
            (state, new_state) if state == new_state => true,
            
            // All other transitions are invalid
            _ => false,
        }
    }
    
    /// Transition to a new state
    pub fn transition_to(&mut self, new_state: RegionCrossingState) -> NetworkResult<()> {
        if !self.can_transition_to(new_state) {
            return Err(NetworkError::Other {
                reason: format!(
                    "Invalid state transition from {} to {}",
                    self.current_state, new_state
                )
            });
        }
        
        let old_state = self.current_state;
        
        debug!("🌍 State transition: {} -> {} (transition #{})",
               old_state, new_state, self.transition_count + 1);
        
        // Update state
        self.previous_state = Some(old_state);
        self.current_state = new_state;
        self.transition_count += 1;
        self.state_entered_at = std::time::Instant::now();
        
        // Log important transitions
        match new_state {
            RegionCrossingState::Connecting => {
                info!("🌍 Starting region crossing");
            },
            RegionCrossingState::Connected => {
                info!("🌍 Region crossing completed successfully");
            },
            RegionCrossingState::Failed => {
                info!("🌍 Region crossing failed");
            },
            _ => {}
        }
        
        Ok(())
    }
    
    /// Reset the state machine to idle
    pub fn reset(&mut self) {
        debug!("🌍 Resetting state machine to idle");
        
        self.previous_state = Some(self.current_state);
        self.current_state = RegionCrossingState::Idle;
        self.transition_count += 1;
        self.state_entered_at = std::time::Instant::now();
    }
    
    /// Check if the state machine is currently processing a crossing
    pub fn is_crossing(&self) -> bool {
        matches!(
            self.current_state,
            RegionCrossingState::Connecting | RegionCrossingState::MovingAgent
        )
    }
    
    /// Check if the state machine is in a stable state
    pub fn is_stable(&self) -> bool {
        matches!(
            self.current_state,
            RegionCrossingState::Idle | RegionCrossingState::Connected
        )
    }
    
    /// Check if the state machine is in an error state
    pub fn is_failed(&self) -> bool {
        self.current_state == RegionCrossingState::Failed
    }
    
    /// Get a description of the current state
    pub fn state_description(&self) -> &'static str {
        match self.current_state {
            RegionCrossingState::Idle => "Not crossing regions",
            RegionCrossingState::Connecting => "Establishing connection to new simulator",
            RegionCrossingState::MovingAgent => "Moving agent to new region",
            RegionCrossingState::Connected => "Successfully connected to region",
            RegionCrossingState::Failed => "Region crossing failed",
        }
    }
    
    /// Get statistics about the state machine
    pub fn statistics(&self) -> StateMachineStats {
        StateMachineStats {
            current_state: self.current_state,
            time_in_current_state: self.time_in_current_state(),
            transition_count: self.transition_count,
            is_crossing: self.is_crossing(),
            is_stable: self.is_stable(),
            is_failed: self.is_failed(),
        }
    }
}

impl Default for RegionCrossingStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the state machine
#[derive(Debug, Clone)]
pub struct StateMachineStats {
    pub current_state: RegionCrossingState,
    pub time_in_current_state: std::time::Duration,
    pub transition_count: u64,
    pub is_crossing: bool,
    pub is_stable: bool,
    pub is_failed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_initial_state() {
        let sm = RegionCrossingStateMachine::new();
        assert_eq!(sm.current_state(), RegionCrossingState::Idle);
        assert_eq!(sm.previous_state(), None);
        assert_eq!(sm.transition_count(), 0);
        assert!(sm.is_stable());
        assert!(!sm.is_crossing());
        assert!(!sm.is_failed());
    }
    
    #[test]
    fn test_valid_transitions() {
        let mut sm = RegionCrossingStateMachine::new();
        
        // Test normal crossing flow
        assert!(sm.transition_to(RegionCrossingState::Connecting).is_ok());
        assert_eq!(sm.current_state(), RegionCrossingState::Connecting);
        assert!(sm.is_crossing());
        
        assert!(sm.transition_to(RegionCrossingState::MovingAgent).is_ok());
        assert_eq!(sm.current_state(), RegionCrossingState::MovingAgent);
        assert!(sm.is_crossing());
        
        assert!(sm.transition_to(RegionCrossingState::Connected).is_ok());
        assert_eq!(sm.current_state(), RegionCrossingState::Connected);
        assert!(sm.is_stable());
        assert!(!sm.is_crossing());
    }
    
    #[test]
    fn test_invalid_transitions() {
        let mut sm = RegionCrossingStateMachine::new();
        
        // Cannot go directly from Idle to MovingAgent
        assert!(sm.transition_to(RegionCrossingState::MovingAgent).is_err());
        
        // Cannot go directly from Idle to Connected
        assert!(sm.transition_to(RegionCrossingState::Connected).is_err());
    }
    
    #[test]
    fn test_failure_transitions() {
        let mut sm = RegionCrossingStateMachine::new();
        
        // Can fail from any state
        assert!(sm.transition_to(RegionCrossingState::Connecting).is_ok());
        assert!(sm.transition_to(RegionCrossingState::Failed).is_ok());
        assert!(sm.is_failed());
        
        // Can recover from failure
        assert!(sm.transition_to(RegionCrossingState::Idle).is_ok());
        assert!(sm.is_stable());
    }
    
    #[test]
    fn test_state_machine_reset() {
        let mut sm = RegionCrossingStateMachine::new();
        
        sm.transition_to(RegionCrossingState::Connecting).unwrap();
        sm.transition_to(RegionCrossingState::MovingAgent).unwrap();
        
        let old_count = sm.transition_count();
        sm.reset();
        
        assert_eq!(sm.current_state(), RegionCrossingState::Idle);
        assert_eq!(sm.previous_state(), Some(RegionCrossingState::MovingAgent));
        assert_eq!(sm.transition_count(), old_count + 1);
    }
}