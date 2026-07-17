//! Build-time generator for exchange and holiday calendar implementations.
//!
/// Generate calendar implementations from JSON definitions.
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct CalendarDef {
    id: String,
    name: String,
    ignore_weekends: Option<bool>,
    rules: Vec<RuleDef>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RuleDef {
    ExactDate {
        year: i32,
        month: MonthName,
        day: u8,
    },
    Fixed {
        month: MonthName,
        day: u8,
        observed: Option<ObservedName>,
        #[serde(default)]
        from_year: Option<i32>,
        #[serde(default)]
        to_year: Option<i32>,
    },
    EasterOffset {
        days: i16,
    },
    NthWeekday {
        n: i8,
        weekday: WeekdayName,
        month: MonthName,
        #[serde(default)]
        from_year: Option<i32>,
        #[serde(default)]
        to_year: Option<i32>,
    },
    WeekdayShift {
        weekday: WeekdayName,
        month: MonthName,
        day: u8,
        dir: DirectionName,
    },
    Span {
        start: Box<RuleDef>,
        len: u8,
        #[serde(default)]
        offset: i16,
        #[serde(default)]
        from_year: Option<i32>,
        #[serde(default)]
        to_year: Option<i32>,
    },
    ChineseNewYear,
    QingMing,
    BuddhasBirthday,
    DragonBoat,
    MidAutumn,
    ChinaBridge {
        festival: Box<RuleDef>,
        #[serde(default)]
        from_year: Option<i32>,
        #[serde(default)]
        to_year: Option<i32>,
    },
    VernalEquinoxJp,
    AutumnalEquinoxJp,
}

#[derive(Debug, Clone, Copy)]
enum ObservedName {
    NextMonday,
    FriIfSatMonIfSun,
    MonIfSun,
    MonIfSatTueIfSun,
}

#[derive(Debug, Clone, Copy)]
enum MonthName {
    January,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

#[derive(Debug, Clone, Copy)]
enum WeekdayName {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Debug, Clone, Copy)]
enum DirectionName {
    After,
    Before,
}

impl<'de> Deserialize<'de> for MonthName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?.to_lowercase();
        match s.as_str() {
            "january" => Ok(Self::January),
            "february" => Ok(Self::February),
            "march" => Ok(Self::March),
            "april" => Ok(Self::April),
            "may" => Ok(Self::May),
            "june" => Ok(Self::June),
            "july" => Ok(Self::July),
            "august" => Ok(Self::August),
            "september" => Ok(Self::September),
            "october" => Ok(Self::October),
            "november" => Ok(Self::November),
            "december" => Ok(Self::December),
            _ => Err(serde::de::Error::custom(format!("Unknown month: {s}"))),
        }
    }
}

impl<'de> Deserialize<'de> for WeekdayName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?.to_lowercase();
        match s.as_str() {
            "monday" => Ok(Self::Monday),
            "tuesday" => Ok(Self::Tuesday),
            "wednesday" => Ok(Self::Wednesday),
            "thursday" => Ok(Self::Thursday),
            "friday" => Ok(Self::Friday),
            "saturday" => Ok(Self::Saturday),
            "sunday" => Ok(Self::Sunday),
            _ => Err(serde::de::Error::custom(format!("Unknown weekday: {s}"))),
        }
    }
}

impl<'de> Deserialize<'de> for DirectionName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?.to_lowercase();
        match s.as_str() {
            "after" => Ok(Self::After),
            "before" => Ok(Self::Before),
            _ => Err(serde::de::Error::custom(format!("Unknown direction: {s}"))),
        }
    }
}

impl<'de> Deserialize<'de> for ObservedName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?.to_lowercase();
        match s.as_str() {
            "next_monday" => Ok(Self::NextMonday),
            "fri_if_sat_mon_if_sun" => Ok(Self::FriIfSatMonIfSun),
            "mon_if_sun" => Ok(Self::MonIfSun),
            "mon_if_sat_tue_if_sun" => Ok(Self::MonIfSatTueIfSun),
            _ => Err(serde::de::Error::custom(format!("Unknown observed: {s}"))),
        }
    }
}

impl MonthName {
    fn to_rust_code(self) -> &'static str {
        match self {
            Self::January => "Month::January",
            Self::February => "Month::February",
            Self::March => "Month::March",
            Self::April => "Month::April",
            Self::May => "Month::May",
            Self::June => "Month::June",
            Self::July => "Month::July",
            Self::August => "Month::August",
            Self::September => "Month::September",
            Self::October => "Month::October",
            Self::November => "Month::November",
            Self::December => "Month::December",
        }
    }
}

