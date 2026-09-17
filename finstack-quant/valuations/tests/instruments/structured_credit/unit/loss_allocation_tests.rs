//! Loss-allocation policy: CLO/ABS notes carry par until legal final
//! (`ParPreserving`), RMBS/CMBS notes are written down at default
//! (`WriteDown`). The 2026-09-15 audit found every deal type written down at
//! default, so a CLO Class D lost its coupon while equity kept receiving
//! residual cash.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AssetPool, DealType, LossAllocationPolicy, PoolAsset, StructuredCredit,
    Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), day).expect("date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("money")
}

fn market(as_of: Date) -> MarketContext {
    let knots: Vec<(f64, f64)> = (0..=12)
        .map(|i| (f64::from(i), (-0.05 * f64::from(i)).exp()))
        .collect();
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(knots)
        .build()
        .expect("curve");
    MarketContext::new().insert(curve)
}

/// Five-class CLO from the audit: ten 10M loans at 8%, A 60 / B 15 / C 10 /
/// D 5 / E 10 (equity), CDR 5%, recovery 40% with an 18-month lag, no
/// coverage tests.
fn clo(policy: Option<LossAllocationPolicy>) -> (StructuredCredit, Date) {
    let close = d(2024, 1, 1);
    let maturity = d(2032, 1, 1);
    let mut pool = AssetPool::new("P", DealType::Clo, Currency::USD);
    for i in 0..10 {
        pool.assets.push(PoolAsset::fixed_rate_bond(
            format!("L{i}"),
            usd(10_000_000.0),
            0.08,
            maturity,
            DayCount::Act360,
        ));
    }
    let tr = |id: &str, a: f64, b: f64, sen: TrancheSeniority, bal: f64, cpn: f64| {
        Tranche::new(
            id,
            a,
            b,
            sen,
            usd(bal),
            TrancheCoupon::Fixed { rate: cpn },
            maturity,
        )
        .expect("tranche")
    };
    let tranches = TrancheStructure::new(vec![
        tr("A", 0.0, 60.0, TrancheSeniority::Senior, 60_000_000.0, 0.05),
        tr(
            "B",
            60.0,
            75.0,
            TrancheSeniority::Mezzanine,
            15_000_000.0,
            0.07,
        ),
        tr(
            "C",
            75.0,
            85.0,
            TrancheSeniority::Mezzanine,
            10_000_000.0,
            0.09,
        ),
        tr(
            "D",
            85.0,
            90.0,
            TrancheSeniority::Subordinated,
            5_000_000.0,
            0.12,
        ),
        tr(
            "E",
            90.0,
            100.0,
            TrancheSeniority::Equity,
            10_000_000.0,
            0.0,
        ),
    ])
    .expect("structure");
    let mut deal = StructuredCredit::new_clo(
        "CLO-LOSS-POLICY",
        pool,
        tranches,
        close,
        maturity,
        "USD-OIS",
    )
    .with_payment_calendar("nyse");
    deal.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
    deal.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.05);
    deal.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.4, 18);
    deal.loss_allocation = policy;
    (deal, close)
}

#[test]
fn policy_defaults_follow_the_deal_type() {
    assert_eq!(
        LossAllocationPolicy::default_for(DealType::Clo),
        LossAllocationPolicy::ParPreserving
    );
    assert_eq!(
        LossAllocationPolicy::default_for(DealType::Abs),
        LossAllocationPolicy::ParPreserving
    );
    assert_eq!(
        LossAllocationPolicy::default_for(DealType::Rmbs),
        LossAllocationPolicy::WriteDown
    );
    assert_eq!(
        LossAllocationPolicy::default_for(DealType::Cmbs),
        LossAllocationPolicy::WriteDown
    );
    let (deal, _) = clo(None);
    assert_eq!(
        deal.effective_loss_allocation(),
        LossAllocationPolicy::ParPreserving,
        "a CLO with no explicit policy is par-preserving"
    );
}

#[test]
fn clo_notes_keep_par_and_their_coupon_until_legal_final() {
    let (deal, as_of) = clo(None);
    let market = market(as_of);
    let results = run_simulation(&deal, &market, as_of).expect("simulation");
    let d_note = &results["D"];

    let final_date = results
        .values()
        .flat_map(|t| t.cashflows.iter().map(|(date, _)| *date))
        .max()
        .expect("cashflows");
    assert!(
        d_note
            .writedown_flows
            .iter()
            .all(|(date, _)| *date >= final_date),
        "a par-preserving note is only written down at legal final; got {:?}",
        d_note.writedown_flows.first()
    );
    assert!(
        d_note.total_deferred.amount() < 1.0,
        "with ample interest proceeds D's coupon is never deferred, got {}",
        d_note.total_deferred.amount()
    );
    // 5M × 12% × ~0.25 (Act/360 quarter) ≈ 150k every period on full par.
    assert!(
        d_note.interest_flows.len() >= 30,
        "D is paid every period, got {} coupons",
        d_note.interest_flows.len()
    );
    for (date, coupon) in &d_note.interest_flows {
        assert!(
            coupon.amount() > 140_000.0,
            "D coupon on {date} must accrue on full par, got {}",
            coupon.amount()
        );
    }
    // Principal received plus the terminal shortfall reconciles to par.
    let reconciled = d_note.total_principal.amount() + d_note.total_writedown.amount();
    assert!(
        (reconciled - 5_000_000.0).abs() < 1.0,
        "principal {} + terminal write-down {} must equal D's par",
        d_note.total_principal.amount(),
        d_note.total_writedown.amount()
    );
    assert!(
        d_note.total_writedown.amount() > 0.0,
        "at 5% CDR / 40% recovery the D note does not get all of its par back"
    );
    assert!(
        d_note.final_balance.amount().abs() < 1.0,
        "nothing is outstanding after legal final"
    );
}

#[test]
fn residual_is_paid_only_after_the_junior_note_coupon() {
    let (deal, as_of) = clo(None);
    let market = market(as_of);
    let results = run_simulation(&deal, &market, as_of).expect("simulation");
    let d_note = &results["D"];
    let equity = &results["E"];
    for (date, residual) in &equity.interest_flows {
        if residual.amount() <= 0.0 {
            continue;
        }
        let d_coupon = d_note
            .interest_flows
            .iter()
            .find(|(cd, _)| cd == date)
            .map_or(0.0, |(_, m)| m.amount());
        assert!(
            d_coupon > 140_000.0,
            "equity received {} on {date} while D's coupon was {d_coupon}",
            residual.amount()
        );
    }
}

#[test]
fn write_down_policy_allocates_losses_at_default() {
    let (deal, as_of) = clo(Some(LossAllocationPolicy::WriteDown));
    let market = market(as_of);
    let results = run_simulation(&deal, &market, as_of).expect("simulation");
    let d_note = &results["D"];
    let first = d_note
        .writedown_flows
        .first()
        .map(|(date, _)| *date)
        .expect("D is written down under WriteDown");
    assert!(
        first < d(2030, 1, 1),
        "realized-loss allocation writes D down before legal final, first on {first}"
    );
    let (par_preserving, _) = clo(None);
    let pp = run_simulation(&par_preserving, &market, as_of).expect("simulation");
    assert!(
        pp["D"].total_interest.amount() > d_note.total_interest.amount() + 500_000.0,
        "the policy must change D's coupon stream: par-preserving {} vs write-down {}",
        pp["D"].total_interest.amount(),
        d_note.total_interest.amount()
    );
}
