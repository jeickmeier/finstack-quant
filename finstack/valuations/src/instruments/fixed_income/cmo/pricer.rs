//! Agency CMO pricing.
//!
//! CMO pricing projects collateral cashflows and distributes them
//! through the waterfall to calculate the PV of the reference tranche.

use super::tranches::pac_support::PacSchedule;
use super::types::{AgencyCmo, CmoTranche, CmoTrancheType, PacCollar};
use super::waterfall::{
    allocate_io_cashflow, execute_waterfall_with_principal_breakdown, PacContext,
};
use crate::cashflow::builder::specs::{PrepaymentCurve, PrepaymentModelSpec};
use crate::cashflow::builder::{CashFlowMeta, CashFlowSchedule};
use crate::cashflow::primitives::{CFKind, CashFlow};
use crate::instruments::fixed_income::mbs_passthrough::pricer::generate_cashflows;
use crate::instruments::fixed_income::mbs_passthrough::{AgencyMbsPassthrough, PoolType};
use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::InstrumentId;
use finstack_core::Result;

/// Tranche cashflow for a single period.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct TrancheCashflow {
    /// Payment date
    pub(crate) payment_date: Date,
    /// Principal payment
    pub(crate) principal: f64,
    /// Scheduled principal payment
    pub(crate) scheduled_principal: f64,
    /// Prepayment principal payment
    pub(crate) prepayment_principal: f64,
    /// Interest payment
    pub(crate) interest: f64,
    /// Total payment
    pub(crate) total: f64,
    /// Ending balance after this period
    pub(crate) ending_balance: f64,
}

/// Resolve the collateral pool used as the canonical source for tranche projection.
pub(crate) fn resolve_collateral(cmo: &AgencyCmo) -> Result<AgencyMbsPassthrough> {
    if let Some(ref pool) = cmo.collateral {
        Ok(pool.as_ref().clone())
    } else {
        create_assumed_collateral(cmo)
    }
}

/// Extract the actual PSA speed multiplier from a prepayment model spec.
///
/// PAC collar checks compare the realized prepayment speed (PSA multiple) of
/// the collateral against the PAC band. A `Psa { speed_multiplier }` model maps
/// directly. For non-PSA models the speed is approximated by converting the
/// terminal CPR back to a PSA multiple (terminal PSA CPR is 6%), which lets the
/// collar check still distinguish slow/fast pools.
fn actual_psa_from_model(model: &PrepaymentModelSpec) -> f64 {
    match &model.curve {
        Some(PrepaymentCurve::Psa { speed_multiplier }) => *speed_multiplier,
        // Constant / lockout: map CPR to a PSA-equivalent multiple (100% PSA
        // terminal CPR = 6%). Clamped non-negative for safety.
        _ => (model.cpr / 0.06).max(0.0),
    }
}

/// Build the PAC context that drives PAC-schedule amortization in the waterfall.
///
/// Returns `None` when the deal has no PAC tranche, so non-PAC deals keep the
/// plain sequential/pro-rata allocation. When a PAC tranche is present, the PAC
/// schedule is generated from the *collateral* balance (the correct basis for
/// the PAC band) and the carved PAC tranche balance.
fn build_pac_context(cmo: &AgencyCmo, collateral: &AgencyMbsPassthrough) -> Option<PacContext> {
    let pac_tranches: Vec<&CmoTranche> = cmo
        .waterfall
        .tranches
        .iter()
        .filter(|t| t.tranche_type == CmoTrancheType::Pac)
        .collect();
    if pac_tranches.is_empty() {
        return None;
    }

    // Aggregate PAC face across all PAC tranches.
    let pac_balance: f64 = pac_tranches.iter().map(|t| t.current_face.amount()).sum();

    // Use the collar from the first PAC tranche, or the standard 100-300 collar.
    let collar = pac_tranches
        .iter()
        .find_map(|t| t.pac_collar.clone())
        .unwrap_or_else(PacCollar::standard);

    let collateral_balance = collateral.current_face.amount();
    let schedule = PacSchedule::generate(
        collateral_balance,
        pac_balance,
        collateral.wam,
        collateral.wac,
        collar,
    );

    Some(PacContext {
        schedule: Some(schedule),
        period_index: 0,
        actual_psa: actual_psa_from_model(&collateral.prepayment_model),
    })
}

