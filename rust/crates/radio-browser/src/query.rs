pub const DEFAULT_PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StationQuery {
    pub name: String,
    pub country: String,
    pub tag: String,
    pub offset: usize,
    pub limit: usize,
}

impl Default for StationQuery {
    fn default() -> Self {
        Self {
            name: String::new(),
            country: String::new(),
            tag: String::new(),
            offset: 0,
            limit: DEFAULT_PAGE_SIZE,
        }
    }
}

fn non_empty_filter(key: &'static str, value: &str) -> Option<(&'static str, String)> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| (key, trimmed.to_owned()))
}

impl StationQuery {
    pub fn is_empty(&self) -> bool {
        [&self.name, &self.country, &self.tag]
            .iter()
            .all(|filter| filter.trim().is_empty())
    }

    pub fn next_page(&self) -> Self {
        Self {
            offset: self.offset.saturating_add(self.limit),
            ..self.clone()
        }
    }

    pub(crate) fn parameters(&self) -> Vec<(&'static str, String)> {
        let filters = [
            non_empty_filter("name", &self.name),
            non_empty_filter("country", &self.country),
            non_empty_filter("tag", &self.tag),
        ];
        let mut parameters: Vec<(&'static str, String)> = filters.into_iter().flatten().collect();
        parameters.extend([
            ("offset", self.offset.to_string()),
            ("limit", self.limit.to_string()),
            ("hidebroken", "true".to_owned()),
            ("order", "votes".to_owned()),
            ("reverse", "true".to_owned()),
        ]);
        parameters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_filters_are_left_out() {
        let query = StationQuery {
            name: " jazz ".into(),
            country: "  ".into(),
            ..StationQuery::default()
        };
        let parameters = query.parameters();
        assert!(parameters.contains(&("name", "jazz".to_owned())));
        assert!(
            !parameters
                .iter()
                .any(|(key, _)| *key == "country" || *key == "tag")
        );
        assert!(parameters.contains(&("hidebroken", "true".to_owned())));
    }

    #[test]
    fn next_page_moves_the_offset_by_the_limit() {
        let query = StationQuery {
            tag: "rock".into(),
            limit: 20,
            ..StationQuery::default()
        };
        let next = query.next_page();
        assert_eq!(
            (next.offset, next.limit, next.tag.as_str()),
            (20, 20, "rock")
        );
        assert!(!query.is_empty());
        assert!(StationQuery::default().is_empty());
    }
}
