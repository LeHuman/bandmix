// https://github.com/pombadev/sunny/blob/8643b3c030c3ddc310111dda9c607108317b6140/src/lib/spider.rs#L132

use std::collections::BTreeMap;

use anyhow::Result;
use html_escape::decode_html_entities;
use scraper::{Html, Selector};
use tracing::{trace, warn};

use super::models::{Album, Track};

/// Parse data from the node: `document.querySelector('script[data-tralbum]')`
fn scrape_by_data_tralbum(dom: &Html) -> Option<Album> {
    let selector = Selector::parse("script[data-tralbum]").ok()?;
    let element = dom.select(&selector).next()?;

    let mut album = Album::default();

    for (name, val) in &element.value().attrs {
        let data = gjson::get(val.trim(), "@this");

        if &name.local == "data-embed" {
            album.id = data
                .get("tralbum_param.value")
                .to_string()
                .parse::<u32>()
                .unwrap_or(0);
            album.artist = data.get("artist").to_string();
            album.name = data.get("album_title").to_string();
        }

        if &name.local == "data-tralbum" {
            if album.name.is_empty() {
                album.name = data.get("current.title").to_string();
            }
            let mut count = 0;

            album.release_date = data.get("album_release_date").to_string();
            album.tracks = data
                .get("trackinfo")
                .array()
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let id = item
                        .get("track_id")
                        .to_string()
                        .parse::<u32>()
                        .unwrap_or(count);
                    count += 1;
                    (
                        id,
                        Track {
                            id,
                            num: (index + 1) as i32,
                            name: item.get("title").to_string(),
                            url: item.get("file.mp3-128").to_string(),
                            // lyrics: None,
                            album_id: album.id,
                        },
                    )
                })
                .collect();
        }
    }

    if album.id == 0 {
        warn!("Album {} has an id of 0", &album.name);
    }

    Some(album)
}

fn find_track_album_by_name(dom: &Html, track_name: &gjson::Value) -> Option<Album> {
    trace!("Searching for track by name {}", track_name);
    scrape_by_data_tralbum(dom)
}

