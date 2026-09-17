//! Deterministic config-sensitivity table: every public behavioral knob on
//! `StructuredCredit`, `AssetPool`, `PoolAsset` and `Tranche` must move the
//! projected tranche cashflows. A knob that validates but never reaches the
//! engine produces bit-identical flows, the "silently inert config" class
//! from the 2026-07 and 2026-09 structured-credit audits.
//!
//! Knobs deliberately absent from the table:
//! - `principal_covers_senior_interest` (needs an interest shortfall; covered
//!   by `funding_tests`), `waterfall_rules` (each spec has its own suite in
//!   `production_waterfall_audit`), `hedge_swaps` beyond one entry
//!   (`hedge_swap_tests`).
//! - `market_conditions.refi_rate` only feeds the stochastic prepayment tree.
//! - `pool.reserve_target` governs replenishment from revolver repayments, so
//!   it only acts on instrument collateral (`instrument_pool_tests`).
//! - `deal_metadata`, `credit_factors`, `attributes`, pricing overrides,
//!   `quote_settlement_date` and the pool's `cumulative_*` fields are labels,
//!   metric inputs or reporting state, not projection inputs.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CreditRating, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AdvancingPolicy, AssetPool, CoverageRules, CoverageTestSpec, CoverageTrigger,
    DealFees, DealType, DelinquencyModel, HedgeSwap, IncentiveFeeSpec, LossAllocationPolicy,
    ModificationSpec, PoolAsset, ReinvestmentAssumptions, ReinvestmentCriteria, ReinvestmentPeriod,
    StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    TriggerConsequence,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    BalloonSpec, CallAssumption, CardPortfolioSpec, EarlyAmortizationSpec, LiquidationSpec,
    PrepaymentPenalty, SpecialServicingSpec, WaterfallRules,
};
use finstack_quant_valuations::instruments::PayReceive;
use time::Month;

use super::instrument_pool_tests::market_with_curves;
use crate::test_support::rates::usd_irs_swap;

fn ymd(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

const CLOSE: (i32, u8, u8) = (2024, 1, 1);
const MATURITY: (i32, u8, u8) = (2032, 1, 1);

fn close() -> Date {
    ymd(CLOSE.0, CLOSE.1, CLOSE.2)
}

fn maturity() -> Date {
    ymd(MATURITY.0, MATURITY.1, MATURITY.2)
}

/// Quarterly CLO: ten 10M BB bullet loans at 8%, A 60 / B 30 / E 10 (equity),
/// standard CLO fees, 10% CPR, 2% CDR, 40% recovery after 12 months, and an
/// OC test on A at 1.20 that passes at par (1.67). Equity is thin enough for
/// the residual stream to earn the incentive-fee hurdle inside the deal's life.
fn baseline() -> StructuredCredit {
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    for i in 0..10 {
        let mut asset = PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(10_000_000.0),
            0.08,
            maturity(),
            DayCount::Act360,
        );
        asset.credit_quality = Some(CreditRating::BB);
        pool.assets.push(asset);
    }
    let tranches = TrancheStructure::new(vec![
        Tranche::new(
            "A",
            0.0,
            60.0,
            TrancheSeniority::Senior,
            usd(60_000_000.0),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity(),
        )
        .expect("A"),
        Tranche::new(
            "B",
            60.0,
            90.0,
            TrancheSeniority::Mezzanine,
            usd(30_000_000.0),
            TrancheCoupon::Fixed { rate: 0.07 },
            maturity(),
        )
        .expect("B"),
        Tranche::new(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            usd(10_000_000.0),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity(),
        )
        .expect("E"),
    ])
    .expect("structure");
    let mut deal =
        StructuredCredit::new_clo("CLO-KNOBS", pool, tranches, close(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse")
            .with_fees(DealFees::clo_standard(Currency::USD))
            .with_coverage_triggers(vec![CoverageTestSpec::oc("A", 1.20)])
            .expect("coverage test");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.10);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.02);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 12);
    deal
}

