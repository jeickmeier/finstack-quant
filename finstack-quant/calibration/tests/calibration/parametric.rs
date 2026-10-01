//! Parametric calibration residual normalization and explicit fit acceptance.

use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::market_datum::MarketDatum;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, ParametricCurveParams, StepParams,
};
use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_calibration::quotes::rates::RateQuote;
use finstack_quant_calibration::CalibrationConfig;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::term_structures::NsVariant;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::HashMap;
use time::Month;

use crate::calibration::calibration_support as cal_utils;

fn contractual_quote_dates(base: Date, index: &str, tenor: &str) -> (Date, Date) {
    use finstack_quant_core::dates::{adjust, calendar_by_id, DateExt};
    use finstack_quant_valuations::market::conventions::ConventionRegistry;

    let registry = ConventionRegistry::try_global().expect("conventions");
    let convention = registry
        .require_rate_index(&IndexId::new(index))
        .expect("index conventions");
    let calendar = calendar_by_id(&convention.market_calendar_id).expect("calendar");
    let spot = base
        .add_business_days(convention.market_settlement_days, calendar)
        .expect("spot date");
    let spot = adjust(spot, convention.market_business_day_convention, calendar)
        .expect("adjusted spot date");
    let maturity = Tenor::parse(tenor)
        .expect("tenor")
        .add_to_date(
            spot,
            Some(calendar),
            convention.market_business_day_convention,
        )
        .expect("contractual maturity");
    (spot, maturity)
}

/// Builds a set of deposit quotes with rates drawn from a known Nelson-Siegel curve.
///
/// The NS zero rate `r(T) = beta0 + (beta1+beta2)*(1-e^{-T/tau})/(T/tau) - beta2*e^{-T/tau}`
/// is used to set each deposit rate. Because deposit pricing is based on a
/// money-market day-count fraction that doesn't exactly equal the Act/365F
/// year fraction used by the NS curve, the best achievable residual (per
/// notional) is `~1e-4` — not `0`. This is fine for the normalization test:
/// the key assertion is that residuals are `O(1e-4)` in per-notional units,
/// NOT `O(1e2)` in raw PV units.
fn build_ns_derived_quotes(_base_date: Date) -> Vec<MarketQuote> {
    let ns_zero_rate = |t: f64| -> f64 {
        let beta0 = 0.04_f64;
        let beta1 = -0.02_f64;
        let beta2 = 0.01_f64;
        let tau = 2.0_f64;
        if t < 1e-9 {
            return beta0 + beta1;
        }
        let x = t / tau;
        let factor = (1.0 - (-x).exp()) / x;
        beta0 + beta1 * factor + beta2 * (factor - (-x).exp())
    };

    // Eight tenors spanning 3M–10Y give a well-determined NS fit.
    let tenors: &[(&str, f64)] = &[
        ("3M", 0.25),
        ("6M", 0.5),
        ("1Y", 1.0),
        ("2Y", 2.0),
        ("3Y", 3.0),
        ("5Y", 5.0),
        ("7Y", 7.0),
        ("10Y", 10.0),
    ];

    tenors
        .iter()
        .map(|(tenor_str, t)| {
            MarketQuote::Rates(RateQuote::Deposit {
                id: QuoteId::new(format!("DEP-{tenor_str}")),
                index: IndexId::new("USD-Deposit"),
                pillar: Pillar::Tenor(Tenor::parse(tenor_str).expect("valid tenor")),
                rate: ns_zero_rate(*t),
            })
        })
        .collect()
}

/// Builds wildly inconsistent deposit quotes that no NS curve can fit well.
///
/// Alternating extreme rates (0% and 20%) across the tenor grid create a
/// quote set that violates the smooth monotone shape assumption of the
/// Nelson-Siegel model. LM will converge to some minimum, but the residuals
/// should far exceed 1e-3 per unit notional.
fn build_inconsistent_quotes(_base_date: Date) -> Vec<MarketQuote> {
    let tenors: &[(&str, f64)] = &[
        ("3M", 0.0_f64),
        ("6M", 0.2_f64),
        ("1Y", 0.0_f64),
        ("2Y", 0.2_f64),
        ("3Y", 0.0_f64),
        ("5Y", 0.2_f64),
        ("7Y", 0.0_f64),
        ("10Y", 0.2_f64),
    ];

    tenors
        .iter()
        .map(|(tenor_str, rate)| {
            MarketQuote::Rates(RateQuote::Deposit {
                id: QuoteId::new(format!("DEP-{tenor_str}")),
                index: IndexId::new("USD-Deposit"),
                pillar: Pillar::Tenor(Tenor::parse(tenor_str).expect("valid tenor")),
                rate: *rate,
            })
        })
        .collect()
}

