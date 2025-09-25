use std::collections::HashSet;
use std::time::Duration;

use crate::model::device::Device;
use crate::model::music::Genre;
use crate::model::music::Track;
use crate::types::Percent;

pub struct LengthFilter {
    smallest: Option<Duration>,
    largest: Option<Duration>,
}

pub struct Filter {
    genres: Vec<Genre>,
    length: Option<LengthFilter>,
}

#[derive(PartialEq, strum::EnumString, strum::Display)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Buffering,
    Error(String),
}

pub struct SongContext {
    pub track: Track,
    pub position: Duration,
}

pub enum RepeatMethod {
    Forever,
    Once,
    Twice,
    Album,
}

pub enum PlayerActions {
    Playing,
    Pausing,
    Seeking,
    SkippingNext,
    SkippingPrev,
    TogglingRepeat,
    TogglingMute,
    ChangingDevice,
}

pub struct PlaybackContext {
    pub device: Device,
    pub volume: Percent,
    pub repeat: RepeatMethod,
    pub mute: bool,
    pub available_actions: HashSet<PlayerActions>,
}

pub struct Player {
    pub state: PlaybackState,
    pub song: Option<SongContext>,
    pub queue: Vec<Track>,
    pub context: PlaybackContext,
}

pub mod preferences {
    use crate::types::Decibel;

    pub struct Persistence {
        pub song_position: bool,
        pub listened_songs: bool,
    }

    pub struct Player {
        pub enable_volume_clip: bool,
        pub volume_clip: Decibel,
    }
}

pub struct Preferences {
    persistent: preferences::Persistence,
    player: preferences::Player,
}
