/// Represents a percentage, where `1.0` = 100%.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Percent(f64);

impl Percent {
    /// Construct from a fraction (e.g., 1.0 = 100%).
    pub fn from_fraction(frac: f64) -> Self {
        Self(frac)
    }

    /// Construct from a percentage value (e.g., 100.0 = 100%).
    pub fn from_percent(percent: f64) -> Self {
        Self(percent / 100.0)
    }

    /// Get the value as a fraction.
    pub fn as_fraction(&self) -> f64 {
        self.0
    }

    /// Get the value as a percentage (0.0 = 0%, 100.0 = 100%).
    pub fn as_percent(&self) -> f64 {
        self.0 * 100.0
    }

    /// Clamp into the 0%–100% range.
    pub fn clamped(&self) -> Self {
        Self(self.0.clamp(0.0, 1.0))
    }
}