/// Parse data from the node: `document.querySelector('script[type="application/ld+json"]')`
fn scrape_by_application_ld_json(dom: &Html) -> Option<Album> {
    let selector = Selector::parse("script[type='application/ld+json']").unwrap();
    let element = dom.select(&selector).next().unwrap();

    let json = element.inner_html();
    let json = json.as_str();

    if !gjson::valid(json) {
        return None;
    }

    let mut album = Album::default();

    let item = gjson::get(json, "@this");

    // println!("{}", item);

    album.name = item.get("name").to_string();
    album.url = item.get("mainEntityOfPage").to_string(); // Use @id instead?
    album.id = item
        .get("albumRelease.#(additionalProperty.#(value=a)).additionalProperty.#(name=item_id).value")
        .to_string()
        .parse::<u32>()
        .ok()?;
    album.featured_track_num = item
        .get("additionalProperty.#(name=featured_track_num).value")
        .to_string()
        .parse::<i32>()
        .ok();

    let tags = item
        .get("keywords")
        .array()
        .iter()
        .filter_map(|tag| {
            let tag = tag.str().trim();

            if tag.is_empty() {
                None
            } else {
                Some(tag)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    album.tags = Some(tags);
    album.release_date = item.get("datePublished").to_string();
    album.album_art_url = Some(item.get("image").to_string());
    album.artist = item.get("byArtist.name").to_string();
    album.artist_art_url = Some(item.get("byArtist.image").to_string());

    let tracks = item.get("track.itemListElement");

    const FILE_PATH: &str = "additionalProperty.#(name=file_mp3-128).value";
    const TRACK_ID: &str = "additionalProperty.#(name=track_id).value";

    let mut tralbum_album = None;

    // case when current url is just a track
    album.tracks = if tracks.array().is_empty() {
        let mut url = decode_html_entities(&item.get(FILE_PATH).to_string()).to_string();
        let track_id = item.get(TRACK_ID).to_string().parse().ok()?;
        let track_name = item.get("name");

        if url.is_empty() {
            tralbum_album = tralbum_album.or(find_track_album_by_name(dom, &track_name));
            if let Some(tralbum_album) = &tralbum_album {
                let track_url = tralbum_album.tracks.iter().find_map(|(_id, inner_track)| {
                    if inner_track.name == track_name.str() {
                        Some(inner_track.url.clone())
                    } else {
                        None
                    }
                });
                if let Some(track_url) = track_url {
                    url = track_url;
                } else {
                    // no url is found for the track's file
                    warn!("No downloadable url found for '{}', skipping.", &track_name);
                    return None;
                }
            } else {
                warn!("No downloadable url found for '{}', skipping.", &track_name);
                return None;
            }
        }

        BTreeMap::from([(
            track_id,
            Track {
                id: track_id,
                num: 1,
                name: track_name.to_string().replace('/', ":"),
                url,
                // lyrics: None,
                album_id: album.id,
            },
        )])
    } else {
        // case when current url is an album

        let mut retval = BTreeMap::<u32, Track>::new();

        for track in tracks.array() {
            let mut url = track.get(&("item.".to_owned() + FILE_PATH)).to_string();
            let track_id = track
                .get(&("item.".to_owned() + TRACK_ID))
                .to_string()
                .parse()
                .ok()?;

            if url.is_empty() {
                let track_name = track.get("item.name");
                tralbum_album = tralbum_album.or(find_track_album_by_name(dom, &track_name));
                if let Some(tralbum_album) = &tralbum_album {
                    let track_url = tralbum_album.tracks.iter().find_map(|(_id, inner_track)| {
                        if inner_track.name == track_name.str() {
                            Some(inner_track.url.clone())
                        } else {
                            None
                        }
                    });
                    if let Some(track_url) = track_url {
                        url = track_url;
                    } else {
                        // no url is found for the track's file
                        warn!("No downloadable url found for '{}', skipping.", &track_name);
                        continue;
                    }
                } else {
                    warn!("No downloadable url found for '{}', skipping.", &track_name);
                    continue;
                }
            }

            let name = decode_html_entities(&track.get("item.name").to_string()).replace('/', ":");
            let name = String::from(html_escape::decode_html_entities(&name));

            retval.insert(
                track_id,
                Track {
                    id: track_id,
                    num: track.get("position").i32(),
                    name,
                    url: decode_html_entities(&url).to_string(),
                    // lyrics: Some(track.get("item.recordingOf.lyrics.text").to_string()),
                    album_id: album.id,
                },
            );
        }
        retval
    };

    album.name = String::from(html_escape::decode_html_entities(&album.name));
    album.artist = String::from(html_escape::decode_html_entities(&album.artist));

    Some(album)
}

/// Facade for `scrape_by_*` methods.
/// Calls `scrape_by_application_ld_json` or `scrape_by_data_tralbum` internal methods if first fails.
fn get_album(dom: &Html) -> Option<Album> {
    scrape_by_application_ld_json(dom)
}

/// Get [`Html`] of a page.
fn fetch_html(url: &str) -> Result<Html> {
    let body = reqwest::blocking::get(url)?.bytes()?.to_vec();
    let body = String::from_utf8(body)?;

    Ok(Html::parse_document(body.as_ref()))
}

pub fn fetch_album(url: &str) -> Option<Album> {
    let html = fetch_html(url).ok()?;
    let album = get_album(&html)?;
    Some(album)
}

#[test]
fn test_get_album() {
    let mut _result = false;
    let html = fetch_html("https://loscampesinos.bandcamp.com/album/all-hell")
        .expect("Failed to get html");
    let _album = get_album(&html).expect("Failed to get Album");
    println!("{}", _album);
    _result = true;
    assert!(_result, "Failed to fetch album")
}
