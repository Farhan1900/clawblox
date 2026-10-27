//! Clawblox game engine library
//!
//! This module exposes the core game engine functionality for testing
//! and potential library use.

pub mod agent;
pub mod agent_recovery;
pub mod agent_runner;
pub mod agent_sandbox;
pub mod capture;
pub mod config;
pub mod db;
pub mod game;
pub mod manifest;
pub mod paths;
pub mod r2;
pub mod recording;
pub mod replay;
pub mod replay_agent_log;
pub mod replay_metadata;
pub mod snapshot;
pub mod workspace_recording;
pub mod world_capability_proxy;
pub mod world_runner;
pub mod world_source;

/// Address local servers listen on: `CLAWBLOX_HOST` (set by the global `--host` flag), default loopback only.
pub fn bind_addr(port: u16) -> std::net::SocketAddr {
    let host = std::env::var("CLAWBLOX_HOST").unwrap_or_default();
    let ip = if host.trim().is_empty() {
        std::net::IpAddr::from([127, 0, 0, 1])
    } else {
        host.trim().parse().unwrap_or_else(|_| {
            eprintln!("Error: invalid --host / CLAWBLOX_HOST value '{}'", host);
            std::process::exit(2);
        })
    };
    std::net::SocketAddr::new(ip, port)
}
