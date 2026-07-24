use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue, ORIGIN, REFERER, USER_AGENT};
use serde_json::Value;
use std::env;

use crate::Result;

const APPLE_MUSIC_BASE_URL: &str = "https://music.apple.com";
const APPLE_MUSIC_API_BASE_URL: &str = "https://amp-api.music.apple.com/v1/catalog";
const USER_AGENT_STR: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemType {
    Song(String),
    Album(String),
    Playlist(String),
}

#[derive(Debug, Clone)]
pub struct ParsedUrl {
    pub storefront: String,
    pub item_type: ItemType,
}

#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub storefront: String,
    pub track_id: String,
}

#[derive(Debug, Clone)]
pub struct SongInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: u32,
}

pub fn get_media_user_token() -> Result<String> {
    let token = env::var("MEDIA_USER_TOKEN").map_err(|_| "environment variable not found")?;

    if token.trim().is_empty() {
        return Err("MEDIA_USER_TOKEN cannot be empty".into());
    }

    Ok(token)
}

pub fn base_headers() -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_STR));
    headers.insert(ORIGIN, HeaderValue::from_static(APPLE_MUSIC_BASE_URL));

    let referer_url = format!("{}/", APPLE_MUSIC_BASE_URL);
    headers.insert(REFERER, HeaderValue::from_str(&referer_url)?);

    let token = get_media_user_token()?;
    let token_value = HeaderValue::from_str(&token)?;

    headers.insert("media-user-token", token_value);

    Ok(headers)
}

fn is_valid_storefront(s: &str) -> bool {
    !s.is_empty() && s.len() <= 10 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn is_valid_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-'
}

pub fn parse_url(url: &str) -> Option<ParsedUrl> {
    let https_prefix = format!("{}/", APPLE_MUSIC_BASE_URL);
    let http_prefix = https_prefix.replace("https://", "http://");

    let rest = url
        .strip_prefix(&https_prefix)
        .or_else(|| url.strip_prefix(&http_prefix))?;

    let (storefront, rest) = rest.split_once('/')?;
    if !is_valid_storefront(storefront) {
        return None;
    }

    let (entity_type, rest) = rest.split_once('/')?;
    if !matches!(entity_type, "album" | "playlist" | "song") {
        return None;
    }

    let (path, query) = match rest.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (rest, None),
    };

    let entity_id = path.rsplit('/').next()?;
    if entity_id.is_empty() || !entity_id.chars().all(is_valid_id_char) {
        return None;
    }

    let track_query_id = query.and_then(|q| {
        q.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == "i" && !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()))
                .then(|| value.to_string())
        })
    });

    let item_type = match track_query_id {
        Some(track_id) => ItemType::Song(track_id),
        None => match entity_type {
            "song" => ItemType::Song(entity_id.to_string()),
            "album" => ItemType::Album(entity_id.to_string()),
            "playlist" => ItemType::Playlist(entity_id.to_string()),
            _ => unreachable!("entity_type already validated above"),
        },
    };

    Some(ParsedUrl {
        storefront: storefront.to_string(),
        item_type,
    })
}

fn find_index_js_path(html: &str) -> Option<String> {
    html.split("/assets/").skip(1).find_map(|segment| {
        let js_end = segment.find(".js")?;
        let candidate = &segment[..js_end];
        if candidate.contains('~') {
            Some(format!("/assets/{candidate}.js"))
        } else {
            None
        }
    })
}

fn find_access_token(js: &str) -> Option<String> {
    js.split('"')
        .find(|s| s.starts_with("eyJ") && s.split('.').count() == 3)
        .map(ToString::to_string)
}

pub async fn fetch_access_token(client: &Client) -> Result<String> {
    let html = client
        .get(APPLE_MUSIC_BASE_URL)
        .send()
        .await?
        .text()
        .await?;

    let js_path = find_index_js_path(&html).ok_or("js index file not found")?;
    let js_url = format!("{}{}", APPLE_MUSIC_BASE_URL, js_path);
    let js_content = client.get(&js_url).send().await?.text().await?;

    find_access_token(&js_content).ok_or_else(|| "token not found".into())
}

