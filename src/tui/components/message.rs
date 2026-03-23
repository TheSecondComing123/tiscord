use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::store::messages::{ReactionEmoji, StoredMessage};
use crate::store::ThreadInfo;
use crate::tui::theme;
use crate::utils::time::format_timestamp;

/// Render a single `StoredMessage` into a sequence of ratatui `Line`s.
///
/// Order: optional reply quote, header, content lines, attachments, optional thread indicator, reactions.
///
/// `thread` is optional thread info if this message has an associated thread.
/// `supports_images` controls whether image attachments show an image indicator vs. plain file indicator.
pub fn render_message(msg: &StoredMessage, _width: u16) -> Vec<Line<'static>> {
    render_message_with_thread(msg, _width, None, false)
}

/// Like `render_message` but also accepts optional thread info to display a thread indicator
/// and a flag indicating whether the terminal supports inline image rendering.
pub fn render_message_with_thread(msg: &StoredMessage, _width: u16, thread: Option<&ThreadInfo>, supports_images: bool) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // ── reply context ────────────────────────────────────────────────────────
    if let Some(reply) = &msg.reply_to {
        let quote = format!(
            "> replying to @{}: {}",
            reply.author_name, reply.content_preview
        );
        lines.push(Line::from(Span::styled(quote, theme::muted())));
    }

    // ── header: author  timestamp ────────────────────────────────────────────
    let author_style = Style::default()
        .fg(theme::ACCENT)
        .add_modifier(Modifier::BOLD);
    let timestamp_text = format!("  {}", format_timestamp(&msg.timestamp));
    let header = Line::from(vec![
        Span::styled(msg.author_name.clone(), author_style),
        Span::styled(timestamp_text, theme::muted()),
    ]);
    lines.push(header);

    // ── content ──────────────────────────────────────────────────────────────
    let raw_lines: Vec<&str> = msg.content.split('\n').collect();
    let total = raw_lines.len();

    for (idx, raw_line) in raw_lines.iter().enumerate() {
        // Parse inline markdown for this line; convert borrowed spans to owned.
        let parsed = crate::tui::markdown::parse(raw_line);
        let mut spans: Vec<Span<'static>> = parsed
            .into_iter()
            .map(|s| Span::styled(s.content.into_owned(), s.style))
            .collect();

        // Append "(edited)" to the last content line.
        let is_last = idx == total - 1;
        if is_last && msg.is_edited {
            spans.push(Span::styled(" (edited)", theme::muted()));
        }

        lines.push(Line::from(spans));
    }

    // ── attachments ──────────────────────────────────────────────────────────
    for attachment in &msg.attachments {
        let size_str = format_size(attachment.size);
        let text = if supports_images && crate::tui::image_renderer::is_image_file(&attachment.filename) {
            format!("[\u{1f5bc} {} ({})]", attachment.filename, size_str)
        } else {
            format!("[file: {} ({})]", attachment.filename, size_str)
        };
        lines.push(Line::from(Span::styled(text, theme::secondary_text())));
    }

    // ── thread indicator ─────────────────────────────────────────────────────
    if let Some(t) = thread {
        let text = format!("\u{1f9f5} {} ({} replies)", t.name, t.message_count);
        lines.push(Line::from(Span::styled(text, Style::default().fg(theme::ACCENT))));
    }

    // ── reactions ────────────────────────────────────────────────────────────
    if !msg.reactions.is_empty() {
        let reaction_spans: Vec<Span> = msg.reactions.iter().flat_map(|r| {
            let emoji_str = match &r.emoji {
                ReactionEmoji::Unicode(e) => e.clone(),
                ReactionEmoji::Custom { name, .. } => format!(":{}:", name),
            };
            let style = if r.me {
                Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::TEXT_MUTED)
            };
            vec![
                Span::styled(format!("[{} {}]", emoji_str, r.count), style),
                Span::raw(" "),
            ]
        }).collect();
        lines.push(Line::from(reaction_spans));
    }

    lines
}

