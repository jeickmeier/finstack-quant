//! Jurisdiction holiday sets whose substitution rules depend on other holidays.
//!
//! Sources: Hong Kong General Holidays Ordinance (Cap. 149), the 1982 LegCo
//! holiday resolution, Holidays (1997 and 1998) Ordinance and the 1998/2011
//! amendments; Japan's Act on National Holidays and Cabinet Office holiday list.
//! - <https://www.legco.gov.hk/yr81-82/english/lc_sitg/hansard/h820210.pdf>
//! - <https://www.legco.gov.hk/yr98-99/english/bills/c044_e.htm>
//! - <https://www.labour.gov.hk/eng/news/Substitution_of_Holidays.htm>
//! - <https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html>

use super::algo;
use smallvec::{Array, SmallVec};
use time::{Date, Duration, Month, Weekday};

type Dates = SmallVec<[Date; 32]>;

fn fixed(year: i32, month: Month, day: u8, dates: &mut Dates) {
    if let Ok(date) = Date::from_calendar_date(year, month, day) {
        dates.push(date);
    }
}

fn nth_monday(year: i32, month: Month, n: i8, dates: &mut Dates) {
    dates.extend(algo::nth_weekday_of_month(year, month, Weekday::Monday, n));
}

fn sunday_substitute(date: Date) -> Date {
    if date.weekday() == Weekday::Sunday {
        date + Duration::days(1)
    } else {
        date
    }
}

/// Materialize Hong Kong general holidays excluding ordinary Sundays.
pub(super) fn hong_kong<A: Array<Item = Date>>(year: i32, out: &mut SmallVec<A>) {
    let mut dates = Dates::new();
    for (month, day) in [
        (Month::January, 1),
        (Month::December, 25),
        (Month::December, 26),
    ] {
        if let Ok(date) = Date::from_calendar_date(year, month, day) {
            dates.push(sunday_substitute(date));
        }
    }
    if let Some(cny) = algo::cny_date(year) {
        for offset in 0..3 {
            let date = cny + Duration::days(offset);
            dates.push(if date.weekday() == Weekday::Sunday {
                // The 2011 amendment took effect after Lunar New Year 2012.
                cny + Duration::days(if (1983..=2012).contains(&year) { -1 } else { 3 })
            } else {
                date
            });
        }
    }
    for date in [
        algo::qing_ming_date(year),
        algo::dragon_boat_date(year),
        algo::chung_yeung_date(year),
    ]
    .into_iter()
    .flatten()
    {
        dates.push(sunday_substitute(date));
    }
    let easter = algo::easter_monday(year);
    dates.extend([
        easter - Duration::days(3),
        easter - Duration::days(2),
        easter,
    ]);
    if let Some(autumn) = algo::mid_autumn_date(year) {
        let after = autumn + Duration::days(1);
        dates.push(
            if after.weekday() == Weekday::Sunday && (1983..=2011).contains(&year) {
                autumn
            } else {
                sunday_substitute(after)
            },
        );
    }
    if year >= 1999 {
        fixed(year, Month::May, 1, &mut dates);
        dates.extend(algo::buddhas_birthday_date(year));
    }
    if year >= 1997 {
        fixed(year, Month::July, 1, &mut dates);
        fixed(year, Month::October, 1, &mut dates);
    }
    if year <= 1982 {
        fixed(year, Month::April, 21, &mut dates);
        if let Ok(july) = Date::from_calendar_date(year, Month::July, 1) {
            dates.push(sunday_substitute(july));
        }
        nth_monday(year, Month::August, 1, &mut dates);
    } else if year <= 1997 {
        // LegCo 1983-11-09 p.202 announced 1984 June 16/18, one week later.
        let week = if year == 1984 { 3 } else { 2 };
        if let Some(birthday) =
            algo::nth_weekday_of_month(year, Month::June, Weekday::Saturday, week)
        {
            dates.extend([birthday, birthday + Duration::days(2)]);
        }
    }
    if year <= 1996 {
        if let Some(liberation) =
            algo::nth_weekday_of_month(year, Month::August, Weekday::Monday, -1)
        {
            dates.push(liberation);
            if year >= 1983 {
                dates.push(liberation - Duration::days(2));
            }
        }
    }
    if (1997..=1998).contains(&year) {
        fixed(year, Month::October, 2, &mut dates);
        nth_monday(year, Month::August, 3, &mut dates);
    }
    if year == 1997 {
        fixed(year, Month::July, 2, &mut dates);
    }
    // One-off general holidays recorded in the 2015 LegCo holiday debate.
    if year == 1981 {
        fixed(year, Month::July, 29, &mut dates);
    }
    if year == 1986 {
        fixed(year, Month::October, 22, &mut dates);
    }
    if year == 2015 {
        fixed(year, Month::September, 3, &mut dates);
    }

    // Reserve every ordinary holiday before resolving collisions, so a
    // substitute cannot consume a later holiday. Saturday is a weekday in
    // Cap.149; only Sunday is skipped by the general holiday substitution.
    for date in &mut dates {
        *date = sunday_substitute(*date);
    }
    dates.sort_unstable();
    let mut unique = dates.clone();
    unique.dedup();
    for pair in dates.windows(2) {
        if pair[0] == pair[1] {
            let mut extra = pair[1] + Duration::days(1);
            while extra.weekday() == Weekday::Sunday || unique.contains(&extra) {
                extra += Duration::days(1);
            }
            unique.push(extra);
        }
    }
    out.extend(unique);
}

