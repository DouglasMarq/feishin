use super::credential::encode_uri_component;

#[derive(Debug, Default, Clone)]
pub struct Params(Vec<(&'static str, String)>);

impl Params {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(mut self, key: &'static str, value: impl ToString) -> Self {
        self.0.push((key, value.to_string()));
        self
    }

    pub fn add_optional(self, key: &'static str, value: Option<impl ToString>) -> Self {
        match value {
            Some(present) => self.add(key, present),
            None => self,
        }
    }

    pub fn add_each(mut self, key: &'static str, values: &[String]) -> Self {
        self.0
            .extend(values.iter().map(|value| (key, value.clone())));
        self
    }

    pub(super) fn append_encoded_to(&self, query: &mut String) {
        for (key, value) in &self.0 {
            query.push('&');
            query.push_str(key);
            query.push('=');
            query.push_str(&encode_uri_component(value));
        }
    }
}
