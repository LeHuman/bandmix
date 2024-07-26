use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, PlatformConfig};

pub fn get_media_controls() -> MediaControls {
    #[cfg(not(target_os = "windows"))]
    let hwnd = None;

    #[cfg(target_os = "windows")]
    let hwnd = {
        let console_window = unsafe {
            use windows::{
                Media::SystemMediaTransportControls,
                Win32::System::{
                    Console::{AllocConsole, FreeConsole, GetConsoleWindow},
                    WinRT::ISystemMediaTransportControlsInterop,
                },
            };
            let mut window = GetConsoleWindow();
            let interop = windows::core::factory::<
                SystemMediaTransportControls,
                ISystemMediaTransportControlsInterop,
            >();
            if let Ok(interop) = interop {
                let controls: Result<SystemMediaTransportControls, windows::core::Error> =
                    interop.GetForWindow(window);
                if controls.is_err() {
                    FreeConsole().expect("Failed to free from console");
                    AllocConsole().expect("Failed to allocate console");
                    window = GetConsoleWindow();
                    // ShowWindow(window, SW_SHOW);
                    // ShowWindow(window, SW_HIDE);
                }
            }
            window
        };
        Some(console_window.0)
    };

    let config = PlatformConfig {
        dbus_name: "bandmix",
        display_name: "BandMix",
        hwnd,
    };

    let controls = MediaControls::new(config).unwrap();

    // // The closure must be Send and have a static lifetime.
    // controls
    //     .attach(|event: MediaControlEvent| println!("Event received: {:?}", event))
    //     .unwrap();

    // // Update the media metadata.
    // controls
    //     .set_metadata(MediaMetadata {
    //         title: Some("Souvlaki Space Station"),
    //         artist: Some("Slowdive"),
    //         album: Some("Souvlaki"),
    //         ..Default::default()
    //     })
    //     .unwrap();

    // Your actual logic goes here.
    // loop {
    // std::thread::sleep(std::time::Duration::from_secs(1));
    // }

    // The controls automatically detach on drop.
    return controls;
}
