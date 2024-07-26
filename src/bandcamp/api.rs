#![allow(non_camel_case_types, unreachable_patterns)]
use reqwest;
use std::collections::HashMap;
use url::Url;

pub struct Api<'a> {
    base_url: &'a str,
}

pub const DISCOVER_API: Api = Api {
    base_url: "https://bandcamp.com/api/discover/3",
};

#[derive(Clone, Default)]
pub struct Function {
    name: String,
    parameters: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Default, strum::EnumString, strum::Display)]
pub enum Genre {
    #[default]
    all,
    electronic,
    rock,
    metal,
    alternative,
    #[strum(
        to_string = "hip-hop-rap",
        serialize = "hip-hop-rap",
        serialize = "hip-hop/rap"
    )]
    hip_hop_rap,
    experimental,
    punk,
    folk,
    pop,
    ambient,
    soundtrack,
    world,
    jazz,
    acoustic,
    funk,
    #[strum(to_string = "r-b-soul", serialize = "r-b-soul", serialize = "r&b/soul")]
    r_b_soul,
    devotional,
    classical,
    reggae,
    podcasts,
    country,
    #[strum(
        to_string = "spoken-word",
        serialize = "spoken-word",
        serialize = "spoken word"
    )]
    spoken_word,
    comedy,
    blues,
    kids,
    audiobooks,
    latin,
}

#[derive(Debug, PartialEq, Default, strum::EnumString, strum::Display)]
pub enum DiscoveryType {
    #[default]
    top,
    new,
    rec,
}

#[derive(Debug, PartialEq, Default, strum::EnumString, strum::Display)]
pub enum RecommendedType {
    #[default]
    most,
    latest,
}

#[derive(Debug, PartialEq, Default, strum::EnumString, strum::Display)]
pub enum Format {
    #[default]
    all,
    digital,
    vinyl,
    cd,
    cassette,
}

impl Function {
    fn update(&mut self, key: String, val: String) {
        if self.parameters.contains_key(&key) {
            self.parameters.insert(key.to_string(), val.to_string());
        } else {
            eprintln!("Key not found in Function {}", key);
        }
    }

    pub fn update_get_web_page(&mut self, page: u32) {
        self.update(String::from("p"), page.to_string());
    }

    pub fn get_web(
        page: u32,
        genre: Option<Genre>,
        discovery_type: Option<DiscoveryType>,
        format: Option<Format>,
        recommended_type: Option<RecommendedType>,
    ) -> Self {
        let discovery_type = discovery_type.unwrap_or_default();
        let mut func = Function {
            name: String::from("get_web"),
            parameters: HashMap::from([
                (String::from("g"), genre.unwrap_or_default().to_string()),
                (String::from("s"), discovery_type.to_string()),
                (String::from("p"), page.to_string()),
                (String::from("gn"), 0.to_string()), // TODO: What is 'gn' for get_web for?
                (String::from("f"), format.unwrap_or_default().to_string()),
                (String::from("w"), 0.to_string()), // NOTE: Appended when no sub-genre is attached
                                                    // (String::from("t"), "death-metal".to_string()), // NOTE: Appended with a sub-genre attached
            ]),
        };

        if discovery_type == DiscoveryType::rec {
            func.parameters.insert(
                String::from("r"),
                recommended_type.unwrap_or_default().to_string(),
            );
        }

        return func;
    }
}

impl Api<'_> {
    pub fn build_query(&self, func: &Function) -> Result<Url, Box<dyn std::error::Error>> {
        let mut url = Url::parse(self.base_url)?;

        if let Ok(mut segments) = url.path_segments_mut() {
            segments.extend([func.name.clone()]);
        } else {
            Err("Failed to extend onto url")?;
        }

        let mut pairs = url.query_pairs_mut();
        pairs.extend_pairs(func.parameters.iter());
        // for (key, value) in &func.parameters {
        //     pairs.append_pair(key, value);
        // }
        let url = pairs.finish();

        Ok(url.to_owned())
    }

    pub fn request(url: Url) -> Result<String, Box<dyn std::error::Error>> {
        let response = reqwest::blocking::get(url)?;

        if response.status().is_success() {
            let body = response.bytes()?.to_vec();
            let json = String::from_utf8(body)?;

            if !gjson::valid(&json) {
                return Err("Failed to get valid json".into());
            }

            Ok(json)
        } else {
            Err(format!(
                "Failed to get a successful response: {}",
                response.status()
            ))?
        }
    }
}

#[test]
fn test_query_request() {
    let url = DISCOVER_API
        .build_query(&Function::get_web(0, None, None, None, None))
        .expect("Failed to build url");

    let mut _res: Result<String, Box<dyn std::error::Error>> = Ok(String::default());
    _res = Api::request(url);

    assert!(_res.is_ok());
    assert!(_res.unwrap() != serde_json::Value::default());
}
