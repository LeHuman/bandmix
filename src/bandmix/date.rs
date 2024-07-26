use chrono::{DateTime, ParseError, Utc};

pub type EpochType = u128;

pub struct Epoch;

// TODO: Is all this casting with epoch fine?
impl Epoch {
    pub fn to_rfc3339(epoch: EpochType) -> Option<String> {
        let datetime = DateTime::from_timestamp_millis(epoch as i64);
        Some(datetime?.to_rfc3339())
    }

    pub fn from_rfc3339(date_str: &str) -> Result<EpochType, ParseError> {
        let datetime = DateTime::parse_from_rfc3339(date_str)?.with_timezone(&Utc);
        Ok(datetime.timestamp_millis() as EpochType)
    }

    pub fn get_local() -> EpochType {
        std::time::UNIX_EPOCH
            .elapsed()
            .expect("Failed to get epoch time")
            .as_millis()
    }
}