pub async fn fetch_track_ids(
    client: &Client,
    token: &str,
    parsed: &ParsedUrl,
) -> Result<Vec<String>> {
    let (endpoint, entity_id) = match &parsed.item_type {
        ItemType::Song(id) => return Ok(vec![id.clone()]),
        ItemType::Album(id) => ("albums", id),
        ItemType::Playlist(id) => ("playlists", id),
    };

    let url = format!(
        "{}/{}/{}/{}",
        APPLE_MUSIC_API_BASE_URL, parsed.storefront, endpoint, entity_id
    );

    let resp = client.get(&url).bearer_auth(token).send().await?;

    if !resp.status().is_success() {
        return Err(format!(
            "failed to fetch track IDs: HTTP {} for URL {}",
            resp.status(),
            url
        )
        .into());
    }

    let resp_json: Value = resp.json().await?;

    let only_songs = matches!(parsed.item_type, ItemType::Playlist(_));
    let track_ids = resp_json["data"][0]["relationships"]["tracks"]["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|track| !only_songs || track["type"].as_str() == Some("songs"))
        .filter_map(|track| track["id"].as_str().map(str::to_string))
        .collect();

    Ok(track_ids)
}

pub async fn fetch_song_info(client: &Client, token: &str, info: &TrackInfo) -> Result<SongInfo> {
    let url = format!(
        "{}/{}/songs/{}",
        APPLE_MUSIC_API_BASE_URL, info.storefront, info.track_id
    );

    let resp = client.get(&url).bearer_auth(token).send().await?;

    if !resp.status().is_success() {
        return Err(format!(
            "failed to fetch song info: HTTP {} for track {}",
            resp.status(),
            info.track_id
        )
        .into());
    }

    let resp_json: Value = resp.json().await?;
    let attrs = &resp_json["data"][0]["attributes"];

    let track_number = attrs["trackNumber"].as_u64().unwrap_or(0) as u32;

    Ok(SongInfo {
        title: attrs["name"]
            .as_str()
            .unwrap_or("Unknown Title")
            .to_string(),
        artist: attrs["artistName"]
            .as_str()
            .unwrap_or("Unknown Artist")
            .to_string(),
        album: attrs["albumName"]
            .as_str()
            .unwrap_or("Unknown Album")
            .to_string(),
        track_number,
    })
}

pub fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    sanitized.trim().to_string()
}

pub async fn fetch_lyrics(client: &Client, token: &str, info: &TrackInfo) -> Result<String> {
    let url = format!(
        "{}/{}/songs/{}/lyrics",
        APPLE_MUSIC_API_BASE_URL, info.storefront, info.track_id
    );

    let resp = client.get(&url).bearer_auth(token).send().await?;

    match resp.status() {
        reqwest::StatusCode::OK => {
            let resp_json: Value = resp.json().await?;
            resp_json["data"][0]["attributes"]["ttml"]
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "lyrics not found in response".into())
        }
        reqwest::StatusCode::NOT_FOUND => {
            Err("lyrics not found for this track on Apple Music".into())
        }
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            Err("invalid or expired MEDIA_USER_TOKEN / access token".into())
        }
        status => Err(format!("failed to fetch lyrics: HTTP {status}").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_storefront() {
        assert!(is_valid_storefront("us"));
        assert!(is_valid_storefront("id-store"));
        assert!(!is_valid_storefront(""));
        assert!(!is_valid_storefront("verylongstorefront"));
        assert!(!is_valid_storefront("us/uk"));
    }

    #[test]
    fn test_is_valid_id_char() {
        assert!(is_valid_id_char('a'));
        assert!(is_valid_id_char('9'));
        assert!(is_valid_id_char('-'));
        assert!(is_valid_id_char('.'));
        assert!(!is_valid_id_char('/'));
        assert!(!is_valid_id_char('?'));
    }

    #[test]
    fn test_parse_url_song() {
        let url = "https://music.apple.com/us/album/song-title/12345?i=67890";
        let parsed = parse_url(url).expect("Failed to parse valid song URL");

        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.item_type, ItemType::Song("67890".to_string()));
    }

    #[test]
    fn test_parse_url_album() {
        let url = "https://music.apple.com/gb/album/album-title/12345";
        let parsed = parse_url(url).expect("Failed to parse valid album URL");

        assert_eq!(parsed.storefront, "gb");
        assert_eq!(parsed.item_type, ItemType::Album("12345".to_string()));
    }

    #[test]
    fn test_parse_url_invalid() {
        let url = "https://invalid.com/us/album/12345";
        assert!(parse_url(url).is_none());
    }

    #[test]
    fn test_find_index_js_path() {
        let html = r#"<html><script src="/assets/index~abc12.js"></script></html>"#;
        assert_eq!(
            find_index_js_path(html),
            Some("/assets/index~abc12.js".to_string())
        );

        let html_invalid = r#"<html><script src="/assets/index.js"></script></html>"#;
        assert_eq!(find_index_js_path(html_invalid), None);
    }

    #[test]
    fn test_find_access_token() {
        let js = r#"const token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.payload.signature";"#;
        let expected = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.payload.signature".to_string();

        assert_eq!(find_access_token(js), Some(expected));
    }

    #[test]
    fn test_sanitize_filename() {
        let raw_name = "Artist: Song Title <Live> | 2026/2027?*\"\\";
        let expected = "Artist_ Song Title _Live_ _ 2026_2027____";

        assert_eq!(sanitize_filename(raw_name), expected);
    }
}