/// Run NS parametric calibration and return `(success, max_residual, residuals)`.
///
/// Uses the provided `settings` verbatim — no extra overrides. Pass
/// `CalibrationConfig::default()` for the production-default scenario.
fn run_parametric_ns_with_config(
    base_date: Date,
    curve_id: &str,
    quotes: Vec<MarketQuote>,
    mut settings: CalibrationConfig,
) -> (bool, f64, std::collections::BTreeMap<String, f64>) {
    let mut market_data: Vec<MarketDatum> = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &quotes);

    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("ns_quotes".to_string(), cal_utils::quote_set_ids(&quotes));

    // Do not throw on bad fit — return the report so we can inspect residuals.
    settings.fail_on_bad_fit = false;

    let plan = CalibrationPlan {
        id: "parametric_ns_plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings,
        steps: vec![CalibrationStep {
            id: "ns_step".to_string(),
            quote_set: "ns_quotes".to_string(),
            params: StepParams::Parametric(ParametricCurveParams {
                curve_id: curve_id.into(),
                base_date,
                model: NsVariant::Ns,
                initial_params: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let result = engine::execute(&envelope).expect("calibration engine must not error");
    let report = result
        .result
        .step_reports
        .get("ns_step")
        .expect("step report for 'ns_step' must be present");

    (
        report.success,
        report.max_residual,
        report.residuals.clone(),
    )
}

#[test]
fn parametric_ns_honors_explicit_acceptance_tolerance() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base_date");
    let quotes = build_ns_derived_quotes(base_date);
    let mut settings = CalibrationConfig::default();
    settings.discount_curve.validation_tolerance = 1e-8;
    let (strict_success, strict_residual, _) =
        run_parametric_ns_with_config(base_date, "USD-NS", quotes.clone(), settings.clone());
    assert!(!strict_success);
    assert!(strict_residual > 1e-8);
    settings.discount_curve.validation_tolerance = 1e-3;
    let (accepted, residual, _) =
        run_parametric_ns_with_config(base_date, "USD-NS", quotes, settings);
    assert!(accepted);
    assert!((residual - strict_residual).abs() < 1e-12);
}

// ─── Per-quote normalization test ──────────────────────────────────────────

/// Verify all per-quote residuals are in per-notional units.
///
/// Before the original fix, each per-quote residual was a raw PV amount (tens of
/// currency units). After the fix they are all `O(1e-4)` per notional.
#[test]
fn parametric_ns_per_quote_residuals_are_in_per_notional_units() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base_date");
    let quotes = build_ns_derived_quotes(base_date);

    let (_success, max_residual, residuals) = run_parametric_ns_with_config(
        base_date,
        "USD-NS-PER-QUOTE",
        quotes,
        CalibrationConfig::default(),
    );

    println!("max_residual={max_residual:.4e}");
    for (key, residual) in &residuals {
        println!("  {key}: {residual:.4e}");
    }

    // Each per-quote residual must be in per-notional units — O(1e-4), not O(100).
    // Threshold 1e-2 is 4 orders of magnitude below the bug-era residual.
    for (key, residual) in &residuals {
        let r: f64 = *residual;
        assert!(
            r.abs() < 1e-2,
            "Per-quote residual for '{key}' is {r:.4e} — expected < 1e-2 (per-notional). \
             A value near 100 means PV is not divided by notional (the pre-fix bug)."
        );
    }
}

// ─── Negative test: genuinely bad fit still reports failure ────────────────

/// A quote set that no NS curve can fit well must still report `success = false`.
///
/// This guards against the relaxed tolerance floor (1e-3) rubber-stamping
/// every calibration: a genuinely inconsistent quote set should produce
/// residuals far above 1e-3 and thus `success = false`.
#[test]
fn parametric_ns_calibration_fails_for_inconsistent_quotes() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base_date");
    let quotes = build_inconsistent_quotes(base_date);

    let (success, max_residual, _residuals) = run_parametric_ns_with_config(
        base_date,
        "USD-NS-BAD",
        quotes,
        CalibrationConfig::default(),
    );

    println!("NS bad-fit calibration: success={success}, max_residual={max_residual:.4e}");

    // Wildly inconsistent quotes (alternating 0% / 20%) cannot be fit by any
    // smooth NS curve; residuals must far exceed the 1e-3 tolerance floor.
    assert!(
        !success,
        "NS calibration on inconsistent quotes should report success=false. \
         max_residual={max_residual:.4e}. \
         If success=true, the tolerance floor may be too lenient."
    );

    assert!(
        max_residual > 1e-3,
        "Inconsistent-quote residual {max_residual:.4e} must exceed the 1e-3 per-notional threshold."
    );
}

#[test]
fn parametric_ns_prices_ordinary_swap_quotes_on_its_candidate_curve() {
    let mut envelope: CalibrationEnvelope = serde_json::from_str(include_str!(
        "../../examples/market_bootstrap/01_usd_discount.json"
    ))
    .expect("reference envelope");
    let base_date = Date::from_calendar_date(2026, Month::May, 8).expect("date");
    envelope.plan.steps[0].params = StepParams::Parametric(ParametricCurveParams {
        curve_id: "USD-NS".into(),
        base_date,
        model: NsVariant::Ns,
        initial_params: None,
    });
    envelope.plan.settings.fail_on_bad_fit = false;

    let result = engine::execute(&envelope).expect("swaps must have a projection curve role");
    let report = &result.result.step_reports[&envelope.plan.steps[0].id];
    assert_eq!(report.residuals.len(), 6);
    for id in [
        "USD-OIS-SWAP-1Y",
        "USD-OIS-SWAP-2Y",
        "USD-OIS-SWAP-5Y",
        "USD-OIS-SWAP-10Y",
    ] {
        assert!(
            report.residuals[id].is_finite(),
            "swap {id} was not repriced"
        );
    }
    assert!(
        report.max_residual < 1e-2,
        "candidate must fit the swap quotes"
    );
}

#[test]
fn parametric_delivered_models_reprice_short_end_deposits_analytically() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, DayCountContext};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{NelsonSiegelModel, ParametricCurve};
    use finstack_quant_core::market_data::traits::Discounting;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::deposit::{
        ConventionDepositParams, Deposit,
    };

    let base = Date::from_calendar_date(2025, Month::January, 2).expect("date");
    let models = [
        (
            NsVariant::Ns,
            NelsonSiegelModel::Ns {
                beta0: 0.03,
                beta1: 0.15,
                beta2: -0.1,
                tau: 0.05,
            },
        ),
        (
            NsVariant::Nss,
            NelsonSiegelModel::Nss {
                beta0: 0.03,
                beta1: 0.15,
                beta2: -0.1,
                beta3: 0.02,
                tau1: 0.05,
                tau2: 0.5,
            },
        ),
    ];
    for (variant, model) in models {
        let reference = ParametricCurve::builder("USD-NS")
            .base_date(base)
            .model(model.clone())
            .build()
            .expect("reference model");
        let mut quotes = Vec::new();
        let mut contracts = Vec::new();
        for tenor in ["1W", "2W", "1M", "2M", "6M", "1Y"] {
            let mut quote = RateQuote::Deposit {
                id: QuoteId::new(tenor),
                index: IndexId::new("USD-Deposit"),
                pillar: Pillar::Tenor(Tenor::parse(tenor).expect("tenor")),
                rate: 0.0,
            };
            let (_, maturity) = contractual_quote_dates(base, "USD-Deposit", tenor);
            let deposit = Deposit::from_conventions(ConventionDepositParams {
                id: tenor.into(),
                notional: Money::from((1_000_000_i64, Currency::USD)),
                trade_date: base,
                maturity,
                fixed_rate: 0.0,
                index_id: "USD-Deposit",
                discount_curve_id: "USD-NS",
                attributes: Default::default(),
            })
            .expect("deposit");
            let start_time = DayCount::Act365F
                .year_fraction(base, deposit.start_date, DayCountContext::default())
                .expect("start time");
            let end_time = DayCount::Act365F
                .year_fraction(base, deposit.maturity, DayCountContext::default())
                .expect("end time");
            let accrual = deposit
                .day_count
                .year_fraction(
                    deposit.start_date,
                    deposit.maturity,
                    DayCountContext::default(),
                )
                .expect("index accrual");
            let rate = (reference.df(start_time) / reference.df(end_time) - 1.0) / accrual;
            if let RateQuote::Deposit {
                rate: quote_rate, ..
            } = &mut quote
            {
                *quote_rate = rate;
            }
            contracts.push((quote.id().to_string(), start_time, end_time, accrual, rate));
            quotes.push(quote);
        }
        let mut settings = CalibrationConfig::default();
        settings.fail_on_bad_fit = true;
        settings.discount_curve.validation_tolerance = 1e-8;
        let envelope = CalibrationEnvelope {
            schema_url: None,
            schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
            plan: CalibrationPlan {
                id: "analytical-fit".to_string(),
                description: None,
                quote_sets: [(
                    "rates".to_string(),
                    quotes.iter().map(|q| q.id().clone()).collect(),
                )]
                .into_iter()
                .collect(),
                settings,
                steps: vec![CalibrationStep {
                    id: "ns".to_string(),
                    quote_set: "rates".to_string(),
                    params: StepParams::Parametric(ParametricCurveParams {
                        curve_id: "USD-NS".into(),
                        base_date: base,
                        model: variant,
                        initial_params: Some(model),
                    }),
                }],
            },
            market_data: quotes.into_iter().map(MarketDatum::RateQuote).collect(),
            prior_market: Vec::new(),
        };
        let result = engine::execute(&envelope).expect("exact analytical calibration");
        let report = &result.result.step_reports["ns"];
        assert!(report.success, "{}", report.convergence_reason);
        let context = MarketContext::try_from(result.result.final_market).expect("market");
        let delivered = context.get_parametric("USD-NS").expect("delivered model");
        for (id, start, end, accrual, rate) in contracts {
            // Independent deposit economics evaluated directly on the delivered
            // analytical object, without the calibration's DiscountCurve adapter.
            let residual = (1.0 + rate * accrual) * delivered.df(end) - delivered.df(start);
            assert!(residual.abs() < 1e-8, "{variant:?} {id}: {residual}");
            assert!(
                (report.residuals[&id] - residual).abs() < 1e-12,
                "reported residual must describe the delivered analytical curve"
            );
        }
    }
}

