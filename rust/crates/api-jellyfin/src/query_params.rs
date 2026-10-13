const MAX_LISTED_YEARS: i64 = 100;

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

    pub fn year_range(self, from: Option<i32>, to: Option<i32>) -> Self {
        match (nonzero_year(from), nonzero_year(to)) {
            (Some(first), Some(last)) if lists_every_year(first, last) => {
                self.set_optional("Years", comma_separated_years(first, last))
            }
            (first, last) => self
                .set_optional("MinPremiereDate", first.map(first_instant_of))
                .set_optional("MaxPremiereDate", last.map(last_instant_of)),
        }
    }

    pub fn pairs(&self) -> &[(&'static str, String)] {
        &self.0
    }
}

pub fn comma_separated_ids(ids: &[String]) -> Option<String> {
    (!ids.is_empty()).then(|| ids.join(","))
}

fn nonzero_year(year: Option<i32>) -> Option<i32> {
    year.filter(|year| *year != 0)
}

fn lists_every_year(first: i32, last: i32) -> bool {
    i64::from(last) - i64::from(first) < MAX_LISTED_YEARS
}

fn first_instant_of(year: i32) -> String {
    format!("{year:04}-01-01T00:00:00Z")
}

fn last_instant_of(year: i32) -> String {
    format!("{year:04}-12-31T23:59:59Z")
}

fn comma_separated_years(first_year: i32, last_year: i32) -> Option<String> {
    let years: Vec<String> = (first_year..=last_year)
        .map(|year| year.to_string())
        .collect();
    (!years.is_empty()).then(|| years.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn year_pairs(from: Option<i32>, to: Option<i32>) -> Vec<(&'static str, String)> {
        QueryParams::default().year_range(from, to).pairs().to_vec()
    }

    #[test]
    fn closed_ranges_list_every_year() {
        assert_eq!(
            year_pairs(Some(2020), Some(2022)),
            [("Years", "2020,2021,2022".to_owned())]
        );
    }

    #[test]
    fn open_ranges_filter_on_the_premiere_date() {
        assert_eq!(
            year_pairs(Some(2022), None),
            [("MinPremiereDate", "2022-01-01T00:00:00Z".to_owned())]
        );
        assert_eq!(
            year_pairs(None, Some(2019)),
            [("MaxPremiereDate", "2019-12-31T23:59:59Z".to_owned())]
        );
        assert!(year_pairs(None, None).is_empty());
    }

    #[test]
    fn wide_closed_ranges_filter_on_the_premiere_date() {
        assert_eq!(
            year_pairs(Some(1), Some(9999)),
            [
                ("MinPremiereDate", "0001-01-01T00:00:00Z".to_owned()),
                ("MaxPremiereDate", "9999-12-31T23:59:59Z".to_owned())
            ]
        );
    }
}
