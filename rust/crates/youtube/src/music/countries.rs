#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChartCountry {
    pub code: &'static str,
    pub name: &'static str,
}

const fn country(code: &'static str, name: &'static str) -> ChartCountry {
    ChartCountry { code, name }
}

pub const GLOBAL_CHARTS: &str = "ZZ";
pub const FALLBACK_COUNTRY: &str = "BR";

pub const CHART_COUNTRIES: [ChartCountry; 69] = [
    country(GLOBAL_CHARTS, "Global"),
    country("AR", "Argentina"),
    country("AU", "Australia"),
    country("AT", "Austria"),
    country("BE", "Belgium"),
    country("BO", "Bolivia"),
    country("BR", "Brazil"),
    country("CA", "Canada"),
    country("CL", "Chile"),
    country("CO", "Colombia"),
    country("CR", "Costa Rica"),
    country("CZ", "Czechia"),
    country("DK", "Denmark"),
    country("DO", "Dominican Republic"),
    country("EC", "Ecuador"),
    country("EG", "Egypt"),
    country("SV", "El Salvador"),
    country("EE", "Estonia"),
    country("FI", "Finland"),
    country("FR", "France"),
    country("DE", "Germany"),
    country("GT", "Guatemala"),
    country("HN", "Honduras"),
    country("HK", "Hong Kong"),
    country("HU", "Hungary"),
    country("IS", "Iceland"),
    country("IN", "India"),
    country("ID", "Indonesia"),
    country("IE", "Ireland"),
    country("IL", "Israel"),
    country("IT", "Italy"),
    country("JP", "Japan"),
    country("KE", "Kenya"),
    country("LU", "Luxembourg"),
    country("MY", "Malaysia"),
    country("MX", "Mexico"),
    country("NL", "Netherlands"),
    country("NZ", "New Zealand"),
    country("NI", "Nicaragua"),
    country("NG", "Nigeria"),
    country("NO", "Norway"),
    country("PA", "Panama"),
    country("PY", "Paraguay"),
    country("PE", "Peru"),
    country("PH", "Philippines"),
    country("PL", "Poland"),
    country("PT", "Portugal"),
    country("RO", "Romania"),
    country("RU", "Russia"),
    country("SA", "Saudi Arabia"),
    country("RS", "Serbia"),
    country("SG", "Singapore"),
    country("ZA", "South Africa"),
    country("KR", "South Korea"),
    country("ES", "Spain"),
    country("SE", "Sweden"),
    country("CH", "Switzerland"),
    country("TW", "Taiwan"),
    country("TZ", "Tanzania"),
    country("TH", "Thailand"),
    country("TR", "Turkey"),
    country("UG", "Uganda"),
    country("UA", "Ukraine"),
    country("AE", "United Arab Emirates"),
    country("GB", "United Kingdom"),
    country("US", "United States"),
    country("UY", "Uruguay"),
    country("VN", "Vietnam"),
    country("ZW", "Zimbabwe"),
];

pub fn chart_country(code: &str) -> Option<ChartCountry> {
    let code = code.trim();
    CHART_COUNTRIES
        .into_iter()
        .find(|country| country.code.eq_ignore_ascii_case(code))
}

pub fn chart_country_or_fallback(preferred: &[Option<&str>]) -> ChartCountry {
    preferred
        .iter()
        .flatten()
        .find_map(|code| chart_country(code))
        .or_else(|| chart_country(FALLBACK_COUNTRY))
        .unwrap_or(country(FALLBACK_COUNTRY, "Brazil"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countries_are_found_by_code_in_any_case() {
        assert_eq!(
            chart_country("us").map(|found| found.name),
            Some("United States")
        );
        assert_eq!(chart_country("ZZ").map(|found| found.name), Some("Global"));
        assert_eq!(chart_country("XX"), None);
    }

    #[test]
    fn the_first_known_country_wins_and_brazil_is_the_fallback() {
        assert_eq!(chart_country_or_fallback(&[None, Some("de")]).code, "DE");
        assert_eq!(
            chart_country_or_fallback(&[Some("XX"), Some("FR")]).code,
            "FR"
        );
        assert_eq!(chart_country_or_fallback(&[Some("XX"), None]).code, "BR");
    }

    #[test]
    fn country_codes_are_unique() {
        let mut codes: Vec<_> = CHART_COUNTRIES.iter().map(|country| country.code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), CHART_COUNTRIES.len());
    }
}
