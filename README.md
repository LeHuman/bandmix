<!-- PROJECT: bandmix -->
<!-- TITLE: bandmix -->
<!-- FONT: Mytupi -->
<!-- KEYWORDS: music, streaming  -->
<!-- LANGUAGES: Rust -->
<!-- TECHNOLOGY: RESTful API, HTTP -->
<!-- STATUS: Work In Progress -->

![Logo](<images/bandmix.png>)

[![Crates.io](https://img.shields.io/crates/v/bandmix.svg)](https://crates.io/crates/bandmix)
[![Docs.rs](https://docs.rs/bandmix/badge.svg)](https://docs.rs/bandmix)
[![CI](https://github.com/lehuman/bandmix/workflows/CI/badge.svg)](https://github.com/lehuman/bandmix/actions)

[About](#about) - [Usage](#usage) - [Related](#related) - [License](#license) - [Contribution](#contribution)

## Status

**`Work In progress`**

## About
<!-- DESCRIPTION START -->
This is a minimal autoplayer of [bandcamp's discover](https://bandcamp.com/discover) section. It plays entire albums listed on the page.
<!-- DESCRIPTION END -->

### Why

I wanted a way to automatically play entire albums on the discover page, as I like to listen to whatever albums pop up there.

I wrote the app in rust for practice.

## Usage

### Requirements

> [!IMPORTANT]
> Linux requires `libasound2-dev` on Debian / Ubuntu or `alsa-lib-devel` on Fedora\
> If not building, `libasound2` on Debian / Ubuntu or `alsa-lib` on Fedora

- [Rust](https://www.rust-lang.org/) == 2021

### Running

There currently are no UI controls for the app. The app can, however, be controlled through media keys or any native media control panel.
A cache file is created in appdata (or equivalent) to remember songs it has played.

This app has mainly been developed for Windows, with a working build on Debian Linux.

```sh
cargo run --release
```

### Installing

```sh
cargo install bandmix
```

## Related

- pombadev/[sunny](https://github.com/pombadev/sunny)
- JasonWei512/[code-radio-cli](https://github.com/JasonWei512/code-radio-cli)
- phunks/[bcradio](https://github.com/phunks/bcradio)
- michaelherger/[Bandcamp-API](https://github.com/michaelherger/Bandcamp-API)

## License

Licensed under either of

- Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](CONTRIBUTING.md).
