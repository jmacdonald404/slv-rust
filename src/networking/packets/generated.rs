//! Generated packet definitions for Second Life protocol
//! 
//! These packets are generated from message_template.msg at build time to match 
//! the exact format expected by Second Life simulators, ensuring 100% protocol 
//! compatibility while providing Rust's type safety.

// Include the build-generated packet definitions
include!(concat!(env!("OUT_DIR"), "/messages.rs"));