/// Generate cashflows for the reference tranche.
///
/// Projects collateral cashflows and runs them through the waterfall
/// to determine the reference tranche's cashflows.
pub(crate) fn generate_tranche_cashflows(
    cmo: &AgencyCmo,
    as_of: Date,
    max_periods: Option<u32>,
) -> Result<Vec<TrancheCashflow>> {
    let collateral = resolve_collateral(cmo)?;

    // Generate collateral cashflows
    let collateral_cfs = generate_cashflows(&collateral, as_of, max_periods)?;

    // Build the PAC context once (None for non-PAC deals). The schedule is
    // collateral-derived; only `period_index` advances per period below.
    let mut pac_context = build_pac_context(cmo, &collateral);

    // Create a working copy of the waterfall
    let mut waterfall = cmo.waterfall.clone();

    let mut tranche_cfs = Vec::new();
    let ref_id = &cmo.reference_tranche_id;

    let ref_tranche = waterfall
        .get_tranche(ref_id)
        .ok_or_else(|| finstack_core::Error::Validation(format!("Tranche {} not found", ref_id)))?;

    let is_io = ref_tranche.tranche_type == CmoTrancheType::InterestOnly;

    // Track collateral factor for IO strips
    let original_collateral = collateral.current_face.amount();

    for (period_idx, cf) in collateral_cfs.iter().enumerate() {
        // Run waterfall for this period
        let total_interest = cf.interest;

        // Advance the PAC schedule cursor to this projection period so the
        // PAC tranche draws its scheduled principal for the correct month.
        if let Some(ctx) = pac_context.as_mut() {
            ctx.period_index = period_idx;
        }

        if is_io {
            // IO gets interest based on collateral factor.
            // Use beginning_balance (not ending_balance) because interest accrues
            // on the balance at the start of the period, before principal payments.
            let factor = cf.beginning_balance / original_collateral;
            // We validated ref_id exists at function start, so this should always succeed
            if let Some(io_tranche) = waterfall.get_tranche(ref_id) {
                // Interest conservation: the IO can never receive more than
                // the collateral interest delivered this period, so
                // IO + PO PV stays bounded by collateral PV.
                let io_payment = allocate_io_cashflow(io_tranche, factor).min(cf.interest);

                tranche_cfs.push(TrancheCashflow {
                    payment_date: cf.payment_date,
                    principal: 0.0,
                    scheduled_principal: 0.0,
                    prepayment_principal: 0.0,
                    interest: io_payment,
                    total: io_payment,
                    ending_balance: io_tranche.original_face.amount() * factor,
                });
            }
        } else {
            // Regular waterfall execution. For PAC deals `pac_context` is
            // `Some`, so PAC tranches amortize on their collateral-derived
            // schedule/collar via `allocate_pac_support` instead of falling
            // through to balance-limited sequential allocation.
            let collateral_factor = cf.beginning_balance / original_collateral;
            let result = execute_waterfall_with_principal_breakdown(
                &mut waterfall,
                cf.scheduled_principal,
                cf.prepayment,
                total_interest,
                collateral_factor,
                pac_context.as_ref(),
            );

            // Find allocation for reference tranche
            if let Some(alloc) = result.allocations.iter().find(|a| a.tranche_id == *ref_id) {
                tranche_cfs.push(TrancheCashflow {
                    payment_date: cf.payment_date,
                    principal: alloc.principal,
                    scheduled_principal: alloc.scheduled_principal,
                    prepayment_principal: alloc.prepayment_principal,
                    interest: alloc.interest,
                    total: alloc.principal + alloc.interest,
                    ending_balance: alloc.ending_balance,
                });
            }
        }
    }

    Ok(tranche_cfs)
}

/// Build the canonical reference-tranche schedule used by pricing and providers.
///
pub(crate) fn build_reference_tranche_schedule(
    cmo: &AgencyCmo,
    as_of: Date,
    max_periods: Option<u32>,
) -> Result<CashFlowSchedule> {
    let tranche = cmo.reference_tranche().ok_or_else(|| {
        finstack_core::Error::Validation(format!("Tranche {} not found", cmo.reference_tranche_id))
    })?;
    let tranche_cashflows = generate_tranche_cashflows(cmo, as_of, max_periods)?;
    let mut flows = Vec::with_capacity(tranche_cashflows.len() * 2);

    for cf in tranche_cashflows {
        if cf.interest.abs() > f64::EPSILON {
            flows.push(CashFlow {
                date: cf.payment_date,
                reset_date: None,
                amount: Money::new(cf.interest, tranche.current_face.currency()),
                kind: CFKind::Fixed,
                accrual_factor: 0.0,
                rate: Some(tranche.coupon),
            });
        }
        if cf.scheduled_principal.abs() > f64::EPSILON {
            flows.push(CashFlow {
                date: cf.payment_date,
                reset_date: None,
                amount: Money::new(cf.scheduled_principal, tranche.current_face.currency()),
                kind: CFKind::Amortization,
                accrual_factor: 0.0,
                rate: None,
            });
        }
        if cf.prepayment_principal.abs() > f64::EPSILON {
            flows.push(CashFlow {
                date: cf.payment_date,
                reset_date: None,
                amount: Money::new(cf.prepayment_principal, tranche.current_face.currency()),
                kind: CFKind::PrePayment,
                accrual_factor: 0.0,
                rate: None,
            });
        }
    }

    Ok(crate::cashflow::traits::schedule_from_classified_flows(
        flows,
        DayCount::Thirty360,
        crate::cashflow::traits::ScheduleBuildOpts {
            notional_hint: Some(tranche.current_face),
            meta: Some(CashFlowMeta {
                representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                calendar_ids: Vec::new(),
                facility_limit: None,
                issue_date: Some(cmo.issue_date),
            }),
            ..Default::default()
        },
    ))
}

