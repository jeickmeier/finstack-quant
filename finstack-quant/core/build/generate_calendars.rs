//! Build-time generator for exchange and holiday calendar implementations.
//!
/// Generate calendar implementations from JSON definitions.
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One maintained calendar definition. Unknown fields are rejected and every
/// financial control is explicit (CORE-CONVENTIONS-004 item 10).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CalendarDef {
    id: String,
    name: String,
    ignore_weekends: bool,
    #[serde(default)]
    supported_years: Option<SupportedYearsDef>,
    notes: Option<String>,
    comment: Option<String>,
    /// Pinned primary sources for the dated data (record CORE-CONVENTIONS-005).
    #[serde(default)]
    sources: Vec<SourceDef>,
    rules: Vec<RuleDef>,
    /// Dated exceptions to the Saturday/Sunday closure rule.
    #[serde(default)]
    open_dates: Vec<OpenDateDef>,
}

/// A documented exceptional trading date, emitted as a checked date constant.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenDateDef {
    year: i32,
    month: u8,
    day: u8,
    notice: String,
}

/// Optional inclusive `(from_year, to_year)` calendar coverage bounds.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct SupportedYearsDef {
    #[serde(default)]
    from_year: Option<i32>,
    #[serde(default)]
    to_year: Option<i32>,
}

/// One pinned primary source: where it is, which edition, when it was
/// retrieved and the SHA-256 of the retrieved bytes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDef {
    name: String,
    url: String,
    edition: String,
    retrieved: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum RuleDef {
    ExactDate {
        #[serde(default)]
        name: Option<String>,
        year: i32,
        month: MonthName,
        day: u8,
    },
    Fixed {
        #[serde(default)]
        name: Option<String>,
        month: MonthName,
        day: u8,
        observed: ObservedName,
        #[serde(default)]
        from_year: Option<i32>,
        #[serde(default)]
        to_year: Option<i32>,
    },
    EasterOffset {
        #[serde(default)]
        name: Option<String>,
        days: i16,
        #[serde(default)]
        from_year: Option<i32>,
        #[serde(default)]
        to_year: Option<i32>,
    },
    NthWeekday {
        #[serde(default)]
        name: Option<String>,
        n: i8,
        weekday: WeekdayName,
        month: MonthName,
        #[serde(default)]
        from_year: Option<i32>,
        #[serde(default)]
        to_year: Option<i32>,
    },
    WeekdayShift {
        #[serde(default)]
        name: Option<String>,
        weekday: WeekdayName,
        month: MonthName,
        day: u8,
        dir: DirectionName,
    },
    Span {
        #[serde(default)]
        name: Option<String>,
        start: Box<RuleDef>,
        len: u8,
        offset: i16,
        #[serde(default)]
        from_year: Option<i32>,
        #[serde(default)]
        to_year: Option<i32>,
    },
    ChineseNewYear {},
    QingMing {},
    DragonBoat {},
    MidAutumn {},
    ChinaBridge {
        #[serde(default)]
        name: Option<String>,
        festival: Box<RuleDef>,
        #[serde(default)]
        from_year: Option<i32>,
        #[serde(default)]
        to_year: Option<i32>,
    },
    HongKongPublicHolidays {
        #[serde(default)]
        name: Option<String>,
    },
    JapanPublicHolidays {
        #[serde(default)]
        name: Option<String>,
    },
}

#[derive(Debug, Clone, Copy)]
enum ObservedName {
    None,
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
            "none" => Ok(Self::None),
            "next_monday" => Ok(Self::NextMonday),
            "fri_if_sat_mon_if_sun" => Ok(Self::FriIfSatMonIfSun),
            "mon_if_sun" => Ok(Self::MonIfSun),
            "mon_if_sat_tue_if_sun" => Ok(Self::MonIfSatTueIfSun),
            _ => Err(serde::de::Error::custom(format!("Unknown observed: {s}"))),
        }
    }
}