#[test]
fn parametric_delivered_curve_reprices_ois_with_payment_lag_analytically() {
    use finstack_quant_cashflows::builder::periods::{build_periods, BuildPeriodsParams};
    use finstack_quant_cashflows::builder::specs::RollRule;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{adjust, calendar_by_id, DayCount, DayCountContext};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{NelsonSiegelModel, ParametricCurve};
    use finstack_quant_core::market_data::traits::Discounting;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::irs::{
        ConventionSwapParams, FloatingLegCompounding, InterestRateSwap, PayReceive,
    };

    // Each term is (overnight accrual start, overnight accrual end, delayed
    // payment time, fixed coupon fraction). This reprices directly on the
    // delivered analytical object, without a sampled DiscountCurve adapter.
    fn legs_pv(curve: &dyn Discounting, terms: &[(f64, f64, f64, f64)]) -> (f64, f64) {
        terms.iter().fold(
            (0.0, 0.0),
            |(floating, annuity), &(start, end, pay, alpha)| {
                let payment_df = curve.df(pay);
                (
                    floating + (curve.df(start) / curve.df(end) - 1.0) * payment_df,
                    annuity + alpha * payment_df,
                )
            },
        )
    }

    let base = Date::from_calendar_date(2025, Month::January, 2).expect("date");
    let model = NelsonSiegelModel::Ns {
        beta0: 0.03,
        beta1: 0.15,
        beta2: -0.1,
        tau: 0.05,
    };
    let reference = ParametricCurve::builder("USD-NS-OIS")
        .base_date(base)
        .model(model.clone())
        .build()
        .expect("reference model");
    let time = |date| {
        DayCount::Act365F
            .year_fraction(base, date, DayCountContext::default())
            .expect("analytical curve time")
    };
    let mut quotes = Vec::new();
    let mut contracts = Vec::new();
    for tenor in ["1M", "6M", "2Y", "5Y"] {
        let mut quote = RateQuote::Swap {
            id: QuoteId::new(format!("OIS-{tenor}")),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Tenor(Tenor::parse(tenor).expect("tenor")),
            rate: 0.0,
            spread_decimal: None,
        };
        let (start_date, maturity) = contractual_quote_dates(base, "USD-SOFR-OIS", tenor);
        let swap = InterestRateSwap::from_conventions(ConventionSwapParams {
            id: format!("OIS-{tenor}").into(),
            notional: Money::from((1_000_000_i64, Currency::USD)),
            side: PayReceive::Pay,
            fixed_rate: 0.0,
            start_date,
            maturity,
            index_id: "USD-SOFR-OIS",
            discount_curve_id: "USD-NS-OIS",
            forward_curve_id: "USD-NS-OIS",
        })
        .expect("OIS contract");
        let fixed = &swap.fixed_leg;
        let float = &swap.float_leg;
        assert_eq!(float.compounding, FloatingLegCompounding::sofr());
        assert_eq!(
            (fixed.start, fixed.end, fixed.frequency),
            (float.start, float.end, float.frequency)
        );
        assert_eq!(fixed.payment_lag_days, float.payment_lag_days);
        assert!(
            fixed.payment_lag_days > 0,
            "regression requires delayed payment"
        );
        let calendar_id = fixed.calendar_id.as_deref().expect("OIS calendar");
        assert_eq!(float.calendar_id.as_deref(), Some(calendar_id));
        let calendar = calendar_by_id(calendar_id).expect("calendar");
        let periods = build_periods(BuildPeriodsParams {
            start: fixed.start,
            end: fixed.end,
            frequency: fixed.frequency,
            stub: fixed.stub,
            business_day_convention: fixed.business_day_convention,
            calendar_id,
            end_of_month: fixed.end_of_month,
            day_count: fixed.day_count,
            payment_lag_days: fixed.payment_lag_days,
            reset_lag_days: None,
            adjust_accrual_dates: false,
            roll_rule: RollRule::None,
        })
        .expect("actual contractual schedule");
        let terms: Vec<_> = periods
            .into_iter()
            .map(|period| {
                let start = adjust(
                    period.accrual_start,
                    float.business_day_convention,
                    calendar,
                )
                .expect("overnight accrual start");
                let end = adjust(period.accrual_end, float.business_day_convention, calendar)
                    .expect("overnight accrual end");
                assert!(period.payment_date > end, "payment delay must be retained");
                (
                    time(start),
                    time(end),
                    time(period.payment_date),
                    period.accrual_year_fraction,
                )
            })
            .collect();
        let (floating, annuity) = legs_pv(&reference, &terms);
        let par_rate = floating / annuity;
        if let RateQuote::Swap { rate, .. } = &mut quote {
            *rate = par_rate;
        }
        contracts.push((quote.id().to_string(), terms, par_rate));
        quotes.push(quote);
    }
    let mut settings = CalibrationConfig::default();
    settings.fail_on_bad_fit = true;
    settings.discount_curve.validation_tolerance = 1e-8;
    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
        plan: CalibrationPlan {
            id: "analytical-ois-fit".to_string(),
            description: None,
            quote_sets: [(
                "rates".to_string(),
                quotes.iter().map(|q| q.id().clone()).collect(),
            )]
            .into_iter()
            .collect(),
            settings,
            steps: vec![CalibrationStep {
                id: "ns".to_string(),
                quote_set: "rates".to_string(),
                params: StepParams::Parametric(ParametricCurveParams {
                    curve_id: "USD-NS-OIS".into(),
                    base_date: base,
                    model: NsVariant::Ns,
                    initial_params: Some(model),
                }),
            }],
        },
        market_data: quotes.into_iter().map(MarketDatum::RateQuote).collect(),
        prior_market: Vec::new(),
    };
    let result = engine::execute(&envelope).expect("exact analytical OIS calibration");
    let report = &result.result.step_reports["ns"];
    assert!(report.success, "{}", report.convergence_reason);
    assert!(report.max_residual < 1e-8);
    let context = MarketContext::try_from(result.result.final_market).expect("market");
    let delivered = context
        .get_parametric("USD-NS-OIS")
        .expect("delivered model");
    for (id, terms, par_rate) in contracts {
        let (floating, annuity) = legs_pv(delivered.as_ref(), &terms);
        let residual = floating - par_rate * annuity;
        assert!(residual.abs() < 1e-8, "{id}: {residual}");
        assert!(
            (report.residuals[&id] - residual).abs() < 1e-12,
            "reported OIS residual must match delivered analytical PV for {id}"
        );
    }
}