/// Create assumed collateral for CMO valuation.
fn create_assumed_collateral(cmo: &AgencyCmo) -> Result<AgencyMbsPassthrough> {
    let defaults = embedded_registry()?.cmo_collateral_defaults();
    let total_face = cmo.waterfall.total_current_face();
    let wac = cmo.collateral_wac.unwrap_or(defaults.wac);
    let wam = cmo.collateral_wam.unwrap_or(defaults.wam_months);

    // Standard fee assumptions
    let servicing_fee = defaults.servicing_fee_rate;
    let guarantee_fee = defaults.guarantee_fee_rate;
    let pass_through = wac - servicing_fee - guarantee_fee;

    let maturity = cmo
        .issue_date
        .checked_add(time::Duration::days((wam as i64) * 30))
        .ok_or_else(|| finstack_core::Error::Validation("Invalid maturity".to_string()))?;

    AgencyMbsPassthrough::builder()
        .id(InstrumentId::new(format!("{}-COLLATERAL", cmo.id.as_str())))
        .pool_id(format!("{}-POOL", cmo.deal_name).into())
        .agency(cmo.agency)
        .pool_type(PoolType::Generic)
        .original_face(total_face)
        .current_face(total_face)
        .current_factor(1.0)
        .wac(wac)
        .pass_through_rate(pass_through)
        .servicing_fee_rate(servicing_fee)
        .guarantee_fee_rate(guarantee_fee)
        .wam(wam)
        .issue_date(cmo.issue_date)
        .maturity(maturity)
        .prepayment_model(PrepaymentModelSpec::psa(defaults.psa_multiplier))
        .discount_curve_id(cmo.discount_curve_id.clone())
        .day_count(DayCount::Thirty360)
        .build()
}

