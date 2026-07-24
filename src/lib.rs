pub mod api;
pub mod parser;

pub use api::{
    ItemType, ParsedUrl, SongInfo, TrackInfo, fetch_access_token, fetch_lyrics, fetch_song_info,
    fetch_track_ids, parse_url,
};

pub use parser::ttml_to_lrc;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
