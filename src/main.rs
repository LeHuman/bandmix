use std::sync::{atomic::AtomicBool, Arc, Mutex};

use bandmix::{
    controls::get_media_controls,
    discovery::{self, Entry},
    stream::Player,
};
use souvlaki::{MediaControlEvent, MediaMetadata};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

mod bandcamp;
mod bandmix;

async fn new_track(track: &Entry, player: &Player) {
    println!("NOW PLAYING: {}", track);
    player.start(&track.url).await;
}

#[cfg(target_os = "windows")]
use windows::{
    core::PCWSTR,
    Win32::{
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{LoadImageW, IMAGE_ICON, LR_DEFAULTSIZE},
    },
};

#[cfg(target_os = "windows")]
fn load_icon() -> windows::core::Result<()> {
    let _icon = unsafe {
        LoadImageW(
            GetModuleHandleW(None)?,
            PCWSTR(1 as _), // Value must match the `nameID` in the .rc script
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE,
        )
    }?;
    Ok(())
}

#[tokio::main]
async fn main() {
    #[cfg(target_os = "windows")]
    let _ = load_icon();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Setting default subscriber failed");

    let mut controls = get_media_controls();
    let player = Player::new().expect("Failed to get Player");
    let update_trigger: Arc<AtomicBool> = Arc::new(AtomicBool::new(true));
    let update_event: Arc<Mutex<MediaControlEvent>> = Arc::new(Mutex::new(MediaControlEvent::Play));
    let update_trigger_clone: Arc<AtomicBool> = Arc::clone(&update_trigger);
    let update_event_clone: Arc<Mutex<MediaControlEvent>> = Arc::clone(&update_event);
    let mut initial = false;
    player.pause();

    discovery::start(None, None, None, None);
    controls
        .attach(move |event: MediaControlEvent| {
            println!("Event received: {:?}", event);
            let mut enum_guard = update_event_clone.lock().unwrap();
            *enum_guard = event; // TODO: should I queue up commands?
            update_trigger_clone.store(true, std::sync::atomic::Ordering::Relaxed);
        })
        .unwrap();

    let mut last_track = Entry::default();

    loop {
        // TODO: separate user and internal controls
        if initial && player.empty() {
            if discovery::mark_current_track().is_none() {
                eprintln!("Failed to mark current track");
            }
            let mut event = update_event.lock().unwrap();
            *event = MediaControlEvent::Next;
            update_trigger.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if !update_trigger.load(std::sync::atomic::Ordering::Relaxed) {
            continue;
        }
        let event = update_event.try_lock();
        if let Ok(event) = event {
            let mut track = Entry::default();
            initial = true;
            match event.to_owned() {
                MediaControlEvent::Play => {
                    println!("[PLAY]");
                    track = discovery::current().unwrap_or_default();

                    if !player.is_paused() {
                        println!("[PAUSING ON ASSUMPTION]");
                        player.pause();
                    } else {
                        if player.empty() {
                            new_track(&track, &player).await;
                        }
                        player.play();
                    }
                }
                MediaControlEvent::Pause => {
                    println!("[PAUSE]");
                    player.pause();
                }
                MediaControlEvent::Toggle => {
                    println!("[TOGGLE]");
                    if player.is_paused() {
                        player.play();
                    } else {
                        player.pause();
                    };
                }
                MediaControlEvent::Next => {
                    println!("[NEXT]");
                    if discovery::mark_current_track().is_none() {
                        eprintln!("Failed to mark last track");
                    }
                    track = discovery::next().unwrap_or_default();
                    new_track(&track, &player).await;
                }
                MediaControlEvent::Previous => {
                    println!("[PREVIOUS]");
                    track = discovery::previous().unwrap_or_default();
                    new_track(&track, &player).await;
                }
                MediaControlEvent::Stop => {
                    println!("[STOP]");
                    player.stop();
                    discovery::stop();
                    break;
                }
                MediaControlEvent::Quit => {
                    println!("[QUIT]");
                    player.stop();
                    discovery::stop();
                    break;
                }
                _ => {
                    println!("[OTHER]");
                } // TODO: other media controls
            };

            // TODO: use album id instead
            if (last_track != track) || (last_track.album_name != track.album_name) {
                // TODO: duration
                // TODO: handle error
                let _ = controls.set_metadata(MediaMetadata {
                    title: Some(&track.name),
                    artist: Some(&track.artist),
                    album: Some(&track.album_name),
                    cover_url: track.album_art_url.as_deref(),
                    ..Default::default()
                });
                last_track = track;
            }
            update_trigger.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
