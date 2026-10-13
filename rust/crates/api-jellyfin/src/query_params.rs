#[derive(Debug, Default, Clone)]
pub struct QueryParams(Vec<(&'static str, String)>);

impl QueryParams {
    pub fn set(mut self, key: &'static str, value: impl ToString) -> Self {
        self.0.push((key, value.to_string()));
        self
    }

    pub fn set_optional(self, key: &'static str, value: Option<impl ToString>) -> Self {
        match value {
            Some(value) => self.set(key, value),
            None => self,
        }
    }

    pub fn fields(self, fields: &[&str]) -> Self {
        fields
            .iter()
            .fold(self, |params, field| params.set("Fields", field))
    }

    pub fn pairs(&self) -> &[(&'static str, String)] {
        &self.0
    }
}

pub fn comma_separated_ids(ids: &[String]) -> Option<String> {
    (!ids.is_empty()).then(|| ids.join(","))
}

pub fn comma_separated_years(from: Option<i32>, to: Option<i32>) -> Option<String> {
    let first_year = from.filter(|year| *year != 0)?;
    let last_year = to.filter(|year| *year != 0)?;
    let years: Vec<String> = (first_year..=last_year)
        .map(|year| year.to_string())
        .collect();
    (!years.is_empty()).then(|| years.join(","))
}