fn window() -> ReinvestmentPeriod {
    ReinvestmentPeriod {
        end_date: ymd(2027, 1, 1),
        is_active: true,
        criteria: ReinvestmentCriteria::default(),
        amortizing_tranches: Vec::new(),
        assumptions: Some(ReinvestmentAssumptions {
            spread_bp: 400.0,
            price_pct: 100.0,
            maturity_months: 60,
            index_id: None,
            coupon_floor: None,
        }),
    }
}

fn fees(deal: &mut StructuredCredit) -> &mut DealFees {
    deal.fees.as_mut().expect("baseline carries fees")
}

fn tranche<'a>(deal: &'a mut StructuredCredit, id: &str) -> &'a mut Tranche {
    deal.tranches
        .tranches
        .iter_mut()
        .find(|t| t.id.as_str() == id)
        .expect("tranche")
}

fn reinvestment(deal: &mut StructuredCredit) -> &mut ReinvestmentPeriod {
    deal.pool
        .reinvestment_period
        .as_mut()
        .expect("common setup opened a window")
}

/// Every tranche flow (interest, principal, PIK, deferred, write-down) by
/// class and date, rounded to the cent.
fn fingerprint(deal: &StructuredCredit, market: &MarketContext) -> Vec<(String, Date, i64)> {
    let results = run_simulation(deal, market, close()).expect("simulation");
    let mut ids: Vec<&String> = results.keys().collect();
    ids.sort();
    let mut out = Vec::new();
    for id in ids {
        let r = &results[id];
        let groups: [(&str, &[(Date, Money)]); 5] = [
            ("interest", &r.interest_flows),
            ("principal", &r.principal_flows),
            ("pik", &r.pik_flows),
            ("deferred", &r.deferred_flows),
            ("writedown", &r.writedown_flows),
        ];
        for (kind, flows) in groups {
            for (date, amount) in flows {
                out.push((
                    format!("{id}:{kind}"),
                    *date,
                    (amount.amount() * 100.0).round() as i64,
                ));
            }
        }
    }
    out
}

struct Knob {
    name: &'static str,
    /// Applied to both the reference and the varied deal (opens a
    /// reinvestment window, funds a reserve, ...), so the knob is the only
    /// difference between the two runs.
    common: fn(&mut StructuredCredit),
    change: fn(&mut StructuredCredit),
}

fn none(_: &mut StructuredCredit) {}

fn open_window(deal: &mut StructuredCredit) {
    deal.pool.reinvestment_period = Some(window());
}

fn fund_reserve(deal: &mut StructuredCredit) {
    deal.pool.reserve_account = usd(5_000_000.0);
}

fn delinquency_model(deal: &mut StructuredCredit) {
    deal.credit_model.delinquency = Some(DelinquencyModel::new(
        vec![0.5, 0.5, 0.5],
        vec![0.2, 0.2, 0.2],
    ));
}

fn card_portfolio(deal: &mut StructuredCredit) {
    deal.credit_model.card = Some(CardPortfolioSpec::new(0.15, 0.18, 0.05));
}

fn divert_from_b(deal: &mut StructuredCredit) {
    deal.coverage_triggers.push(CoverageTestSpec::ic("A", 5.0));
}

/// Tranche-level triggers may not coexist with a deal-level test on the same
/// class, so the tranche-trigger knobs start from a deal without one.
fn clear_deal_tests(deal: &mut StructuredCredit) {
    deal.coverage_triggers.clear();
}

/// The incentive fee only pays once equity's IRR to date clears the hurdle;
/// with the baseline's 7% class B the residual never returns the 10M before
/// legal final, so the hurdle knob starts from a cheaper mezzanine coupon.
fn cheap_mezzanine(deal: &mut StructuredCredit) {
    tranche(deal, "B").coupon = TrancheCoupon::Fixed { rate: 0.03 };
}

