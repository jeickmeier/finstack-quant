//! Discount factor metric tests (DfStart, DfEnd, DfEndFromQuote).

use crate::deposit::common::*;
use finstack_quant_valuations::metrics::MetricId;

#[test]
fn test_df_start_matches_curve() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).build();

    // Execute
    let df_start_metric = compute_metric(&dep, &ctx, base, MetricId::DfStart);
    let disc = ctx.get_discount("USD-OIS").unwrap();
    let df_start_curve = disc
        .df_on_date_curve(dep.effective_start_date().unwrap())
        .expect("try_df_on_date_curve should succeed");

    // Validate
    assert!((df_start_metric - df_start_curve).abs() < DF_TOLERANCE);
}

#[test]
fn test_df_end_matches_curve() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).maturity(date(2025, 7, 1)).build();

    // Execute
    let df_end_metric = compute_metric(&dep, &ctx, base, MetricId::DfEnd);
    let disc = ctx.get_discount("USD-OIS").unwrap();
    let df_end_curve = disc
        .df_on_date_curve(dep.effective_end_date().unwrap())
        .expect("try_df_on_date_curve should succeed");

    // Validate
    assert!((df_end_metric - df_end_curve).abs() < DF_TOLERANCE);
}

#[test]
fn test_df_end_less_than_df_start() {
    // Setup - for positive rates, DF should decrease over time
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base)
        .start_date(date(2025, 1, 1))
        .maturity(date(2025, 7, 1))
        .build();

    // Execute
    let metrics = compute_metrics(&dep, &ctx, base, &[MetricId::DfStart, MetricId::DfEnd]);

    // Validate
    assert!(metrics[&MetricId::DfEnd] < metrics[&MetricId::DfStart]);
}

#[test]
fn test_df_at_base_date_is_one() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).start_date(base).build();

    // Execute
    let df_start = compute_metric(&dep, &ctx, base, MetricId::DfStart);

    // Validate - DF at base date should be 1.0
    assert!((df_start - 1.0).abs() < DF_TOLERANCE);
}

#[test]
fn test_df_end_from_quote_consistency() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let metrics = compute_metrics(
        &dep,
        &ctx,
        base,
        &[MetricId::DfStart, MetricId::Yf, MetricId::DfEndFromQuote],
    );

    let df_s = metrics[&MetricId::DfStart];
    let yf = metrics[&MetricId::Yf];
    let df_e_from_quote = metrics[&MetricId::DfEndFromQuote];

    // Validate - DF(end) = DF(start) / (1 + rate × yf)
    let expected = df_s / (1.0 + 0.03 * yf);
    assert!((df_e_from_quote - expected).abs() < DF_TOLERANCE);
}

#[test]
fn test_df_end_from_quote_with_zero_rate() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.0)
        .build();

    // Execute
    let metrics = compute_metrics(
        &dep,
        &ctx,
        base,
        &[MetricId::DfStart, MetricId::DfEndFromQuote],
    );

    // Validate - with zero rate, DFs should be equal
    assert!(
        (metrics[&MetricId::DfStart] - metrics[&MetricId::DfEndFromQuote]).abs() < DF_TOLERANCE
    );
}

#[test]
fn test_df_end_from_quote_with_high_rate() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.10) // 10% rate
        .build();

    // Execute
    let metrics = compute_metrics(
        &dep,
        &ctx,
        base,
        &[MetricId::DfStart, MetricId::Yf, MetricId::DfEndFromQuote],
    );

    let df_s = metrics[&MetricId::DfStart];
    let yf = metrics[&MetricId::Yf];
    let df_e = metrics[&MetricId::DfEndFromQuote];

    // Validate - formula holds
    assert!((df_e - df_s / (1.0 + 0.10 * yf)).abs() < DF_TOLERANCE);
}

#[test]
fn test_longer_maturity_gives_lower_df() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_short = DepositBuilder::new(base).maturity(date(2025, 4, 1)).build();

    let dep_long = DepositBuilder::new(base).maturity(date(2026, 1, 1)).build();

    // Execute
    let df_short = compute_metric(&dep_short, &ctx, base, MetricId::DfEnd);
    let df_long = compute_metric(&dep_long, &ctx, base, MetricId::DfEnd);

    // Validate - longer maturity should have lower DF
    assert!(df_long < df_short);
}

#[test]
fn test_df_with_steep_curve() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_steep_curve(base, "USD-OIS");
    let dep = DepositBuilder::new(base).maturity(date(2026, 1, 1)).build();

    // Execute
    let df_end = compute_metric(&dep, &ctx, base, MetricId::DfEnd);

    // Validate - steep curve should give low DF
    assert!(df_end < 0.95);
}