/// Format a byte count as a human-readable string.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;

    if bytes >= MB {
        let mb = bytes as f64 / MB as f64;
        format!("{:.1} MB", mb)
    } else if bytes >= KB {
        let kb = bytes as f64 / KB as f64;
        format!("{:.1} KB", kb)
    } else {
        format!("{} B", bytes)
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::messages::{Attachment, ReplyContext, StoredMessage};
    use twilight_model::id::Id;

    fn make_msg(id: u64, content: &str) -> StoredMessage {
        StoredMessage {
            id: Id::new(id),
            author_name: "alice".to_string(),
            author_id: Id::new(id),
            content: content.to_string(),
            timestamp: "2025-06-01T12:00:00+00:00".to_string(),
            reply_to: None,
            attachments: vec![],
            is_edited: false,
            reactions: vec![],
        }
    }

    /// Extract the plain text content of all spans across all lines.
    fn all_text(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    // ── simple message ───────────────────────────────────────────────────────

    #[test]
    fn simple_message_renders_header_and_content() {
        let msg = make_msg(1, "hello");
        let lines = render_message(&msg, 80);

        // Expect: header line, one content line
        assert_eq!(lines.len(), 2);

        // Header contains author name
        let header_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(header_text.contains("alice"), "header missing author");

        // Header's first span (author) is bold + accent
        let author_span = &lines[0].spans[0];
        assert_eq!(author_span.style.fg, Some(theme::ACCENT));
        assert!(author_span.style.add_modifier.contains(Modifier::BOLD));

        // Content line
        let content_text: String = lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(content_text, "hello");
    }

    // ── reply context ────────────────────────────────────────────────────────

    #[test]
    fn message_with_reply_shows_quote_first() {
        let mut msg = make_msg(2, "got it");
        msg.reply_to = Some(ReplyContext {
            author_name: "bob".to_string(),
            content_preview: "original message".to_string(),
        });

        let lines = render_message(&msg, 80);

        // Expect: reply line, header line, content line
        assert_eq!(lines.len(), 3);

        let reply_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(reply_text.contains("> replying to @bob"));
        assert!(reply_text.contains("original message"));

        // Reply line should be muted
        assert_eq!(lines[0].spans[0].style.fg, Some(theme::TEXT_MUTED));
    }

    // ── attachments ──────────────────────────────────────────────────────────

    #[test]
    fn message_with_attachments_shows_file_info() {
        let mut msg = make_msg(3, "see attached");
        msg.attachments = vec![
            Attachment {
                filename: "photo.png".to_string(),
                size: 204_800, // 200 KB
                url: "https://example.com/photo.png".to_string(),
            },
            Attachment {
                filename: "tiny.txt".to_string(),
                size: 512, // 512 B
                url: "https://example.com/tiny.txt".to_string(),
            },
        ];

        let lines = render_message(&msg, 80);
        // header + content + 2 attachments = 4 lines
        assert_eq!(lines.len(), 4);

        let texts = all_text(&lines);
        assert!(texts[2].contains("photo.png"), "first attachment missing");
        assert!(texts[2].contains("200.0 KB"), "KB formatting wrong");
        assert!(texts[3].contains("tiny.txt"), "second attachment missing");
        assert!(texts[3].contains("512 B"), "byte formatting wrong");
    }

    #[test]
    fn attachment_size_mb() {
        let mut msg = make_msg(4, "big file");
        msg.attachments = vec![Attachment {
            filename: "video.mp4".to_string(),
            size: 5 * 1024 * 1024, // 5 MB
            url: "https://example.com/video.mp4".to_string(),
        }];

        let lines = render_message(&msg, 80);
        let texts = all_text(&lines);
        assert!(texts[2].contains("5.0 MB"), "MB formatting wrong");
    }

    // ── edited indicator ─────────────────────────────────────────────────────

    #[test]
    fn edited_message_shows_indicator() {
        let mut msg = make_msg(5, "whoops typo");
        msg.is_edited = true;

        let lines = render_message(&msg, 80);
        // header + one content line
        assert_eq!(lines.len(), 2);

        let content_text: String = lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            content_text.contains("(edited)"),
            "edited indicator missing"
        );
    }

    // ── multiline content ────────────────────────────────────────────────────

    #[test]
    fn multiline_content_produces_multiple_lines() {
        let msg = make_msg(6, "line one\nline two\nline three");
        let lines = render_message(&msg, 80);

        // header + 3 content lines
        assert_eq!(lines.len(), 4);

        let texts = all_text(&lines);
        assert_eq!(texts[1], "line one");
        assert_eq!(texts[2], "line two");
        assert_eq!(texts[3], "line three");
    }

    #[test]
    fn edited_indicator_on_last_line_of_multiline() {
        let mut msg = make_msg(7, "first\nsecond");
        msg.is_edited = true;

        let lines = render_message(&msg, 80);
        // header + 2 content lines
        assert_eq!(lines.len(), 3);

        // "(edited)" should be on the last content line, not the first
        let first_content: String = lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        let last_content: String = lines[2]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(!first_content.contains("(edited)"));
        assert!(last_content.contains("(edited)"));
    }

    // ── format_size helper ───────────────────────────────────────────────────

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(2048), "2.0 KB");
    }

    #[test]
    fn format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(3 * 1024 * 1024), "3.0 MB");
    }
}
