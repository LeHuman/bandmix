use crate::types::Percent;

/// Device object
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: Option<String>,
    pub is_active: bool,
    pub name: String,
    pub volume: Percent,
}
