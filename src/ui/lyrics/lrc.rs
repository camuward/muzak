/// A single timed line from an LRC file.
pub struct LrcLine {
    /// Timestamp in milliseconds.
    pub time_ms: u64,
    pub text: String,
}

/// Parse an LRC time tag of the form `mm:ss.xx` or `mm:ss.xxx`.
/// Returns the time in milliseconds, or `None` if the format is invalid.
fn parse_time_tag(tag: &str) -> Option<u64> {
    let colon = tag.find(':')?;
    let minutes: u64 = tag[..colon].trim().parse().ok()?;
    let after_colon = &tag[colon + 1..];
    let dot = after_colon.find('.')?;
    let seconds: u64 = after_colon[..dot].trim().parse().ok()?;
    let frac_str = after_colon[dot + 1..].trim();

    // normalize to milliseconds
    let frac_ms = match frac_str.len() {
        1 => frac_str.parse::<u64>().ok()? * 100,
        2 => frac_str.parse::<u64>().ok()? * 10,
        _ => frac_str[..3].parse::<u64>().ok()?,
    };

    Some(minutes * 60_000 + seconds * 1_000 + frac_ms)
}

/// Attempt to parse a single line of an LRC file.
/// Returns `Some(vec)` with one entry per timestamp on the line, or `None` if the line has no
/// valid time tags.
fn parse_lrc_line(line: &str) -> Option<Vec<LrcLine>> {
    let mut timestamps: Vec<u64> = Vec::new();
    let mut rest = line;

    while rest.starts_with('[') {
        let end = rest.find(']')?;
        let tag = &rest[1..end];
        rest = &rest[end + 1..];

        if let Some(ms) = parse_time_tag(tag) {
            timestamps.push(ms);
        } else {
            // Metadata or unknown tag — stop consuming tags.
            break;
        }
    }

    if timestamps.is_empty() {
        return None;
    }

    let text = rest.trim().to_string();
    Some(
        timestamps
            .into_iter()
            .map(|time_ms| LrcLine {
                time_ms,
                text: text.clone(),
            })
            .collect(),
    )
}

/// Try to parse `content` as an LRC file.
///
/// Returns `Some(lines)` sorted by timestamp when at least one timed line is found.
/// Returns `None` when no time tags are present (plain-text).
pub fn parse_lrc(content: &str) -> Option<Vec<LrcLine>> {
    let mut lines: Vec<LrcLine> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            if let Some(last) = lines.last() {
                lines.push(LrcLine {
                    time_ms: last.time_ms,
                    text: String::new(),
                });
            }
            continue;
        }
        if let Some(parsed) = parse_lrc_line(line) {
            lines.extend(parsed);
        }
    }

    if lines.is_empty() {
        return None;
    }

    lines.sort_by_key(|l| l.time_ms);
    Some(lines)
}
