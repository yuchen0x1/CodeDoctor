//! Tauri command handlers, one module per feature area.
#[cfg(any(feature = "desktop", test))]
pub mod architecture;
#[cfg(feature = "desktop")]
pub mod history;
pub mod project;
#[cfg(any(feature = "desktop", test))]
pub mod report;
pub mod scan;
#[cfg(any(feature = "desktop", test))]
pub mod source;
