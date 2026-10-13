use std::cmp::Ordering;

#[derive(PartialEq, Eq)]
pub enum SortValue {
    Text(Option<String>),
    Number(Option<i64>),
}

struct SortField {
    value: SortValue,
    descending: bool,
}

struct SortKey(Vec<SortField>);

fn compare_missing_last<T: Ord>(left: Option<&T>, right: Option<&T>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_values(left: &SortValue, right: &SortValue) -> Ordering {
    match (left, right) {
        (SortValue::Text(left), SortValue::Text(right)) => {
            compare_missing_last(left.as_ref(), right.as_ref())
        }
        (SortValue::Number(left), SortValue::Number(right)) => {
            compare_missing_last(left.as_ref(), right.as_ref())
        }
        _ => Ordering::Equal,
    }
}

fn compare_fields(left: &SortField, right: &SortField) -> Ordering {
    let ordering = compare_values(&left.value, &right.value);
    match left.descending {
        true => ordering.reverse(),
        false => ordering,
    }
}

impl Ord for SortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .iter()
            .zip(&other.0)
            .map(|(left, right)| compare_fields(left, right))
            .find(|ordering| *ordering != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for SortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for SortKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for SortKey {}

pub fn text(value: Option<&str>) -> SortValue {
    SortValue::Text(value.map(str::to_lowercase))
}

pub fn number(value: Option<i64>) -> SortValue {
    SortValue::Number(value)
}

pub fn created_day(created_at: Option<&str>) -> SortValue {
    SortValue::Text(created_at.map(|timestamp| timestamp.chars().take(10).collect()))
}

pub fn order_by<T>(items: &mut [T], descending: bool, sort_values: impl Fn(&T) -> Vec<SortValue>) {
    items.sort_by_cached_key(|item| {
        SortKey(
            sort_values(item)
                .into_iter()
                .map(|value| SortField { value, descending })
                .collect(),
        )
    });
}
