//! Checked schedule construction across frequencies, genuinely irregular stubs,
//! tenor, EOM and IMM rules, plus clone-and-visit/collect workflows.
//! Each selected case creates and checks its own untimed fixture. Construction
//! cases time the complete builder and consume every emitted schedule field.

use criterion::{Bencher, BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::dates::{
    BusinessDayConvention, HolidayCalendar, Schedule, StubKind, Tenor, calendar_by_id_strict,
};
use std::hint::black_box;
use time::{Date, Duration, Month};

fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("month"), day)
        .expect("valid benchmark date")
}

fn start_date() -> Date {
    date(2025, 1, 15)
}

fn end_date_years(years: u32) -> Date {
    date(2025 + years as i32, 1, 15)
}

// Independent fixture grid: construct each calendar month explicitly instead
// of calling the schedule generator or its tenor/roll helpers.
fn month_date(offset: u32, day: u8) -> Date {
    let year = 2025 + (offset / 12) as i32;
    let month = (offset % 12 + 1) as u8;
    let next = if month == 12 {
        date(year + 1, 1, 1)
    } else {
        date(year, month + 1, 1)
    };
    date(year, month, day.min((next - Duration::days(1)).day()))
}

fn regular_dates(years: u32, months: usize, day: u8) -> Vec<Date> {
    (0..=12 * years)
        .step_by(months)
        .map(|offset| month_date(offset, day))
        .collect()
}

fn check_schedule(schedule: &Schedule, dates: &[Date], payments: &[Date]) {
    assert_eq!(schedule.get_dates(), dates);
    assert_eq!(schedule.get_payment_dates(), payments);
    assert_eq!(
        schedule.get_payment_dates().len(),
        schedule.get_dates().len() - 1
    );
    assert!(
        schedule
            .get_dates()
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert!(schedule.get_fixing_dates().is_empty());
}

fn bench_build(
    b: &mut Bencher<'_>,
    build: impl Fn() -> Schedule,
    dates: &[Date],
    payments: &[Date],
) {
    check_schedule(&build(), dates, payments);
    b.iter(|| black_box(build()));
}

fn bench_frequency_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_frequency");
    for (name, frequency, months) in [
        ("monthly", Tenor::monthly(), 1),
        ("quarterly", Tenor::quarterly(), 3),
        ("semi_annual", Tenor::semi_annual(), 6),
        ("annual", Tenor::annual(), 12),
    ] {
        group.bench_function(name, |b| {
            let start = start_date();
            let end = end_date_years(10);
            let expected = regular_dates(10, months, 15);
            bench_build(
                b,
                || {
                    Schedule::builder(start, end)
                        .expect("date range")
                        .frequency(frequency)
                        .build()
                        .expect("valid frequency schedule")
                },
                &expected,
                &expected[1..],
            );
        });
    }
    group.finish();
}

fn bench_stub_conventions(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_stub_kind");
    for (name, stub) in [
        ("aligned_none", StubKind::None),
        ("irregular_short_front", StubKind::ShortFront),
        ("irregular_short_back", StubKind::ShortBack),
        ("irregular_long_front", StubKind::LongFront),
        ("irregular_long_back", StubKind::LongBack),
    ] {
        group.bench_function(name, |b| {
            let start = start_date();
            let end = if stub == StubKind::None {
                end_date_years(10)
            } else {
                date(2035, 2, 28)
            };
            let expected = match stub {
                StubKind::None => regular_dates(10, 3, 15),
                StubKind::ShortBack | StubKind::LongBack => {
                    let mut dates = regular_dates(10, 3, 15);
                    if stub == StubKind::LongBack {
                        dates.pop();
                    }
                    dates.push(end);
                    dates
                }
                _ => {
                    assert!(matches!(stub, StubKind::ShortFront | StubKind::LongFront));
                    let first = if stub == StubKind::LongFront { 4 } else { 1 };
                    std::iter::once(start)
                        .chain(
                            (first..=121)
                                .step_by(3)
                                .map(|offset| month_date(offset, 28)),
                        )
                        .collect()
                }
            };
            match stub {
                StubKind::ShortFront => assert!((expected[1] - expected[0]).whole_days() < 60),
                StubKind::LongFront => assert!((expected[1] - expected[0]).whole_days() > 100),
                StubKind::ShortBack => {
                    assert!((end - expected[expected.len() - 2]).whole_days() < 60)
                }
                StubKind::LongBack => {
                    assert!((end - expected[expected.len() - 2]).whole_days() > 100)
                }
                _ => {
                    assert_eq!(stub, StubKind::None);
                    assert_eq!(expected.len(), 41);
                }
            }
            bench_build(
                b,
                || {
                    Schedule::builder(start, end)
                        .expect("date range")
                        .frequency(Tenor::quarterly())
                        .stub_rule(stub)
                        .build()
                        .expect("valid stub schedule")
                },
                &expected,
                &expected[1..],
            );
        });
    }
    group.finish();
}

fn bench_tenor_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_tenor_scaling");
    for (name, frequency, months) in [
        ("quarterly", Tenor::quarterly(), 3),
        ("monthly", Tenor::monthly(), 1),
    ] {
        for years in [1, 2, 5, 10, 20, 30] {
            group.bench_with_input(
                BenchmarkId::new(name, format!("{years}y")),
                &years,
                |b, &years| {
                    let start = start_date();
                    let end = end_date_years(years);
                    let expected = regular_dates(years, months, 15);
                    bench_build(
                        b,
                        || {
                            Schedule::builder(start, end)
                                .expect("date range")
                                .frequency(frequency)
                                .build()
                                .expect("valid tenor schedule")
                        },
                        &expected,
                        &expected[1..],
                    );
                },
            );
        }
    }
    group.finish();
}

