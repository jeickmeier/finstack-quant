use finstack_quant_core::dates::{create_date, Date, Duration, Month};

#[test]
fn dates_facade_reexports_time_primitives_needed_by_downstream_crates() {
    let start: Date = create_date(2025, Month::January, 1).expect("valid date");
    let next = start + Duration::days(1);

    assert_eq!(
        next,
        create_date(2025, Month::January, 2).expect("valid date")
    );
}
