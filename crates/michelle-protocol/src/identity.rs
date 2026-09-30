//! Shared application identity used by the daemon and desktop client.

#[cfg(debug_assertions)]
pub const APP_NAME: &str = "Michelle Debug";
#[cfg(not(debug_assertions))]
pub const APP_NAME: &str = "Michelle";

#[cfg(debug_assertions)]
pub const APP_ID: &str = "co.myrt.michelle.dev";
#[cfg(not(debug_assertions))]
pub const APP_ID: &str = "co.myrt.michelle";

#[cfg(debug_assertions)]
pub const DATA_DIRECTORY_NAME: &str = "Michelle Debug";
#[cfg(not(debug_assertions))]
pub const DATA_DIRECTORY_NAME: &str = "Michelle";