fn knobs() -> Vec<Knob> {
    vec![
        // --- StructuredCredit -------------------------------------------
        Knob {
            name: "frequency",
            common: none,
            change: |d| d.frequency = Tenor::monthly(),
        },
        Knob {
            name: "payment_business_day_convention",
            common: none,
            change: |d| d.payment_business_day_convention = Some(BusinessDayConvention::Preceding),
        },
        Knob {
            name: "credit_model.prepayment_spec",
            common: none,
            change: |d| d.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.20),
        },
        Knob {
            name: "credit_model.default_spec",
            common: none,
            change: |d| d.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.05),
        },
        Knob {
            name: "credit_model.recovery_spec",
            common: none,
            change: |d| d.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.7, 3),
        },
        Knob {
            name: "behavior_overrides.cpr_annual",
            common: none,
            change: |d| d.behavior_overrides.cpr_annual = Some(0.25),
        },
        Knob {
            name: "credit_model.prepayment_spec (abs curve)",
            common: none,
            change: |d| d.credit_model.prepayment_spec = PrepaymentModelSpec::abs(0.02),
        },
        Knob {
            name: "credit_model.prepayment_spec (vector curve)",
            common: none,
            change: |d| {
                d.credit_model.prepayment_spec = PrepaymentModelSpec::vector(vec![0.02, 0.05, 0.20])
            },
        },
        Knob {
            name: "credit_model.default_spec (vector curve)",
            common: none,
            change: |d| d.credit_model.default_spec = DefaultModelSpec::vector(vec![0.01, 0.06]),
        },
        Knob {
            name: "credit_model.default_spec (cumulative loss curve)",
            common: none,
            change: |d| {
                d.credit_model.default_spec =
                    DefaultModelSpec::cumulative_loss(vec![0.5, 1.0, 2.0, 3.0, 4.0, 5.0], 0.6)
            },
        },
        Knob {
            name: "credit_model.default_spec (timing curve)",
            common: none,
            change: |d| {
                d.credit_model.default_spec =
                    DefaultModelSpec::timing(0.08, vec![15.0, 30.0, 30.0, 15.0, 10.0])
            },
        },
        Knob {
            name: "credit_model.delinquency",
            common: none,
            change: delinquency_model,
        },
        Knob {
            name: "credit_model.delinquency.advancing",
            common: delinquency_model,
            change: |d| {
                d.credit_model
                    .delinquency
                    .as_mut()
                    .expect("model")
                    .advancing = AdvancingPolicy::PrincipalAndInterest {
                    recoverability_cap_pct: 100.0,
                }
            },
        },
        Knob {
            name: "credit_model.delinquency.modification",
            common: delinquency_model,
            change: |d| {
                d.credit_model
                    .delinquency
                    .as_mut()
                    .expect("model")
                    .modification = Some(ModificationSpec {
                    rate_reduction_bp: 200.0,
                    term_extension_months: 12,
                    share_of_delinquent: 0.5,
                })
            },
        },
        Knob {
            name: "pool.assets[0].delinquency_buckets",
            common: delinquency_model,
            change: |d| {
                d.pool.assets[0].delinquency_buckets =
                    Some(vec![usd(2_000_000.0), usd(0.0), usd(0.0)])
            },
        },
        Knob {
            name: "pool.assets[0].balloon",
            common: |d| d.pool.assets[0].maturity = ymd(2027, 1, 1),
            change: |d| {
                d.pool.assets[0].balloon = Some(BalloonSpec {
                    default_prob: 0.3,
                    extension_months: 24,
                    extension_rate: None,
                })
            },
        },
        Knob {
            name: "pool.assets[0].prepayment_penalty",
            common: none,
            change: |d| {
                d.pool.assets[0].prepayment_penalty = Some(PrepaymentPenalty::Fixed {
                    pct: 3.0,
                    through: None,
                })
            },
        },
        Knob {
            name: "pool.assets[0].special_servicing",
            common: none,
            change: |d| {
                d.pool.assets[0].special_servicing = Some(SpecialServicingSpec {
                    appraisal_reduction_pct: 40.0,
                })
            },
        },
        Knob {
            name: "pool.assets[0].liquidation",
            common: none,
            change: |d| {
                d.pool.assets[0].liquidation = Some(LiquidationSpec {
                    months_to_resolution: 12,
                    proceeds_pct: 55.0,
                    carry_cost_pct: 5.0,
                    reperformance_prob: 0.2,
                    modified_rate: Some(0.04),
                })
            },
        },
        Knob {
            name: "credit_model.recovery_spec.severity_vector",
            common: none,
            change: |d| {
                d.credit_model.recovery_spec =
                    RecoveryModelSpec::with_lag(0.4, 12).with_severity_vector(vec![0.9, 0.8, 0.7])
            },
        },
        Knob {
            name: "behavior_overrides.psa_speed_multiplier",
            common: none,
            change: |d| d.behavior_overrides.psa_speed_multiplier = Some(3.0),
        },
        Knob {
            name: "behavior_overrides.cdr_annual",
            common: none,
            change: |d| d.behavior_overrides.cdr_annual = Some(0.06),
        },
        Knob {
            name: "behavior_overrides.sda_speed_multiplier",
            common: none,
            change: |d| d.behavior_overrides.sda_speed_multiplier = Some(3.0),
        },
        Knob {
            name: "behavior_overrides.recovery_rate",
            common: none,
            change: |d| d.behavior_overrides.recovery_rate = Some(0.8),
        },
        Knob {
            name: "behavior_overrides.recovery_lag_months",
            common: none,
            change: |d| d.behavior_overrides.recovery_lag_months = Some(3),
        },
        Knob {
            name: "behavior_overrides.reinvestment_price",
            common: open_window,
            change: |d| d.behavior_overrides.reinvestment_price = Some(90.0),
        },
        Knob {
            name: "hedge_swaps",
            common: none,
            change: |d| {
                let mut swap = usd_irs_swap(
                    InstrumentId::new("HEDGE-A"),
                    usd(60_000_000.0),
                    0.04,
                    close(),
                    maturity(),
                    PayReceive::Pay,
                )
                .expect("swap");
                swap.fixed.frequency = Tenor::quarterly();
                swap.float.frequency = Tenor::quarterly();
                d.hedge_swaps.push(HedgeSwap::new(swap).on_tranche_par("A"));
            },
        },
        Knob {
            name: "fees.trustee_fee_annual",
            common: none,
            change: |d| fees(d).trustee_fee_annual = usd(500_000.0),
        },
        Knob {
            name: "fees.senior_mgmt_fee_bp",
            common: none,
            change: |d| fees(d).senior_mgmt_fee_bp = 200.0,
        },
        Knob {
            name: "fees.subordinated_mgmt_fee_bp",
            common: none,
            change: |d| fees(d).subordinated_mgmt_fee_bp = 200.0,
        },
        Knob {
            name: "fees.servicing_fee_bp",
            common: none,
            change: |d| fees(d).servicing_fee_bp = 100.0,
        },
        Knob {
            name: "fees.master_servicer_fee_bp",
            common: none,
            change: |d| fees(d).master_servicer_fee_bp = Some(50.0),
        },
        Knob {
            name: "fees.special_servicer_fee_bp",
            // The fee accrues on specially serviced balances only.
            common: |d| {
                d.pool.assets[0].special_servicing = Some(SpecialServicingSpec {
                    appraisal_reduction_pct: 0.0,
                })
            },
            change: |d| fees(d).special_servicer_fee_bp = Some(50.0),
        },
        Knob {
            name: "fees.incentive_fee",
            common: cheap_mezzanine,
            change: |d| {
                fees(d).incentive_fee = Some(IncentiveFeeSpec {
                    hurdle_irr: 0.0,
                    share_pct: 0.5,
                })
            },
        },
        Knob {
            name: "coverage_triggers",
            common: none,
            change: divert_from_b,
        },
        Knob {
            name: "coverage_rules",
            common: none,
            change: |d| {
                d.coverage_rules = Some(CoverageRules {
                    rating_haircuts: [(CreditRating::BB, 0.5)].into_iter().collect(),
                    ..Default::default()
                })
            },
        },
        Knob {
            name: "cleanup_call_pct",
            common: none,
            change: |d| d.cleanup_call_pct = Some(0.6),
        },
        Knob {
            name: "loss_allocation",
            common: none,
            change: |d| d.loss_allocation = Some(LossAllocationPolicy::WriteDown),
        },
        Knob {
            name: "credit_model.card",
            common: none,
            change: card_portfolio,
        },
        Knob {
            name: "credit_model.card.monthly_payment_rate",
            common: card_portfolio,
            change: |d| {
                d.credit_model
                    .card
                    .as_mut()
                    .expect("card")
                    .monthly_payment_rate = 0.10
            },
        },
        Knob {
            name: "credit_model.card.portfolio_yield",
            common: card_portfolio,
            change: |d| d.credit_model.card.as_mut().expect("card").portfolio_yield = 0.22,
        },
        Knob {
            name: "credit_model.card.charge_off_rate",
            common: card_portfolio,
            change: |d| d.credit_model.card.as_mut().expect("card").charge_off_rate = 0.10,
        },
        Knob {
            name: "waterfall_rules.early_amortization.min_excess_spread_3m",
            common: open_window,
            change: |d| {
                d.waterfall_rules = Some(WaterfallRules {
                    afc: None,
                    excess_spread: None,
                    step_down: None,
                    shifting_interest: None,
                    early_amortization: Some(EarlyAmortizationSpec {
                        max_cumulative_loss_pct: 1.0,
                        min_excess_spread_3m: Some(0.5),
                    }),
                    controlled_accumulation: None,
                })
            },
        },
        Knob {
            name: "call_assumption",
            common: none,
            change: |d| d.call_assumption = Some(CallAssumption::new(ymd(2027, 1, 1), 100.0)),
        },
        Knob {
            name: "liquidation_price_pct",
            common: |d| d.cleanup_call_pct = Some(0.6),
            change: |d| d.liquidation_price_pct = Some(80.0),
        },
        // --- AssetPool ----------------------------------------------------
        Knob {
            name: "pool.reinvestment_period",
            common: none,
            change: open_window,
        },
        Knob {
            name: "pool.reinvestment_period.end_date",
            common: open_window,
            change: |d| reinvestment(d).end_date = ymd(2029, 1, 1),
        },
        Knob {
            name: "pool.reinvestment_period.is_active",
            common: open_window,
            change: |d| reinvestment(d).is_active = false,
        },
        Knob {
            name: "pool.reinvestment_period.amortizing_tranches",
            common: open_window,
            change: |d| reinvestment(d).amortizing_tranches = vec!["A".to_string()],
        },
        Knob {
            name: "pool.reinvestment_period.assumptions.spread_bp",
            common: open_window,
            change: |d| {
                reinvestment(d)
                    .assumptions
                    .as_mut()
                    .expect("assumptions")
                    .spread_bp = 800.0
            },
        },
        Knob {
            name: "pool.reinvestment_period.assumptions.price_pct",
            common: open_window,
            change: |d| {
                reinvestment(d)
                    .assumptions
                    .as_mut()
                    .expect("assumptions")
                    .price_pct = 90.0
            },
        },
        Knob {
            name: "pool.reinvestment_period.assumptions.maturity_months",
            common: open_window,
            change: |d| {
                reinvestment(d)
                    .assumptions
                    .as_mut()
                    .expect("assumptions")
                    .maturity_months = 24
            },
        },
        Knob {
            name: "pool.collection_account",
            common: none,
            change: |d| d.pool.collection_account = usd(1_000_000.0),
        },
        Knob {
            name: "pool.reserve_account",
            common: none,
            change: fund_reserve,
        },
        Knob {
            name: "pool.reserve_account_rate",
            common: fund_reserve,
            change: |d| d.pool.reserve_account_rate = 0.05,
        },
        // --- PoolAsset ----------------------------------------------------
        Knob {
            name: "pool.assets[0].smm_override",
            common: none,
            change: |d| d.pool.assets[0].smm_override = Some(0.05),
        },
        Knob {
            name: "pool.assets[0].mdr_override",
            common: none,
            change: |d| d.pool.assets[0].mdr_override = Some(0.02),
        },
        Knob {
            name: "pool.assets[0].recovery_rate",
            common: none,
            change: |d| d.pool.assets[0].recovery_rate = Some(0.9),
        },
        Knob {
            name: "pool.assets[0].rate",
            common: none,
            change: |d| d.pool.assets[0].rate = 0.12,
        },
        Knob {
            name: "pool.assets[0].is_defaulted",
            common: none,
            change: |d| {
                d.pool.assets[0].is_defaulted = true;
                d.pool.assets[0].default_date = Some(ymd(2023, 12, 1));
                d.pool.assets[0].recovery_amount = Some(usd(4_000_000.0));
            },
        },
        // --- Tranche ------------------------------------------------------
        Knob {
            name: "tranches.A.coupon",
            common: none,
            change: |d| tranche(d, "A").coupon = TrancheCoupon::Fixed { rate: 0.06 },
        },
        Knob {
            name: "tranches.A.day_count",
            common: none,
            change: |d| tranche(d, "A").day_count = DayCount::Thirty360,
        },
        Knob {
            name: "tranches.A.current_balance",
            common: none,
            change: |d| tranche(d, "A").current_balance = usd(50_000_000.0),
        },
        Knob {
            name: "tranches.A.deferred_interest",
            common: none,
            change: |d| tranche(d, "A").deferred_interest = usd(1_000_000.0),
        },
        Knob {
            name: "tranches.B.pik_enabled",
            common: divert_from_b,
            change: |d| tranche(d, "B").pik_enabled = true,
        },
        Knob {
            name: "tranches.A.oc_trigger",
            common: clear_deal_tests,
            change: |d| {
                tranche(d, "A").oc_trigger = Some(CoverageTrigger::new(
                    1.70,
                    TriggerConsequence::DivertCashFlow,
                ))
            },
        },
        Knob {
            name: "tranches.A.ic_trigger",
            common: clear_deal_tests,
            change: |d| {
                tranche(d, "A").ic_trigger = Some(CoverageTrigger::new(
                    5.0,
                    TriggerConsequence::DivertCashFlow,
                ))
            },
        },
        Knob {
            name: "tranches.payment_priority",
            common: none,
            change: |d| {
                tranche(d, "A").payment_priority = 2;
                tranche(d, "B").payment_priority = 1;
            },
        },
    ]
}

/// Each knob, varied alone, must change at least one projected tranche flow.
#[test]
fn every_deterministic_knob_moves_the_tranche_cashflows() {
    let market = market_with_curves(close());
    let mut inert = Vec::new();

    for knob in knobs() {
        let mut reference = baseline();
        (knob.common)(&mut reference);
        let mut varied = reference.clone();
        (knob.change)(&mut varied);

        let before = fingerprint(&reference, &market);
        let after = fingerprint(&varied, &market);
        if before == after {
            inert.push(knob.name);
        }
    }

    assert!(
        inert.is_empty(),
        "knobs that never reached the engine (identical cashflows): {inert:?}"
    );
}
