//! Unicode character budgets include separators, escaping and data envelopes.
const TRUNCATION: &str = "\n[Context truncated]";

pub(super) struct RenderedData {
    pub text: String,
    pub chars: usize,
    pub truncated: bool,
}

fn escaped_char(character: char) -> String {
    match character {
        '&' => "&amp;".into(),
        '<' => "&lt;".into(),
        '>' => "&gt;".into(),
        _ => character.to_string(),
    }
}

/// Source is a host-owned constant. Content cannot close its data envelope,
/// even when a cut falls inside text that resembles a closing tag or entity.
pub(super) fn render_data(source: &str, content: &str, limit: usize) -> RenderedData {
    render_envelope(
        &format!("untrusted_data source=\"{source}\""),
        "untrusted_data",
        content,
        limit,
    )
}

pub(super) fn render_instructions(content: &str, limit: usize) -> RenderedData {
    render_envelope(
        "connection_instructions",
        "connection_instructions",
        content,
        limit,
    )
}

fn render_envelope(tag: &str, closing_tag: &str, content: &str, limit: usize) -> RenderedData {
    if content.is_empty() {
        return RenderedData {
            text: String::new(),
            chars: 0,
            truncated: false,
        };
    }
    let open = format!("\n\n<{tag}>\n");
    let close = format!("\n</{closing_tag}>");
    let overhead = open.chars().count() + close.chars().count();
    let available = limit.saturating_sub(overhead);
    // Count only up to the limit, rather than allocating an escaped copy first.
    let escaped_size = content
        .chars()
        .map(|character| match character {
            '&' => 5,
            '<' | '>' => 4,
            _ => 1,
        })
        .try_fold(0usize, |sum, count| {
            let size = sum.saturating_add(count);
            (size <= available).then_some(size)
        });
    let truncated = escaped_size.is_none();
    let marker_chars = if truncated {
        TRUNCATION.chars().count()
    } else {
        0
    };
    if limit < overhead + marker_chars {
        return RenderedData {
            text: String::new(),
            chars: 0,
            truncated: true,
        };
    }
    let body_limit = available - marker_chars;
    let mut text = open;
    let mut body_chars = 0;
    for character in content.chars() {
        let escaped = escaped_char(character);
        let size = escaped.chars().count();
        if size > body_limit.saturating_sub(body_chars) {
            break;
        }
        body_chars += size;
        text.push_str(&escaped);
    }
    if truncated {
        text.push_str(TRUNCATION);
    }
    text.push_str(&close);
    RenderedData {
        text,
        chars: overhead + body_chars + marker_chars,
        truncated,
    }
}
