use regex::Regex;
use std::fmt::Write;
use std::sync::LazyLock;

static TTML_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)<p\s+[^>]*?begin="([^"]+)"[^>]*>(.*?)</p>"#).unwrap());

static XML_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

pub fn ttml_to_lrc(ttml: &str) -> String {
    let mut lrc = String::new();

    for cap in TTML_LINE.captures_iter(ttml) {
        let Some(time) = format_time(&cap[1]) else {
            continue;
        };
        let text = clean_line(&cap[2]);
        let _ = writeln!(lrc, "[{time}] {text}");
    }

    lrc
}

fn clean_line(raw: &str) -> String {
    XML_TAG
        .replace_all(raw, "")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

fn parse_total_seconds(time_str: &str) -> Option<f64> {
    if let Some(s) = time_str.strip_suffix('s') {
        return s.parse().ok();
    }

    let parts: Vec<&str> = time_str.split(':').collect();
    match parts.as_slice() {
        [min, sec] => {
            let m: f64 = min.parse().ok()?;
            let s: f64 = sec.parse().ok()?;
            Some(m * 60.0 + s)
        }
        [hours, min, sec] => {
            let h: f64 = hours.parse().ok()?;
            let m: f64 = min.parse().ok()?;
            let s: f64 = sec.parse().ok()?;
            Some(h * 3600.0 + m * 60.0 + s)
        }
        _ => None,
    }
}

fn format_time(time_str: &str) -> Option<String> {
    let total_sec = parse_total_seconds(time_str)?;
    let total_ms = (total_sec * 1000.0).round() as u64;

    let min = total_ms / 60_000;
    let sec = (total_ms % 60_000) / 1000;
    let hundredths = (total_ms % 1000) / 10;

    Some(format!("{min:02}:{sec:02}.{hundredths:02}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_line_strips_xml_and_entities() {
        let input = "<span class=\"lyric\">Hello &amp; &lt;World&gt;&#39;s &quot;Test&quot;</span>";
        let expected = "Hello & <World>'s \"Test\"";

        assert_eq!(clean_line(input), expected);
    }

    #[test]
    fn test_parse_total_seconds() {
        assert_eq!(parse_total_seconds("15.75s"), Some(15.75));
        assert_eq!(parse_total_seconds("02:45"), Some(165.0));
        assert_eq!(parse_total_seconds("01:02:45"), Some(3765.0));
        assert_eq!(parse_total_seconds("invalid_time"), None);
    }

    #[test]
    fn test_format_time_converts_to_lrc_format() {
        assert_eq!(format_time("12.345s"), Some("00:12.34".to_string()));
        assert_eq!(format_time("01:30.5"), Some("01:30.50".to_string()));
        assert_eq!(format_time("01:00:05"), Some("60:05.00".to_string()));
    }

    #[test]
    fn test_ttml_to_lrc_conversion() {
        let ttml_input = r#"
            <div>
                <p begin="10.0s" end="12.0s">First line of lyrics</p>
                <p begin="00:15.5" end="18.0s">Second line with &amp; entity</p>
                <p begin="invalid" end="20.0s">This should be skipped</p>
            </div>
        "#;

        let expected_lrc =
            "[00:10.00] First line of lyrics\n[00:15.50] Second line with & entity\n";

        assert_eq!(ttml_to_lrc(ttml_input), expected_lrc);
    }
}
