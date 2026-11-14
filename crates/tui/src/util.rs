pub const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
pub const SPINNER_ASCII: &[&str] = &["|", "/", "-", "\\"];

pub fn spinner(tick: usize) -> &'static str {
    let frames = if std::env::var("TERMAGENT_ASCII").is_ok() {
        SPINNER_ASCII
    } else {
        SPINNER
    };
    frames[tick % frames.len()]
}

/// Wrap a string to `width` columns, keeping explicit newlines.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut used = 0usize;
        for word in line.split(' ') {
            let w = word_width(word);
            if used == 0 {
                current.push_str(word);
                used = w;
            } else if used + 1 + w <= width {
                current.push(' ');
                current.push_str(word);
                used += 1 + w;
            } else {
                out.push(current);
                current = word.to_string();
                used = w;
            }
            while used > width && current.len() > 1 {
                // Hard-break unbroken tokens (paths, urls, code).
                let (head, tail) = split_at_width(&current, width);
                out.push(head);
                current = tail;
                used = word_width(&current);
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn split_at_width(s: &str, width: usize) -> (String, String) {
    let mut used = 0;
    let mut idx = 0;
    for (i, ch) in s.char_indices() {
        let w = ch.width().unwrap_or(1);
        if used + w > width {
            idx = i;
            break;
        }
        used += w;
        idx = i + ch.len_utf8();
    }
    (s[..idx].to_string(), s[idx..].to_string())
}

use unicode_width::UnicodeWidthChar;

pub fn word_width(s: &str) -> usize {
    s.chars().map(|c| c.width().unwrap_or(1)).sum()
}

pub fn ellipsize(s: &str, width: usize) -> String {
    if word_width(s) <= width {
        return s.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let w = ch.width().unwrap_or(1);
        if used + w + 1 > width {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

/// Slice `s` by column width, skipping `skip` columns and taking `max` columns.
pub fn slice_width(s: &str, skip: usize, max: usize) -> String {
    let mut out = String::new();
    let mut col = 0usize;
    for ch in s.chars() {
        let w = ch.width().unwrap_or(1);
        if col < skip {
            col += w;
            continue;
        }
        if col + w > skip + max {
            break;
        }
        out.push(ch);
        col += w;
    }
    out
}

pub fn ascii() -> bool {
    std::env::var("TERMAGENT_ASCII").is_ok()
}

pub fn sym(unicode: &'static str, fallback: &'static str) -> &'static str {
    if ascii() {
        fallback
    } else {
        unicode
    }
}

pub fn window(len: usize, sel: usize, height: usize) -> usize {
    if len <= height || height == 0 {
        return 0;
    }
    sel.saturating_sub(height / 2).min(len - height)
}
