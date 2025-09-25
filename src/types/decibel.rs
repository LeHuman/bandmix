/// Represents a value in decibels (dB).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Decibel(f64);

impl Decibel {
    /// Construct directly from a dB value.
    pub fn from_db(db: f64) -> Self {
        Self(db)
    }

    /// Convert from a linear amplitude ratio.
    /// For power ratios, use `10.0 * log10(x)`.
    pub fn from_amplitude_ratio(ratio: f64) -> Self {
        Self(20.0 * ratio.log10())
    }

    /// Convert from a linear power ratio.
    pub fn from_power_ratio(ratio: f64) -> Self {
        Self(10.0 * ratio.log10())
    }

    /// Get the raw dB value.
    pub fn as_db(&self) -> f64 {
        self.0
    }

    /// Convert back to a linear amplitude ratio.
    pub fn to_amplitude_ratio(&self) -> f64 {
        10f64.powf(self.0 / 20.0)
    }

    /// Convert back to a linear power ratio.
    pub fn to_power_ratio(&self) -> f64 {
        10f64.powf(self.0 / 10.0)
    }
}