impl WeekdayName {
    fn to_rust_code(self) -> &'static str {
        match self {
            Self::Monday => "Weekday::Monday",
            Self::Tuesday => "Weekday::Tuesday",
            Self::Wednesday => "Weekday::Wednesday",
            Self::Thursday => "Weekday::Thursday",
            Self::Friday => "Weekday::Friday",
            Self::Saturday => "Weekday::Saturday",
            Self::Sunday => "Weekday::Sunday",
        }
    }
}

impl DirectionName {
    fn to_rust_code(self) -> &'static str {
        match self {
            Self::After => "Direction::After",
            Self::Before => "Direction::Before",
        }
    }
}

impl ObservedName {
    fn to_rust_code(self) -> &'static str {
        match self {
            Self::NextMonday => "Observed::NextMonday",
            Self::FriIfSatMonIfSun => "Observed::FriIfSatMonIfSun",
            Self::MonIfSun => "Observed::MonIfSun",
            Self::MonIfSatTueIfSun => "Observed::MonIfSatTueIfSun",
        }
    }
}

/// Wrap an inner rule's generated code in a `Rule::Effective { .. }` gate when
/// either bound is present; otherwise return the inner code unchanged.
fn wrap_effective(inner: String, from_year: Option<i32>, to_year: Option<i32>) -> String {
    if from_year.is_none() && to_year.is_none() {
        return inner;
    }
    let render = |b: Option<i32>| match b {
        Some(y) => format!("Some({y})"),
        None => "None".to_string(),
    };
    format!(
        "Rule::Effective {{ from_year: {}, to_year: {}, inner: &({}) }}",
        render(from_year),
        render(to_year),
        inner,
    )
}

impl RuleDef {
    fn to_rust_code(&self) -> String {
        match self {
            RuleDef::ExactDate { year, month, day } => format!(
                "Rule::ExactDate {{ year: {}, month: {}, day: {} }}",
                year,
                month.to_rust_code(),
                day
            ),
            RuleDef::Fixed {
                month,
                day,
                observed,
                from_year,
                to_year,
            } => {
                let inner = match observed {
                    None => format!("Rule::fixed({}, {})", month.to_rust_code(), day),
                    Some(obs) => format!(
                        "Rule::Fixed {{ month: {}, day: {}, observed: {} }}",
                        month.to_rust_code(),
                        day,
                        obs.to_rust_code()
                    ),
                };
                wrap_effective(inner, *from_year, *to_year)
            }
            RuleDef::EasterOffset { days } => {
                format!("Rule::EasterOffset({})", days)
            }
            RuleDef::NthWeekday {
                n,
                weekday,
                month,
                from_year,
                to_year,
            } => {
                let inner = format!(
                    "Rule::NthWeekday {{ n: {}, weekday: {}, month: {} }}",
                    n,
                    weekday.to_rust_code(),
                    month.to_rust_code()
                );
                wrap_effective(inner, *from_year, *to_year)
            }
            RuleDef::WeekdayShift {
                weekday,
                month,
                day,
                dir,
            } => {
                format!(
                    "Rule::WeekdayShift {{ weekday: {}, month: {}, day: {}, dir: {} }}",
                    weekday.to_rust_code(),
                    month.to_rust_code(),
                    day,
                    dir.to_rust_code()
                )
            }
            RuleDef::Span {
                start,
                len,
                offset,
                from_year,
                to_year,
            } => {
                let inner = format!(
                    "Rule::Span {{ start: &({}), len: {}, offset: {} }}",
                    start.to_rust_code(),
                    len,
                    offset
                );
                wrap_effective(inner, *from_year, *to_year)
            }
            RuleDef::ChineseNewYear => "Rule::ChineseNewYear".to_string(),
            RuleDef::QingMing => "Rule::QingMing".to_string(),
            RuleDef::BuddhasBirthday => "Rule::BuddhasBirthday".to_string(),
            RuleDef::DragonBoat => "Rule::DragonBoat".to_string(),
            RuleDef::MidAutumn => "Rule::MidAutumn".to_string(),
            RuleDef::ChinaBridge {
                festival,
                from_year,
                to_year,
            } => {
                let inner = format!(
                    "Rule::ChinaBridge {{ festival: &({}) }}",
                    festival.to_rust_code()
                );
                wrap_effective(inner, *from_year, *to_year)
            }
            RuleDef::VernalEquinoxJp => "Rule::VernalEquinoxJP".to_string(),
            RuleDef::AutumnalEquinoxJp => "Rule::AutumnalEquinoxJP".to_string(),
        }
    }
}

