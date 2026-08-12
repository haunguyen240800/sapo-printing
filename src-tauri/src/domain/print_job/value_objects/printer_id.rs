//! `PrinterId` — cross-aggregate reference to a printer resource.
//!
//! Held by `PrintJob` aggregate as the **identity** of the destination printer.
//! The printer itself is an OS-owned resource (not a domain aggregate), so the
//! `PrintJob` only carries its identifier — the actual printer state lives in
//! the OS spooler and is queried at print time via the `PrinterPort` port.
//!
//! The underlying value is the OS device identifier:
//! - Windows: print queue / device name returned by `Get-Printer`
//! - macOS / Linux: CUPS queue name
//!
//! On all current platforms the device id equals the display name, but the
//! type-level distinction is preserved for forward compatibility.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrinterId(String);

impl PrinterId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PrinterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for PrinterId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for PrinterId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}
