//! Text formats as Project shows them in pt-BR: "Qui 22/10/26", "3 dias", "2,5 dias".

use chrono::{Datelike, NaiveDate, NaiveDateTime};

pub const WEEKDAYS: [&str; 7] = ["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"];
pub const DAY_LETTERS: [&str; 7] = ["D", "S", "T", "Q", "Q", "S", "S"];
pub const MONTHS: [&str; 12] = ["Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez"];

/// "Qui 22/10/26".
pub fn date(dt: NaiveDateTime) -> String {
    format!("{} {}", WEEKDAYS[dt.weekday().num_days_from_sunday() as usize], dt.format("%d/%m/%y"))
}

/// "11/Out/26" (timescale week label).
pub fn week_label(d: NaiveDate) -> String {
    format!("{:02}/{}/{}", d.day(), MONTHS[d.month0() as usize], d.format("%y"))
}

/// "1 dia", "3 dias", "2,5 dias", "0 dias".
pub fn days(days: f64) -> String {
    let rounded = (days * 100.0).round() / 100.0;
    let text = if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}").replace('.', ",")
    };
    if rounded == 1.0 { format!("{text} dia") } else { format!("{text} dias") }
}

/// Parses "3", "3d", "3 dias", "2,5 dia".
pub fn parse_days(text: &str) -> Option<f64> {
    let t = text.trim().to_lowercase();
    let number: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == ',' || *c == '.').collect();
    let rest = t[number.len()..].trim();
    if !(rest.is_empty() || rest.starts_with('d')) {
        return None;
    }
    number.replace(',', ".").parse().ok()
}

/// Parses "22/10/26", "22/10/2026" or "Qui 22/10/26".
pub fn parse_date(text: &str) -> Option<NaiveDate> {
    let t = text.trim();
    let t = t.rsplit(' ').next().unwrap_or(t);
    NaiveDate::parse_from_str(t, "%d/%m/%y").or_else(|_| NaiveDate::parse_from_str(t, "%d/%m/%Y")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_project() {
        let dt = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap().and_hms_opt(9, 0, 0).unwrap();
        assert_eq!(date(dt), "Qui 22/10/26");
        assert_eq!(days(1.0), "1 dia");
        assert_eq!(days(2.5), "2,5 dias");
        assert_eq!(parse_days("3 dias"), Some(3.0));
        assert_eq!(parse_days("2,5"), Some(2.5));
        assert_eq!(parse_days("abc"), None);
        assert_eq!(parse_date("Qui 22/10/26"), NaiveDate::from_ymd_opt(2026, 10, 22));
        assert_eq!(week_label(NaiveDate::from_ymd_opt(2026, 10, 11).unwrap()), "11/Out/26");
    }
}