pub(crate) fn generate() -> io::Result<()> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let calendar_dir = Path::new(&manifest_dir).join("data").join("calendars");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join("calendars.rs");

    // Collect all calendar definitions
    let mut calendars = BTreeMap::new();

    for entry in fs::read_dir(calendar_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let json_str = fs::read_to_string(&path)?;
            let cal: CalendarDef = serde_json::from_str(&json_str).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to parse {}: {}", path.display(), e),
                )
            })?;
            calendars.insert(cal.id.to_owned(), cal);
        }
    }

    let mut output = String::new();

    // Header
    output.push_str("// Auto-generated from JSON calendar definitions - DO NOT EDIT\n\n");
    output.push_str("use time::{Month, Weekday};\n");
    output.push_str("use crate::dates::calendar::rule::{Rule, Observed, Direction};\n");
    output.push_str("use crate::dates::calendar::types::Calendar;\n");
    output.push_str("use crate::dates::calendar::business_days::HolidayCalendar;\n\n");

    // Generate constants for each calendar
    let mut calendar_names = Vec::new();
    for (id, cal) in &calendars {
        let const_name = id.to_uppercase();
        calendar_names.push((id.clone(), const_name.clone()));

        // Generate rules array
        output.push_str(&format!("static {}_RULES: &[Rule] = &[\n", const_name));
        for rule in &cal.rules {
            output.push_str("    ");
            output.push_str(&rule.to_rust_code());
            output.push_str(",\n");
        }
        output.push_str("];\n\n");

        // Generate calendar constant
        output.push_str(&format!(
            "/// {}\npub static {}: Calendar = Calendar::new(\n    \"{}\",\n    \"{}\",\n    {},\n    {}_RULES,\n);\n\n",
            cal.name,
            const_name,
            id,
            cal.name,
            cal.ignore_weekends.unwrap_or(false),
            const_name
        ));
    }

    // Generate ALL_IDS array
    output.push_str("/// All available calendar identifiers.\npub static ALL_IDS: &[&str] = &[\n");
    for (id, _) in &calendar_names {
        output.push_str(&format!("    \"{}\",\n", id));
    }
    output.push_str("];\n\n");

    // Generate calendar_by_id function. The inner `lookup` matches against the
    // canonical lowercase ids; the outer function tries the input verbatim first
    // (the common case — callers pass canonical ids) to avoid allocating, then
    // falls back to an ASCII-lowercased copy in a stack buffer for mixed-case
    // input. All built-in ids are short ASCII, so the 32-byte buffer never
    // truncates a known id.
    output.push_str("/// Resolve a calendar by its identifier (ASCII case-insensitive).\n///\n/// # Arguments\n///\n/// * `id` - Built-in calendar identifier. Matching is ASCII case-insensitive;\n///   unknown or non-ASCII identifiers return `None`.\npub fn calendar_by_id(id: &str) -> Option<&'static dyn HolidayCalendar> {\n");
    output.push_str("    fn lookup(id: &str) -> Option<&'static dyn HolidayCalendar> {\n");
    output.push_str("        match id {\n");
    for (id, const_name) in &calendar_names {
        output.push_str(&format!(
            "            \"{}\" => Some(&{}),\n",
            id, const_name
        ));
    }
    output.push_str("            _ => None,\n");
    output.push_str("        }\n");
    output.push_str("    }\n");
    output.push_str("    if let Some(c) = lookup(id) {\n");
    output.push_str("        return Some(c);\n");
    output.push_str("    }\n");
    output.push_str("    let bytes = id.as_bytes();\n");
    output.push_str("    if id.is_ascii() && bytes.len() <= 32 {\n");
    output.push_str("        let mut buf = [0u8; 32];\n");
    output.push_str("        for (b, s) in buf.iter_mut().zip(bytes) {\n");
    output.push_str("            *b = s.to_ascii_lowercase();\n");
    output.push_str("        }\n");
    output.push_str("        if let Ok(lower) = core::str::from_utf8(&buf[..bytes.len()]) {\n");
    output.push_str("            return lookup(lower);\n");
    output.push_str("        }\n");
    output.push_str("    }\n");
    output.push_str("    None\n");
    output.push_str("}\n\n");

    fs::write(out_path, output)?;
    Ok(())
}
