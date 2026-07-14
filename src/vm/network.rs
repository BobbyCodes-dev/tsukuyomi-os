#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkMode {
    Isolated,
    HostOnly,
    Nat,
}

impl NetworkMode {
    pub fn from_id(id: &str) -> Self {
        match id {
            "hostonly" => NetworkMode::HostOnly,
            "nat" => NetworkMode::Nat,
            _ => NetworkMode::Isolated,
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            NetworkMode::Isolated => "isolated",
            NetworkMode::HostOnly => "hostonly",
            NetworkMode::Nat => "nat",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            NetworkMode::Isolated => "Isolated (default, no host/internet access)",
            NetworkMode::HostOnly => "Host-Only (bridges to host machine)",
            NetworkMode::Nat => "NAT (internet access via host)",
        }
    }

    pub fn nic_args(&self) -> Vec<String> {
        match self {
            NetworkMode::Isolated => vec![
                "--nic1".to_string(),
                "intnet".to_string(),
                "--intnet1".to_string(),
                "tsukuyomi-sandbox".to_string(),
            ],
            NetworkMode::HostOnly => vec![
                "--nic1".to_string(),
                "hostonly".to_string(),
                "--hostonlyadapter1".to_string(),
                "vboxnet0".to_string(),
            ],
            NetworkMode::Nat => vec!["--nic1".to_string(), "nat".to_string()],
        }
    }
}

impl Default for NetworkMode {
    fn default() -> Self {
        NetworkMode::Isolated
    }
}

pub const ALL_MODES: &[NetworkMode] = &[NetworkMode::Isolated, NetworkMode::HostOnly, NetworkMode::Nat];
