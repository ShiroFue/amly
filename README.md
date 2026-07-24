# Amly

A CLI tool for downloading Apple Music lyrics as LRC files.

## Features

- Batch download multiple URLs (song, album, playlist)
- Includes timestamped lyrics when available

## Prerequisite

- Active Apple Music subscription.
- Apple Music "media-user-token" to access Apple Music APIs. See the note below on how to obtain it.

## Getting started

Download the latest pre-built binary from the [Releases](../../releases) page.

You must provide a valid Apple Music `MEDIA_USER_TOKEN`. One way to obtain it is:

- Sign in to https://music.apple.com in your browser.
- Open Developer Tools (F12) and go to the Application > Storage > Cookies.
- Find the cookie for `https://music.apple.com` named `media-user-token` and copy its value.
- Copy `env.example` to `.env` and place the token there.

## CLI Usage

```text
Usage: amly [OPTIONS] <URLs>...

Arguments:
  <URLs>...  One or more Apple Music URLs (song, album, or playlist)

Options:
  -o, --output-dir <OUTPUT_DIR>  [default: downloads]
  -v, --verbose                  Enable verbose output
  -h, --help                     Print help
  -V, --version                  Print version
```

**Examples:**

```bash
# Download lyrics for a single song URL
amly "https://music.apple.com/us/song/some-song/1234567890"

# Download multiple URLs
amly "https://music.apple.com/us/album/some-album/1234567890" "https://music.apple.com/us/playlist/some-playlist/1234567890"

# Download lyrics and write to a custom directory
amly --output-dir my-lyrics "https://music.apple.com/us/song/some-song/1234567890"

# Enable verbose logging
amly -v "https://music.apple.com/us/album/some-album/1234567890"
```

## LICENSE

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
