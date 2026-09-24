//! AI ghost-text command suggestions for zsh.

pub mod context;
pub mod daemon;
pub mod history;
#[cfg(target_os = "macos")]
pub mod menubar;
pub mod model;
pub mod paths;
pub mod stats;
pub mod status;
