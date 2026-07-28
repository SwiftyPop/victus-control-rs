use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub const DESTINATION: &str = "org.hp.VictusControl";
pub const PATH: &str = "/org/hp/VictusControl";
pub const INTERFACE: &str = "org.hp.VictusControl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FanMode {
    Auto,
    BetterAuto,
    Manual,
    Max,
}

impl FanMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            FanMode::Auto => "AUTO",
            FanMode::BetterAuto => "BETTER_AUTO",
            FanMode::Manual => "MANUAL",
            FanMode::Max => "MAX",
        }
    }
}

impl fmt::Display for FanMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for FanMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_uppercase().replace(['-', ' '], "_");
        match normalized.as_str() {
            "AUTO" => Ok(FanMode::Auto),
            "BETTER_AUTO" | "BETTERAUTO" => Ok(FanMode::BetterAuto),
            "MANUAL" => Ok(FanMode::Manual),
            "MAX" => Ok(FanMode::Max),
            _ => Err(format!("Unknown fan mode: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fan_mode_parsing() {
        assert_eq!(FanMode::from_str("AUTO").unwrap(), FanMode::Auto);
        assert_eq!(
            FanMode::from_str("better_auto").unwrap(),
            FanMode::BetterAuto
        );
        assert_eq!(
            FanMode::from_str("BetterAuto").unwrap(),
            FanMode::BetterAuto
        );
        assert_eq!(
            FanMode::from_str("  better-auto  ").unwrap(),
            FanMode::BetterAuto
        );
        assert_eq!(
            FanMode::from_str("better auto").unwrap(),
            FanMode::BetterAuto
        );
        assert_eq!(FanMode::from_str("manual").unwrap(), FanMode::Manual);
        assert_eq!(FanMode::from_str("MAX").unwrap(), FanMode::Max);
        assert!(FanMode::from_str("invalid_mode").is_err());
        assert!(FanMode::from_str("").is_err());
    }

    #[test]
    fn test_fan_mode_formatting() {
        assert_eq!(FanMode::Auto.as_str(), "AUTO");
        assert_eq!(FanMode::BetterAuto.to_string(), "BETTER_AUTO");
        assert_eq!(FanMode::Manual.to_string(), "MANUAL");
        assert_eq!(FanMode::Max.to_string(), "MAX");
    }

    #[test]
    fn test_fan_mode_clone_and_equality() {
        let mode = FanMode::BetterAuto;
        let mode_copy = mode;
        assert_eq!(mode, mode_copy);
        assert_ne!(FanMode::Auto, FanMode::Max);
    }

    #[test]
    fn test_constants() {
        assert_eq!(DESTINATION, "org.hp.VictusControl");
        assert_eq!(PATH, "/org/hp/VictusControl");
        assert_eq!(INTERFACE, "org.hp.VictusControl");
    }
}
