// https://github.com/pombadev/sunny/blob/8643b3c030c3ddc310111dda9c607108317b6140/src/lib/models.rs

#[derive(Debug, Default, Clone)]
pub struct Track {
    pub id: u32,
    pub num: i32,
    pub name: String,
    pub url: String,
    // pub lyrics: Option<String>,
    pub album_id: u32,
}

impl Track {
    pub fn valid(&self) -> bool {
        !self.name.is_empty() && !self.url.is_empty()
    }
}

impl std::fmt::Display for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{} : {}", self.name, self.id))
    }
}

#[derive(Default, Debug, Clone)]
pub struct Album {
    pub id: u32,
    pub artist: String,
    pub name: String,
    pub url: String,
    pub release_date: String,
    pub featured_track_num: Option<i32>,
    pub tracks: Vec<Track>,
    pub tags: Option<String>,
    pub album_art_url: Option<String>,
    pub artist_art_url: Option<String>,
}

impl std::fmt::Display for Album {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut result = self.name.to_string() + " : " + &self.id.to_string();
        if !self.url.is_empty() {
            result += &format!("\n{}", self.url);
        }
        result += &format!("\n {}", self.artist);
        for track in &self.tracks {
            let mut extra = " ";
            if let Some(featured) = self.featured_track_num {
                if featured == track.num {
                    extra = "*"
                }
            }
            result += &format!("\n{} {}", extra, track);
        }
        f.write_str(&result)
    }
}
