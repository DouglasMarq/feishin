const ENTITIES: [(&str, &str); 7] = [
    ("&nbsp;", " "),
    ("&quot;", "\""),
    ("&#39;", "'"),
    ("&apos;", "'"),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&amp;", "&"),
];

fn breaks_line(tag: &str) -> bool {
    let tag = tag.to_ascii_lowercase();
    let name = tag.trim_start_matches('/').split([' ', '/']).next();
    matches!(name, Some("br" | "p" | "div" | "li"))
}

fn strip_tags(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some((before_tag, tag_and_rest)) = rest.split_once('<') {
        text.push_str(before_tag);
        let Some((tag, after_tag)) = tag_and_rest.split_once('>') else {
            return text;
        };
        if breaks_line(tag) {
            text.push('\n');
        }
        rest = after_tag;
    }
    text.push_str(rest);
    text
}

fn decode_entities(text: String) -> String {
    ENTITIES
        .iter()
        .fold(text, |text, (entity, value)| text.replace(entity, value))
}

fn paragraphs(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn plain_text(html: &str) -> String {
    paragraphs(&decode_entities(strip_tags(html)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biography_html_becomes_plain_paragraphs() {
        let html = "<p>Ada  Vale is a <i>folk</i> artist.</p><p>Small &amp; noisy rooms.<br/>\
                    <a href=\"https://www.last.fm\">Read more on Last.fm</a></p>";
        assert_eq!(
            plain_text(html),
            "Ada Vale is a folk artist.\n\nSmall & noisy rooms.\n\nRead more on Last.fm"
        );
        assert_eq!(plain_text("plain text"), "plain text");
        assert_eq!(plain_text(""), "");
        assert_eq!(plain_text("broken <b"), "broken");
    }
}
