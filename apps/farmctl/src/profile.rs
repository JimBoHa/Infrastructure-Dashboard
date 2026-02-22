use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum InstallProfile {
    Prod,
    E2e,
}

impl Default for InstallProfile {
    fn default() -> Self {
        Self::Prod
    }
}

impl InstallProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prod => "prod",
            Self::E2e => "e2e",
        }
    }
}

impl std::fmt::Display for InstallProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