fn bench_end_of_month(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_eom");
    for (name, eom) in [("eom_false", false), ("eom_true", true)] {
        group.bench_function(name, |b| {
            // February month-end distinguishes preserving day 28 from snapping
            // later months to their last day; a January-31 anchor did not.
            let start = date(2025, 2, 28);
            let end = date(2035, 2, 28);
            let expected: Vec<_> = (1..=121)
                .map(|offset| month_date(offset, if eom { 31 } else { 28 }))
                .collect();
            assert_eq!(expected[1], date(2025, 3, if eom { 31 } else { 28 }));
            bench_build(
                b,
                || {
                    Schedule::builder(start, end)
                        .expect("date range")
                        .frequency(Tenor::monthly())
                        .end_of_month(eom)
                        .build()
                        .expect("valid EOM schedule")
                },
                &expected,
                &expected[1..],
            );
        });
    }
    group.finish();
}

fn imm_dates(years: u32, cds: bool) -> Vec<Date> {
    let mut dates = vec![if cds {
        date(2024, 12, 20)
    } else {
        start_date()
    }];
    for year in 2025..2025 + years as i32 {
        for month in [3, 6, 9, 12] {
            let day = if cds {
                20
            } else {
                let weekday = date(year, month, 15).weekday().number_days_from_monday();
                15 + (9 - weekday) % 7
            };
            dates.push(date(year, month, day));
        }
    }
    dates.push(end_date_years(years));
    dates
}

fn bench_imm_schedules(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_imm");
    for years in [1, 5, 10] {
        for (name, cds) in [("cds_imm", true), ("standard_imm", false)] {
            group.bench_with_input(
                BenchmarkId::new(name, format!("{years}y")),
                &years,
                |b, &years| {
                    let start = start_date();
                    let end = end_date_years(years);
                    let expected = imm_dates(years, cds);
                    bench_build(
                        b,
                        || {
                            let builder = Schedule::builder(start, end).expect("date range");
                            let builder = if cds {
                                builder.cds_imm()
                            } else {
                                builder.imm()
                            };
                            builder.build().expect("valid IMM schedule")
                        },
                        &expected,
                        &expected[1..],
                    );
                },
            );
        }
    }
    group.finish();
}

// Independent date scan for the benchmark's payment-date expectation.
fn following_payment(day: Date, cal: &dyn HolidayCalendar, modified: bool) -> Date {
    let mut next = day;
    while !cal.is_business_day(next).expect("calendar lookup") {
        next = next.next_day().expect("next date");
    }
    if modified && next.month() != day.month() {
        let mut previous = day;
        while !cal.is_business_day(previous).expect("calendar lookup") {
            previous = previous.previous_day().expect("previous date");
        }
        previous
    } else {
        next
    }
}

fn bench_business_day_adjustment(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_bday_adjust");
    for (name, convention) in [
        ("unadjusted", None),
        (
            "mod_following_nyse",
            Some(BusinessDayConvention::ModifiedFollowing),
        ),
        ("following_nyse", Some(BusinessDayConvention::Following)),
    ] {
        group.bench_function(name, |b| {
            let start = start_date();
            let end = end_date_years(10);
            let expected = regular_dates(10, 3, 15);
            let payments = match convention {
                None => expected[1..].to_vec(),
                Some(convention) => {
                    let cal = calendar_by_id_strict("nyse").expect("NYSE calendar");
                    expected[1..]
                        .iter()
                        .map(|&day| {
                            following_payment(
                                day,
                                cal,
                                convention == BusinessDayConvention::ModifiedFollowing,
                            )
                        })
                        .collect()
                }
            };
            if convention.is_some() {
                assert_ne!(payments, expected[1..]);
            }
            bench_build(
                b,
                || {
                    let builder = Schedule::builder(start, end)
                        .expect("date range")
                        .frequency(Tenor::quarterly());
                    let builder = match convention {
                        Some(convention) => builder.adjust_with(convention, "nyse"),
                        None => builder,
                    };
                    builder.build().expect("valid adjusted schedule")
                },
                &expected,
                &payments,
            );
        });
    }
    group.finish();
}

fn visit_dates(schedule: Schedule) -> (usize, i64) {
    schedule.into_iter().fold((0, 0), |(count, checksum), day| {
        (
            count + 1,
            checksum + i64::from(black_box(day).to_julian_day()),
        )
    })
}

fn bench_schedule_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_iteration");
    for (name, collect) in [
        ("clone_and_visit_30y_monthly", false),
        ("clone_and_collect_30y_monthly", true),
    ] {
        group.bench_function(name, |b| {
            let expected = regular_dates(30, 1, 15);
            let schedule = Schedule::builder(start_date(), end_date_years(30))
                .expect("date range")
                .frequency(Tenor::monthly())
                .build()
                .expect("valid iteration fixture");
            check_schedule(&schedule, &expected, &expected[1..]);
            if collect {
                let collected: Vec<_> = schedule.clone().into_iter().collect();
                assert_eq!(collected, expected);
                b.iter(|| {
                    let dates: Vec<_> = black_box(schedule.clone()).into_iter().collect();
                    black_box(dates)
                });
            } else {
                let checksum: i64 = expected
                    .iter()
                    .map(|day| i64::from(day.to_julian_day()))
                    .sum();
                assert_eq!(visit_dates(schedule.clone()), (361, checksum));
                b.iter(|| black_box(visit_dates(black_box(schedule.clone()))));
            }
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_frequency_variants,
    bench_stub_conventions,
    bench_tenor_scaling,
    bench_end_of_month,
    bench_imm_schedules,
    bench_business_day_adjustment,
    bench_schedule_iteration,
);
criterion_main!(benches);