/// Price a CMO reference tranche.
///
/// Generates tranche cashflows and discounts them to present value.
pub(crate) fn price_cmo(cmo: &AgencyCmo, market: &MarketContext, as_of: Date) -> Result<Money> {
    let schedule = build_reference_tranche_schedule(cmo, as_of, None)?;
    let currency = cmo
        .reference_tranche()
        .map(|tranche| tranche.current_face.currency())
        .unwrap_or(Currency::USD);

    if schedule.flows.is_empty() {
        return Ok(Money::new(0.0, currency));
    }

    let discount_curve = market.get_discount(&cmo.discount_curve_id)?;
    let dc = discount_curve.day_count();

    let mut pv = 0.0;
    for cf in &schedule.flows {
        let years = dc.year_fraction(as_of, cf.date, DayCountContext::default())?;
        let df = discount_curve.df(years);
        pv += cf.amount.amount() * df;
    }

    Ok(Money::new(pv, currency))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::primitives::CFKind;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use time::Month;

    fn create_test_market(as_of: Date) -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, 0.96),
                (5.0, 0.80),
                (10.0, 0.60),
                (30.0, 0.30),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .expect("valid curve");

        MarketContext::new().insert(disc)
    }

    #[test]
    fn test_generate_tranche_cashflows() {
        let cmo = AgencyCmo::example().expect("AgencyCmo example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");

        let cfs = generate_tranche_cashflows(&cmo, as_of, Some(12)).expect("should generate");

        assert!(!cfs.is_empty());

        // Sequential A tranche should get principal first
        for cf in &cfs {
            // Should have some cashflow
            assert!(cf.total >= 0.0);
        }
    }

    #[test]
    fn test_reference_tranche_schedule_preserves_scheduled_and_prepay_rows() {
        let cmo = AgencyCmo::example().expect("AgencyCmo example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let schedule = build_reference_tranche_schedule(&cmo, as_of, Some(6))
            .expect("reference tranche schedule should build");

        assert!(!schedule.flows.is_empty());
        assert!(schedule.flows.iter().any(|cf| cf.kind == CFKind::Fixed));
        assert!(schedule
            .flows
            .iter()
            .any(|cf| cf.kind == CFKind::Amortization));
        assert!(schedule
            .flows
            .iter()
            .any(|cf| cf.kind == CFKind::PrePayment));
    }

    /// PAC/support schedule classification.
    ///
    /// RE-BLESSED for item 1 (library-self-calculated regression, no external
    /// provenance). Before the PAC context was wired in, the PAC tranche fell
    /// through to balance-limited sequential allocation and therefore received
    /// prepayment principal — this test previously asserted that buggy
    /// behavior (`PrePayment` rows on the PAC reference tranche).
    ///
    /// With the PAC schedule now driving amortization, a PAC tranche *within
    /// its collar* receives only scheduled principal (its collateral-derived
    /// collar schedule) — never prepayment. The correct, fixture-independent
    /// expectations are:
    ///   - the PAC reference schedule has `Amortization` (scheduled) rows;
    ///   - the PAC reference schedule has **no** `PrePayment` rows while the
    ///     pool runs inside the collar.
    #[test]
    fn test_pac_support_reference_schedule_preserves_prepayment_rows() {
        let cmo = AgencyCmo::example_pac_support().expect("PAC/support example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");

        // PAC reference: within the collar it amortizes on its schedule, so it
        // has scheduled-principal (`Amortization`) rows.
        let pac_schedule = build_reference_tranche_schedule(&cmo, as_of, Some(12))
            .expect("PAC schedule should build");
        assert!(
            pac_schedule
                .flows
                .iter()
                .any(|cf| cf.kind == CFKind::Amortization),
            "PAC tranche should receive scheduled principal"
        );
        // The PAC tranche, priced on its schedule, must not show prepayment
        // rows while inside the collar — prepayment variability is diverted to
        // the support tranche. (Under the pre-fix sequential fallback the PAC
        // wrongly carried prepayment rows.)
        assert!(
            !pac_schedule
                .flows
                .iter()
                .any(|cf| cf.kind == CFKind::PrePayment),
            "PAC tranche within collar must not carry prepayment rows"
        );
    }

    #[test]
    fn test_price_cmo() {
        let cmo = AgencyCmo::example().expect("AgencyCmo example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let pv = price_cmo(&cmo, &market, as_of).expect("should price");

        // PV should be positive
        assert!(pv.amount() > 0.0);
    }

    /// Item 1 regression: PAC tranches must amortize on the PAC schedule, not
    /// fall through to balance-limited sequential allocation.
    ///
    /// Before the fix, `generate_tranche_cashflows` called the waterfall with
    /// `pac_context = None`, so `PacSchedule`/`allocate_pac_support` were dead
    /// code and a PAC bond was priced identically to a plain sequential. With
    /// the PAC context wired in, the PAC tranche's per-period principal is
    /// capped by its collateral-derived collar schedule and is therefore
    /// strictly less than what an uncapped sequential front tranche would
    /// receive from the same collateral.
    #[test]
    fn test_pac_tranche_amortizes_on_pac_schedule_not_sequential() {
        let cmo = AgencyCmo::example_pac_support().expect("PAC/support example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");

        // PAC reference tranche cashflows (PAC context active).
        let pac_cfs = generate_tranche_cashflows(&cmo, as_of, Some(24))
            .expect("PAC tranche cashflows should generate");
        assert!(!pac_cfs.is_empty());

        // Build the collateral-derived PAC schedule directly for comparison.
        let collateral = resolve_collateral(&cmo).expect("collateral resolves");
        let pac_tranche = cmo.waterfall.get_tranche("PAC").expect("PAC tranche");
        let schedule = super::PacSchedule::generate(
            collateral.current_face.amount(),
            pac_tranche.current_face.amount(),
            collateral.wam,
            collateral.wac,
            pac_tranche.pac_collar.clone().expect("PAC has a collar"),
        );

        // Within the collar, the PAC tranche's principal each period must not
        // exceed the PAC schedule amount for that period (the defining PAC
        // property). A plain-sequential fallback would hand the PAC the full
        // collateral principal and violate this for early periods.
        let mut total_pac_principal = 0.0;
        for (i, cf) in pac_cfs.iter().enumerate() {
            let scheduled = schedule.scheduled_at(i);
            total_pac_principal += cf.principal;
            assert!(
                cf.principal <= scheduled + 1.0,
                "period {i}: PAC principal {} exceeds PAC schedule {scheduled}; \
                 PAC is being priced as a plain sequential",
                cf.principal
            );
        }

        // And the PAC must actually receive some scheduled principal — i.e.
        // the schedule path is live, not a degenerate all-zero schedule.
        assert!(
            total_pac_principal > 0.0,
            "PAC tranche received no principal over 24 months"
        );
    }

    #[test]
    fn test_price_io_strip() {
        let cmo = AgencyCmo::example_io_po().expect("AgencyCmo IO/PO example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        // Change reference to IO
        let mut io_cmo = cmo.clone();
        io_cmo.reference_tranche_id = "IO".to_string();

        let pv = price_cmo(&io_cmo, &market, as_of).expect("should price");

        // IO should have positive value
        assert!(pv.amount() > 0.0);
    }

    /// Interest conservation (finding 17): the IO strip's interest is capped
    /// at the collateral interest each period, so IO + PO PV cannot exceed
    /// the PV of the collateral's total cashflows.
    #[test]
    fn io_plus_po_pv_bounded_by_collateral_pv() {
        let cmo = AgencyCmo::example_io_po().expect("AgencyCmo IO/PO example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let mut io_cmo = cmo.clone();
        io_cmo.reference_tranche_id = "IO".to_string();
        let io_pv = price_cmo(&io_cmo, &market, as_of).expect("IO prices");

        let mut po_cmo = cmo.clone();
        po_cmo.reference_tranche_id = "PO".to_string();
        let po_pv = price_cmo(&po_cmo, &market, as_of).expect("PO prices");

        // Collateral PV: discount the pool's total investor cashflows with
        // the same curve and day count as the tranche pricer.
        let collateral = resolve_collateral(&cmo).expect("collateral resolves");
        let collateral_cfs =
            generate_cashflows(&collateral, as_of, None).expect("collateral cashflows");
        let curve = market
            .get_discount(&cmo.discount_curve_id)
            .expect("discount curve");
        let dc = curve.day_count();
        let mut collateral_pv = 0.0;
        for cf in &collateral_cfs {
            let years = dc
                .year_fraction(as_of, cf.payment_date, DayCountContext::default())
                .expect("year fraction");
            let total = cf.interest + cf.scheduled_principal + cf.prepayment;
            collateral_pv += total * curve.df(years);
        }

        let combined = io_pv.amount() + po_pv.amount();
        assert!(
            combined <= collateral_pv * (1.0 + 1e-9) + 1e-6,
            "IO ({}) + PO ({}) = {combined} must not exceed collateral PV {collateral_pv}",
            io_pv.amount(),
            po_pv.amount()
        );
    }

    /// The IO strip's notional must amortize with the collateral factor: its
    /// reported ending balance tracks `original_face × factor` and declines
    /// as the pool pays down.
    #[test]
    fn io_notional_amortizes_with_collateral_factor() {
        let cmo = AgencyCmo::example_io_po().expect("AgencyCmo IO/PO example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");

        let mut io_cmo = cmo.clone();
        io_cmo.reference_tranche_id = "IO".to_string();
        let cfs = generate_tranche_cashflows(&io_cmo, as_of, Some(60)).expect("IO cashflows");
        assert!(cfs.len() > 12);

        let original = io_cmo
            .waterfall
            .get_tranche("IO")
            .expect("IO tranche")
            .original_face
            .amount();

        // Balances strictly decrease (positive prepayment + amortization)
        // and stay below the unamortized notional after the first period.
        for window in cfs.windows(2) {
            assert!(
                window[1].ending_balance < window[0].ending_balance,
                "IO notional must amortize with the pool factor: {} -> {}",
                window[0].ending_balance,
                window[1].ending_balance
            );
        }
        let last = cfs.last().expect("non-empty");
        assert!(
            last.ending_balance < original,
            "IO ending balance {} must fall below original notional {original}",
            last.ending_balance
        );
    }

    #[test]
    fn test_price_po_strip() {
        let cmo = AgencyCmo::example_io_po().expect("AgencyCmo IO/PO example is valid");
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        // Change reference to PO
        let mut po_cmo = cmo.clone();
        po_cmo.reference_tranche_id = "PO".to_string();

        let pv = price_cmo(&po_cmo, &market, as_of).expect("should price");

        // PO should have positive value
        assert!(pv.amount() > 0.0);
    }
}