impl MonthName {
    fn max_days(self, leap_year: bool) -> u8 {
        match self {
            Self::January
            | Self::March
            | Self::May
            | Self::July
            | Self::August
            | Self::October
            | Self::December => 31,
            Self::April | Self::June | Self::September | Self::November => 30,
            Self::February => {
                if leap_year {
                    29
                } else {
                    28
                }
            }
        }
    }

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
            Self::None => "Observed::None",
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
    /// Human-readable rule name, emitted as a comment in the generated code.
    fn name(&self) -> Option<&str> {
        match self {
            RuleDef::ExactDate { name, .. }
            | RuleDef::Fixed { name, .. }
            | RuleDef::EasterOffset { name, .. }
            | RuleDef::NthWeekday { name, .. }
            | RuleDef::WeekdayShift { name, .. }
            | RuleDef::Span { name, .. }
            | RuleDef::ChinaBridge { name, .. }
            | RuleDef::HongKongPublicHolidays { name }
            | RuleDef::JapanPublicHolidays { name } => name.as_deref(),
            RuleDef::ChineseNewYear {}
            | RuleDef::QingMing {}
            | RuleDef::DragonBoat {}
            | RuleDef::MidAutumn {} => None,
        }
    }

    fn to_rust_code(&self) -> String {
        match self {
            RuleDef::ExactDate {
                year, month, day, ..
            } => format!(
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
                ..
            } => {
                let inner = format!(
                    "Rule::Fixed {{ month: {}, day: {}, observed: {} }}",
                    month.to_rust_code(),
                    day,
                    observed.to_rust_code()
                );
                wrap_effective(inner, *from_year, *to_year)
            }
            RuleDef::EasterOffset {
                days,
                from_year,
                to_year,
                ..
            } => wrap_effective(
                format!("Rule::EasterOffset({})", days),
                *from_year,
                *to_year,
            ),
            RuleDef::NthWeekday {
                n,
                weekday,
                month,
                from_year,
                to_year,
                ..
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
                ..
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
                ..
            } => {
                let inner = format!(
                    "Rule::Span {{ start: &({}), len: {}, offset: {} }}",
                    start.to_rust_code(),
                    len,
                    offset
                );
                wrap_effective(inner, *from_year, *to_year)
            }
            RuleDef::ChineseNewYear {} => "Rule::ChineseNewYear".to_string(),
            RuleDef::QingMing {} => "Rule::QingMing".to_string(),
            RuleDef::DragonBoat {} => "Rule::DragonBoat".to_string(),
            RuleDef::MidAutumn {} => "Rule::MidAutumn".to_string(),
            RuleDef::ChinaBridge {
                festival,
                from_year,
                to_year,
                ..
            } => {
                let inner = format!(
                    "Rule::ChinaBridge {{ festival: &({}) }}",
                    festival.to_rust_code()
                );
                wrap_effective(inner, *from_year, *to_year)
            }
            RuleDef::HongKongPublicHolidays { .. } => "Rule::HongKongPublicHolidays".to_string(),
            RuleDef::JapanPublicHolidays { .. } => "Rule::JapanPublicHolidays".to_string(),
        }
    }
}

fn is_gregorian_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn validate_year_bounds(
    from_year: Option<i32>,
    to_year: Option<i32>,
    context: &str,
) -> io::Result<()> {
    if let (Some(from), Some(to)) = (from_year, to_year)
        && from > to
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{context}: inverted year bounds from_year={from} > to_year={to}"),
        ));
    }
    Ok(())
}

