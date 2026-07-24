use clap::Parser;
use reqwest::Client;
use std::path::{Path, PathBuf};

use amly::Result;
use amly::api::{self, ParsedUrl, TrackInfo};
use amly::parser;

#[derive(Parser, Debug)]
#[command(name = "amly", version, about)]
struct Args {
    #[arg(short, long, default_value = "downloads", value_name = "OUTPUT_DIR")]
    output_dir: PathBuf,

    #[arg(short, long, help = "Enable verbose output")]
    verbose: bool,

    #[arg(
        value_name = "URLs",
        required = true,
        help = "One or more Apple Music URLs (song, album, or playlist)"
    )]
    urls: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    let args = Args::parse();

    let client = Client::builder()
        .default_headers(api::base_headers()?)
        .build()?;

    let token = api::fetch_access_token(&client).await?;
    if args.verbose {
        println!("Access token successfully fetched.");
    }

    for url in &args.urls {
        if let Err(e) = process_url(&client, &token, url, &args.output_dir, args.verbose).await {
            eprintln!("error: failed to process {url}: {e}");
        }
    }

    Ok(())
}

async fn process_url(
    client: &Client,
    token: &str,
    url: &str,
    output_dir: &Path,
    verbose: bool,
) -> Result<()> {
    let parsed: ParsedUrl = api::parse_url(url).ok_or("invalid or unsupported URL")?;
    let track_ids = api::fetch_track_ids(client, token, &parsed).await?;

    for track_id in track_ids {
        let info = TrackInfo {
            storefront: parsed.storefront.clone(),
            track_id,
        };

        if let Err(e) = save_lyrics(client, token, &info, output_dir, verbose).await {
            eprintln!("error: failed to process track ID {}: {e}", info.track_id);
        }
    }

    Ok(())
}

async fn save_lyrics(
    client: &Client,
    token: &str,
    info: &TrackInfo,
    output_dir: &Path,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("Fetching lyrics for track ID {}...", info.track_id);
    }

    let ttml = api::fetch_lyrics(client, token, info).await?;
    let lrc = parser::ttml_to_lrc(&ttml);

    let (artist, album, title, track_number) = match api::fetch_song_info(client, token, info).await
    {
        Ok(song) => (
            api::sanitize_filename(&song.artist),
            api::sanitize_filename(&song.album),
            api::sanitize_filename(&song.title),
            song.track_number,
        ),
        Err(e) => {
            if verbose {
                eprintln!(
                    "warning: failed to fetch metadata for track {}: {e}",
                    info.track_id
                );
            }
            (
                "Unknown Artist".to_string(),
                "Unknown Album".to_string(),
                info.track_id.clone(),
                0,
            )
        }
    };

    let dir = output_dir.join(&artist).join(&album);
    std::fs::create_dir_all(&dir)?;

    let filename = if track_number > 0 {
        format!("{:02}. {}.lrc", track_number, title)
    } else {
        format!("{}.lrc", title)
    };

    let path = dir.join(filename);
    std::fs::write(&path, &lrc)?;
    println!("Saved: {}", path.display());

    Ok(())
}
