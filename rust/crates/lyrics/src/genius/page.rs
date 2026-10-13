use scraper::html::Select;
use scraper::{ElementRef, Html, Selector};

const LEGACY_LYRICS_SELECTOR: &str = "div.lyrics";
const LYRICS_CONTAINER_SELECTOR: &str = r#"div[data-lyrics-container="true"]"#;

pub(super) fn extract_lyrics(html: &str) -> String {
    let document = Html::parse_document(html);
    let (Ok(legacy_selector), Ok(container_selector)) = (
        Selector::parse(LEGACY_LYRICS_SELECTOR),
        Selector::parse(LYRICS_CONTAINER_SELECTOR),
    ) else {
        return String::new();
    };
    let legacy: Vec<ElementRef<'_>> = document.select(&legacy_selector).collect();
    if legacy.is_empty() {
        return container_text(document.select(&container_selector));
    }
    let mut text = String::new();
    for element in legacy {
        collect_text(element, &mut text);
    }
    text.trim().to_owned()
}

fn container_text(containers: Select<'_, '_>) -> String {
    containers
        .map(|element| {
            let mut text = String::new();
            collect_text(element, &mut text);
            text
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect_text(element: ElementRef<'_>, output: &mut String) {
    for child in element.children() {
        match (child.value().as_text(), ElementRef::wrap(child)) {
            (Some(text), _) => output.push_str(text),
            (None, Some(child_element)) => collect_element_text(child_element, output),
            (None, None) => {}
        }
    }
}

fn collect_element_text(element: ElementRef<'_>, output: &mut String) {
    let value = element.value();
    match value.name() {
        "br" => output.push('\n'),
        _ if value.attr("data-exclude-from-selection") == Some("true") => {}
        _ => collect_text(element, output),
    }
}