fn validate_rule(rule: &RuleDef, context: &str) -> io::Result<()> {
    match rule {
        RuleDef::ExactDate {
            year, month, day, ..
        } => {
            let max_day = month.max_days(is_gregorian_leap_year(*year));
            if !(1..=max_day).contains(day) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{context}: invalid exact_date {year}-{month:?}-{day}"),
                ));
            }
        }
        RuleDef::Fixed {
            month,
            day,
            from_year,
            to_year,
            ..
        } => {
            let max_day = month.max_days(true);
            if !(1..=max_day).contains(day) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{context}: impossible fixed month/day {month:?}-{day}"),
                ));
            }
            validate_year_bounds(*from_year, *to_year, context)?;
        }
        RuleDef::EasterOffset {
            from_year, to_year, ..
        } => {
            validate_year_bounds(*from_year, *to_year, context)?;
        }
        RuleDef::NthWeekday {
            n,
            from_year,
            to_year,
            ..
        } => {
            if !((-5..=-1).contains(n) || (1..=5).contains(n)) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{context}: invalid nth_weekday ordinal {n} (expected -5..=-1 or 1..=5)"
                    ),
                ));
            }
            validate_year_bounds(*from_year, *to_year, context)?;
        }
        RuleDef::WeekdayShift { month, day, .. } => {
            let max_day = month.max_days(true);
            if !(1..=max_day).contains(day) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{context}: impossible weekday_shift month/day {month:?}-{day}"),
                ));
            }
        }
        RuleDef::Span {
            start,
            len,
            from_year,
            to_year,
            ..
        } => {
            if *len == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{context}: span rule len must be >= 1"),
                ));
            }
            validate_year_bounds(*from_year, *to_year, context)?;
            validate_rule(start, context)?;
        }
        RuleDef::ChinaBridge {
            festival,
            from_year,
            to_year,
            ..
        } => {
            validate_year_bounds(*from_year, *to_year, context)?;
            if matches!(
                festival.as_ref(),
                RuleDef::Span { .. }
                    | RuleDef::ChinaBridge { .. }
                    | RuleDef::HongKongPublicHolidays { .. }
                    | RuleDef::JapanPublicHolidays { .. }
            ) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{context}: china_bridge festival must be a single-occurrence rule"),
                ));
            }
            validate_rule(festival, context)?;
        }
        RuleDef::ChineseNewYear {}
        | RuleDef::QingMing {}
        | RuleDef::DragonBoat {}
        | RuleDef::MidAutumn {}
        | RuleDef::HongKongPublicHolidays { .. }
        | RuleDef::JapanPublicHolidays { .. } => {}
    }
    Ok(())
}

fn validate_calendar_def(cal: &CalendarDef, path: &Path) -> io::Result<()> {
    let context = format!("{} (calendar '{}')", path.display(), cal.id);
    if cal.name.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{context}: calendar name cannot be empty"),
        ));
    }
    if let Some(sy) = cal.supported_years {
        if sy.from_year.is_none() && sy.to_year.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{context}: supported_years must specify from_year and/or to_year"),
            ));
        }
        validate_year_bounds(
            sy.from_year,
            sy.to_year,
            &format!("{context}.supported_years"),
        )?;
    }
    for date in &cal.open_dates {
        if date.notice.trim().is_empty()
            || cal.supported_years.is_some_and(|years| {
                years.from_year.is_some_and(|year| date.year < year)
                    || years.to_year.is_some_and(|year| date.year > year)
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{context}: exceptional opening requires a notice and supported year"),
            ));
        }
    }
    for (idx, rule) in cal.rules.iter().enumerate() {
        validate_rule(rule, &format!("{context}.rules[{idx}]"))?;
    }
    Ok(())
}