/// Materialize Japan's national, citizens' and substitute holidays.
pub(super) fn japan<A: Array<Item = Date>>(year: i32, out: &mut SmallVec<A>) {
    let mut national = Dates::new();
    for (month, day) in [
        (Month::January, 1),
        (Month::February, 11),
        (Month::April, 29),
        (Month::May, 3),
        (Month::May, 5),
        (Month::November, 3),
        (Month::November, 23),
    ] {
        fixed(year, month, day, &mut national);
    }
    national.extend(algo::vernal_equinox_jp_date(year));
    national.extend(algo::autumnal_equinox_jp_date(year));
    if year < 2000 {
        fixed(year, Month::January, 15, &mut national);
    } else {
        nth_monday(year, Month::January, 2, &mut national);
    }
    if year < 2003 {
        fixed(year, Month::September, 15, &mut national);
    } else {
        nth_monday(year, Month::September, 3, &mut national);
    }
    match year {
        2020 => {
            fixed(year, Month::July, 23, &mut national);
            fixed(year, Month::July, 24, &mut national);
            fixed(year, Month::August, 10, &mut national);
        }
        2021 => {
            fixed(year, Month::July, 22, &mut national);
            fixed(year, Month::July, 23, &mut national);
            fixed(year, Month::August, 8, &mut national);
        }
        _ => {
            if year >= 1996 {
                if year < 2003 {
                    fixed(year, Month::July, 20, &mut national);
                } else {
                    nth_monday(year, Month::July, 3, &mut national);
                }
            }
            if year < 2000 {
                fixed(year, Month::October, 10, &mut national);
            } else {
                nth_monday(year, Month::October, 2, &mut national);
            }
            if year >= 2016 {
                fixed(year, Month::August, 11, &mut national);
            }
        }
    }
    if year >= 2007 {
        fixed(year, Month::May, 4, &mut national);
    }
    if (1989..=2018).contains(&year) {
        fixed(year, Month::December, 23, &mut national);
    } else if year >= 2020 {
        fixed(year, Month::February, 23, &mut national);
    }
    match year {
        1989 => fixed(year, Month::February, 24, &mut national),
        1990 => fixed(year, Month::November, 12, &mut national),
        1993 => fixed(year, Month::June, 9, &mut national),
        2019 => {
            fixed(year, Month::May, 1, &mut national);
            fixed(year, Month::October, 22, &mut national);
        }
        _ => {}
    }
    let mut holidays = national.clone();
    if year >= 1986 {
        for &date in &national {
            let between = date + Duration::days(1);
            if between.weekday() != Weekday::Sunday
                && !national.contains(&between)
                && national.contains(&(date + Duration::days(2)))
            {
                holidays.push(between);
            }
        }
    }
    for &date in &national {
        // Act No.10 of 1973 entered into force on April 12. The 2007
        // amendment extends substitution past intervening national holidays.
        if date.weekday() == Weekday::Sunday && (year, date.ordinal()) >= (1973, 102) {
            let mut substitute = date + Duration::days(1);
            if year >= 2007 {
                while national.contains(&substitute) {
                    substitute += Duration::days(1);
                }
            }
            holidays.push(substitute);
        }
    }
    holidays.sort_unstable();
    holidays.dedup();
    out.extend(holidays);
}
