use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::tui::theme;

pub fn parse(input: &str) -> Vec<Span<'_>> {
    let mut spans: Vec<Span<'_>> = Vec::new();
    let bytes = input.as_bytes();
    let len = input.len();
    let mut i = 0;
    // start of the current plain-text accumulation window
    let mut plain_start = 0;

    // Flush any accumulated plain text up to (but not including) `end`.
    macro_rules! flush_plain {
        ($end:expr) => {
            if plain_start < $end {
                spans.push(Span::raw(&input[plain_start..$end]));
            }
        };
    }

    while i < len {
        // ── blockquote  > text ───────────────────────────────────────────────
        // Only match at the start of the string or after a newline.
        let at_line_start = i == 0 || bytes[i - 1] == b'\n';
        if at_line_start && bytes[i] == b'>' && i + 1 < len && bytes[i + 1] == b' ' {
            flush_plain!(i);
            // The blockquote prefix character.
            spans.push(Span::styled(
                "│ ",
                Style::default().fg(theme::TEXT_MUTED),
            ));
            // The rest of the line (after "> ") rendered in muted style.
            let line_end = find_byte(bytes, b'\n', i + 2).unwrap_or(len);
            let line_content = &input[i + 2..line_end];
            spans.push(Span::styled(
                line_content,
                Style::default().fg(theme::TEXT_MUTED),
            ));
            i = line_end;
            plain_start = i;
            continue;
        }

        // ── masked link  [text](url) ─────────────────────────────────────────
        if bytes[i] == b'[' {
            // Find the closing bracket.
            if let Some(close_bracket) = find_byte(bytes, b']', i + 1) {
                // Check for immediately following '('.
                let after_bracket = close_bracket + 1;
                if after_bracket < len && bytes[after_bracket] == b'(' {
                    if let Some(close_paren) = find_byte(bytes, b')', after_bracket + 1) {
                        flush_plain!(i);
                        let link_text = &input[i + 1..close_bracket];
                        spans.push(Span::styled(
                            link_text,
                            Style::default()
                                .fg(theme::LINK)
                                .add_modifier(Modifier::UNDERLINED),
                        ));
                        i = close_paren + 1;
                        plain_start = i;
                        continue;
                    }
                }
            }
        }


        // ── code block  ```...``` ────────────────────────────────────────────
        if bytes[i] == b'`' && i + 2 < len && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
            // find closing ```
            if let Some(close) = find_str(input, "```", i + 3) {
                flush_plain!(i);
                let inner = &input[i + 3..close];
                // strip optional language tag on first line
                let display = match inner.find('\n') {
                    Some(nl) => {
                        let lang = inner[..nl].trim();
                        let code = &inner[nl + 1..];
                        if !lang.is_empty() {
                            // emit language label then code body
                            spans.push(Span::styled(
                                format!("[{}] ", lang),
                                Style::default().fg(theme::TEXT_MUTED),
                            ));
                            code
                        } else {
                            code
                        }
                    }
                    None => inner,
                };
                spans.push(Span::styled(
                    display.to_owned(),
                    Style::default().bg(Color::Rgb(35, 36, 40)),
                ));
                i = close + 3;
                plain_start = i;
                continue;
            }
        }

        // ── inline code  `...` ───────────────────────────────────────────────
        if bytes[i] == b'`' {
            if let Some(close) = find_byte(bytes, b'`', i + 1) {
                flush_plain!(i);
                let code_text = &input[i + 1..close];
                spans.push(Span::styled(
                    code_text,
                    Style::default().bg(Color::Rgb(35, 36, 40)),
                ));
                i = close + 1;
                plain_start = i;
                continue;
            }
        }

        // ── bold  **...** ────────────────────────────────────────────────────
        if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'*' {
            if let Some(close) = find_str(input, "**", i + 2) {
                flush_plain!(i);
                let text = &input[i + 2..close];
                spans.push(Span::styled(
                    text,
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                i = close + 2;
                plain_start = i;
            } else {
                // no closing **, skip both chars as plain text
                i += 2;
            }
            continue;
        }

        // ── italic  *...* ────────────────────────────────────────────────────
        if bytes[i] == b'*' {
            if let Some(close) = find_byte(bytes, b'*', i + 1) {
                flush_plain!(i);
                let text = &input[i + 1..close];
                spans.push(Span::styled(
                    text,
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
                i = close + 1;
                plain_start = i;
                continue;
            }
        }

        // ── underline  __...__ ───────────────────────────────────────────────
        if bytes[i] == b'_' && i + 1 < len && bytes[i + 1] == b'_' {
            if let Some(close) = find_str(input, "__", i + 2) {
                flush_plain!(i);
                let text = &input[i + 2..close];
                spans.push(Span::styled(
                    text,
                    Style::default().add_modifier(Modifier::UNDERLINED),
                ));
                i = close + 2;
                plain_start = i;
                continue;
            }
        }

        // ── italic  _..._ ────────────────────────────────────────────────────
        if bytes[i] == b'_' {
            if let Some(close) = find_byte(bytes, b'_', i + 1) {
                flush_plain!(i);
                let text = &input[i + 1..close];
                spans.push(Span::styled(
                    text,
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
                i = close + 1;
                plain_start = i;
                continue;
            }
        }

        // ── strikethrough  ~~...~~ ───────────────────────────────────────────
        if bytes[i] == b'~' && i + 1 < len && bytes[i + 1] == b'~' {
            if let Some(close) = find_str(input, "~~", i + 2) {
                flush_plain!(i);
                let text = &input[i + 2..close];
                spans.push(Span::styled(
                    text,
                    Style::default().add_modifier(Modifier::CROSSED_OUT),
                ));
                i = close + 2;
                plain_start = i;
                continue;
            }
        }

        // ── spoiler  ||...|| ─────────────────────────────────────────────────
        if bytes[i] == b'|' && i + 1 < len && bytes[i + 1] == b'|' {
            if let Some(close) = find_str(input, "||", i + 2) {
                flush_plain!(i);
                spans.push(Span::styled(
                    "[spoiler]",
                    Style::default().fg(theme::TEXT_MUTED),
                ));
                i = close + 2;
                plain_start = i;
                continue;
            }
        }

        // ── role mention  <@&id> ─────────────────────────────────────────────
        if bytes[i] == b'<' && i + 2 < len && bytes[i + 1] == b'@' && bytes[i + 2] == b'&' {
            if let Some(close) = find_byte(bytes, b'>', i + 3) {
                flush_plain!(i);
                spans.push(Span::styled(
                    "@role",
                    Style::default().fg(theme::ACCENT),
                ));
                i = close + 1;
                plain_start = i;
                continue;
            }
        }

        // ── user mention  <@id> or <@!id> ────────────────────────────────────
        if bytes[i] == b'<' && i + 1 < len && bytes[i + 1] == b'@' {
            if let Some(close) = find_byte(bytes, b'>', i + 2) {
                flush_plain!(i);
                spans.push(Span::styled(
                    "@mention",
                    Style::default().fg(theme::ACCENT),
                ));
                i = close + 1;
                plain_start = i;
                continue;
            }
        }

        // ── channel mention  <#id> ───────────────────────────────────────────
        if bytes[i] == b'<' && i + 1 < len && bytes[i + 1] == b'#' {
            if let Some(close) = find_byte(bytes, b'>', i + 2) {
                flush_plain!(i);
                spans.push(Span::styled(
                    "#channel",
                    Style::default().fg(theme::ACCENT),
                ));
                i = close + 1;
                plain_start = i;
                continue;
            }
        }

        // ── URL  http(s)://... ───────────────────────────────────────────────
        if (bytes[i] == b'h')
            && input[i..].starts_with("https://")
            || (bytes[i] == b'h' && input[i..].starts_with("http://"))
        {
            let end = url_end(input, i);
            flush_plain!(i);
            let url = &input[i..end];
            spans.push(Span::styled(url, Style::default().fg(theme::LINK)));
            i = end;
            plain_start = i;
            continue;
        }

        i += 1;
    }

    // flush any remaining plain text
    flush_plain!(len);

    spans
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Find the next occurrence of `needle` in `haystack` starting at byte offset `from`.
fn find_str(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    haystack[from..].find(needle).map(|rel| rel + from)
}

/// Find the next occurrence of `byte` in `bytes` starting at index `from`.
fn find_byte(bytes: &[u8], byte: u8, from: usize) -> Option<usize> {
    bytes[from..].iter().position(|&b| b == byte).map(|rel| rel + from)
}

/// Return the byte index just past the end of the URL starting at `start`.
fn url_end(s: &str, start: usize) -> usize {
    let tail = &s[start..];
    let end_rel = tail
        .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '`'))
        .unwrap_or(tail.len());
    start + end_rel
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier, Style};

    fn content<'a>(spans: &'a [Span<'a>]) -> Vec<&'a str> {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn plain_text() {
        let spans = parse("hello world");
        assert_eq!(content(&spans), vec!["hello world"]);
        assert_eq!(spans[0].style, Style::default());
    }

    #[test]
    fn bold() {
        let spans = parse("**bold**");
        assert_eq!(content(&spans), vec!["bold"]);
        assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn italic_star() {
        let spans = parse("*italic*");
        assert_eq!(content(&spans), vec!["italic"]);
        assert!(spans[0].style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn italic_underscore() {
        let spans = parse("_italic_");
        assert_eq!(content(&spans), vec!["italic"]);
        assert!(spans[0].style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn underline_double_underscore() {
        let spans = parse("__underline__");
        assert_eq!(content(&spans), vec!["underline"]);
        assert!(spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn underline_does_not_apply_italic() {
        let spans = parse("__underline__");
        assert!(!spans[0].style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn underline_in_sentence() {
        let spans = parse("hello __world__ there");
        assert_eq!(content(&spans), vec!["hello ", "world", " there"]);
        assert!(spans[1].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn single_underscore_still_italic() {
        // Single underscore should still produce italic, not underline.
        let spans = parse("_italic_");
        assert!(spans[0].style.add_modifier.contains(Modifier::ITALIC));
        assert!(!spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn inline_code() {
        let spans = parse("`code`");
        assert_eq!(content(&spans), vec!["code"]);
        assert_eq!(spans[0].style.bg, Some(Color::Rgb(35, 36, 40)));
    }

    #[test]
    fn strikethrough() {
        let spans = parse("~~strike~~");
        assert_eq!(content(&spans), vec!["strike"]);
        assert!(spans[0].style.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn mixed_bold_in_sentence() {
        let spans = parse("hello **bold** world");
        assert_eq!(content(&spans), vec!["hello ", "bold", " world"]);
        assert_eq!(spans[0].style, Style::default());
        assert!(spans[1].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(spans[2].style, Style::default());
    }

    #[test]
    fn spoiler() {
        let spans = parse("||secret||");
        assert_eq!(content(&spans), vec!["[spoiler]"]);
        assert_eq!(spans[0].style.fg, Some(theme::TEXT_MUTED));
    }

    #[test]
    fn user_mention() {
        let spans = parse("<@123456>");
        assert_eq!(content(&spans), vec!["@mention"]);
        assert_eq!(spans[0].style.fg, Some(theme::ACCENT));
    }

    #[test]
    fn user_mention_nickname() {
        // <@!id> is a nickname/member mention
        let spans = parse("<@!789>");
        assert_eq!(content(&spans), vec!["@mention"]);
        assert_eq!(spans[0].style.fg, Some(theme::ACCENT));
    }

    #[test]
    fn role_mention() {
        let spans = parse("<@&111>");
        assert_eq!(content(&spans), vec!["@role"]);
        assert_eq!(spans[0].style.fg, Some(theme::ACCENT));
    }

    #[test]
    fn channel_mention() {
        let spans = parse("<#456>");
        assert_eq!(content(&spans), vec!["#channel"]);
        assert_eq!(spans[0].style.fg, Some(theme::ACCENT));
    }

    #[test]
    fn url_https() {
        let spans = parse("https://example.com");
        assert_eq!(content(&spans), vec!["https://example.com"]);
        assert_eq!(spans[0].style.fg, Some(theme::LINK));
    }

    #[test]
    fn url_http() {
        let spans = parse("http://example.com");
        assert_eq!(content(&spans), vec!["http://example.com"]);
        assert_eq!(spans[0].style.fg, Some(theme::LINK));
    }

    #[test]
    fn url_in_sentence() {
        let spans = parse("see https://example.com here");
        assert_eq!(content(&spans), vec!["see ", "https://example.com", " here"]);
    }

    #[test]
    fn code_block_no_lang() {
        let spans = parse("```\nsome code\n```");
        // no language label, just the code body
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "some code\n");
        assert_eq!(spans[0].style.bg, Some(Color::Rgb(35, 36, 40)));
    }

    #[test]
    fn code_block_with_lang() {
        let spans = parse("```rust\nlet x = 1;\n```");
        // first span: language label, second: code
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content.as_ref(), "[rust] ");
        assert_eq!(spans[1].content.as_ref(), "let x = 1;\n");
        assert_eq!(spans[1].style.bg, Some(Color::Rgb(35, 36, 40)));
    }

    #[test]
    fn unclosed_marker_is_plain() {
        // no closing **, should degrade to plain text
        let spans = parse("**unclosed");
        assert_eq!(content(&spans), vec!["**unclosed"]);
    }

    #[test]
    fn empty_input() {
        let spans = parse("");
        assert!(spans.is_empty());
    }

    #[test]
    fn blockquote() {
        let spans = parse("> hello");
        assert_eq!(content(&spans), vec!["│ ", "hello"]);
        assert_eq!(spans[0].style.fg, Some(theme::TEXT_MUTED));
        assert_eq!(spans[1].style.fg, Some(theme::TEXT_MUTED));
    }

    #[test]
    fn blockquote_mid_text() {
        // A "> " that is NOT at the start of a line should be treated as plain text.
        let spans = parse("hello > world");
        assert_eq!(content(&spans), vec!["hello > world"]);
    }

    #[test]
    fn masked_link() {
        let spans = parse("[click here](https://example.com)");
        assert_eq!(content(&spans), vec!["click here"]);
        assert_eq!(spans[0].style.fg, Some(theme::LINK));
        assert!(spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn masked_link_in_sentence() {
        let spans = parse("see [docs](https://docs.rs) for details");
        assert_eq!(content(&spans), vec!["see ", "docs", " for details"]);
        assert_eq!(spans[1].style.fg, Some(theme::LINK));
    }
}