pub(crate) fn generate() -> io::Result<()> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let calendar_dir = Path::new(&manifest_dir).join("data").join("calendars");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join("calendars.rs");

    // Collect all calendar definitions
    let mut calendars = BTreeMap::new();
    let mut calendar_sources: BTreeMap<String, PathBuf> = BTreeMap::new();

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
            let valid_id = !cal.id.is_empty()
                && cal.id.len() <= 32
                && cal
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
            if !valid_id {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{}: calendar id '{}' must be 1..=32 lowercase ASCII letters, digits or '_'",
                        path.display(),
                        cal.id
                    ),
                ));
            }
            validate_calendar_def(&cal, &path)?;
            if let Some(previous_path) = calendar_sources.get(&cal.id) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Duplicate calendar id '{}': {} and {}",
                        cal.id,
                        previous_path.display(),
                        path.display()
                    ),
                ));
            }
            calendar_sources.insert(cal.id.clone(), path.clone());
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
            if let Some(name) = rule.name() {
                output.push_str(&format!("    // {}\n", name.replace('\n', " ")));
            }
            output.push_str("    ");
            output.push_str(&rule.to_rust_code());
            output.push_str(",\n");
        }
        output.push_str("];\n\n");

        // Generate calendar constant
        output.push_str(&format!("/// {}\n", cal.name));
        for note in [cal.notes.as_deref(), cal.comment.as_deref()]
            .into_iter()
            .flatten()
        {
            output.push_str("///\n");
            for line in note.lines() {
                output.push_str(&format!("/// {line}\n"));
            }
        }
        for source in &cal.sources {
            output.push_str(&format!(
                "///\n/// Source: {} ({}), <{}>, retrieved {}, sha256 `{}`.\n",
                source.name, source.edition, source.url, source.retrieved, source.sha256
            ));
        }
        // Market calendars are reached by id (`calendar_by_id_strict`); only the
        // weekends-only fallback is a public static (`dates::WEEKENDS_ONLY`).
        let visibility = if id == "weekends_only" {
            "pub"
        } else {
            "pub(crate)"
        };
        let supported_years_code = match cal.supported_years {
            Some(sy) => format!("Some(({:?}, {:?}))", sy.from_year, sy.to_year),
            None => "None".to_string(),
        };
        let open_dates_code = cal
            .open_dates
            .iter()
            .map(|date| {
                format!(
                    "time::macros::date!({}-{:02}-{:02})",
                    date.year, date.month, date.day
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "{} static {}: Calendar = Calendar::new(\n    \"{}\",\n    \"{}\",\n    {},\n    {}_RULES,\n    {},\n    &[{}],\n);\n\n",
            visibility,
            const_name,
            id,
            cal.name,
            cal.ignore_weekends,
            const_name,
            supported_years_code,
            open_dates_code
        ));
    }

    // Test-only list of every built-in calendar, so rule tests reach the rules.
    output.push_str("#[cfg(test)]\npub(crate) static ALL_CALENDARS: &[&Calendar] = &[\n");
    for (_, const_name) in &calendar_names {
        output.push_str(&format!("    &{},\n", const_name));
    }
    output.push_str("];\n\n");

    output.push_str("pub(in crate::dates) mod registry {\n");
    output.push_str("    use super::*;\n\n");

    // Generate ALL_IDS array
    output.push_str(
        "    /// All available calendar identifiers.\n    pub static ALL_IDS: &[&str] = &[\n",
    );
    for (id, _) in &calendar_names {
        output.push_str(&format!("        \"{}\",\n", id));
    }
    output.push_str("    ];\n\n");

    // Generate calendar_by_id function. The inner `lookup` matches against the
    // canonical lowercase ids; the outer function tries the input verbatim first
    // (the common case — callers pass canonical ids) to avoid allocating, then
    // falls back to an ASCII-lowercased copy in a stack buffer for mixed-case
    // input. All built-in ids are short ASCII, so the 32-byte buffer never
    // truncates a known id.
    output.push_str("    /// Resolve a calendar by its identifier (ASCII case-insensitive).\n    ///\n    /// # Arguments\n    ///\n    /// * `id` - Built-in calendar identifier. Matching is ASCII case-insensitive;\n    ///   unknown or non-ASCII identifiers return `None`.\n    pub fn calendar_by_id(id: &str) -> Option<&'static dyn HolidayCalendar> {\n");
    output.push_str("        fn lookup(id: &str) -> Option<&'static dyn HolidayCalendar> {\n");
    output.push_str("            match id {\n");
    for (id, const_name) in &calendar_names {
        output.push_str(&format!(
            "                \"{}\" => Some(&{}),\n",
            id, const_name
        ));
    }
    output.push_str("                _ => None,\n");
    output.push_str("            }\n");
    output.push_str("        }\n");
    output.push_str("        if let Some(c) = lookup(id) {\n");
    output.push_str("            return Some(c);\n");
    output.push_str("        }\n");
    output.push_str("        let bytes = id.as_bytes();\n");
    output.push_str("        if id.is_ascii() && bytes.len() <= 32 {\n");
    output.push_str("            let mut buf = [0u8; 32];\n");
    output.push_str("            for (b, s) in buf.iter_mut().zip(bytes) {\n");
    output.push_str("                *b = s.to_ascii_lowercase();\n");
    output.push_str("            }\n");
    output.push_str("            if let Ok(lower) = core::str::from_utf8(&buf[..bytes.len()]) {\n");
    output.push_str("                return lookup(lower);\n");
    output.push_str("            }\n");
    output.push_str("        }\n");
    output.push_str("        None\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");

    fs::write(out_path, output)?;
    Ok(())
}
