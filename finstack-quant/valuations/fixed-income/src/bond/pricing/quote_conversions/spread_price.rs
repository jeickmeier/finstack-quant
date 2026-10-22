use super::compute::clear_price_driving_overrides;
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::bond::{Bond, CashflowSpec};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_core::math::z_spread_discount_factor;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::pricer::ModelKey;

/// Price from Z-spread added to zero rates in the bond's compounding convention.
///
/// # Settlement origin
///
/// `as_of` is the **valuation/trade date**. The Z-spread is, by market
/// convention, a settlement-anchored quantity: [`ZSpreadCalculator`] solves it
/// with discounting and year-fractions measured from the bond's settlement
/// (`quote_date`), not from `as_of`. This inverter mirrors that exactly — it
/// derives the same `quote_date` internally via `QuoteDateContext` and
/// anchors all discounting there. As a result the documented round-trip
///
/// ```text
/// price_from_z_spread(bond, market, as_of, ZSpreadCalculator.solve(...)) == dirty
/// ```
///
/// holds for bullet bonds and for callable/putable bonds with a quoted clean
/// price that selects an explicit yield-to-worst workout path. An
/// option-bearing bond without that quoted workout price is rejected; use OAS
/// for model-based optional pricing. Bonds with a non-zero `settlement_days`
/// lag (`quote_date != as_of`) remain settlement-anchored. Callers must pass
/// the valuation date as `as_of`; workout selection and settlement are handled
/// here.
///
/// [`ZSpreadCalculator`]: crate::bond::ZSpreadCalculator
///
/// # Arguments
///
/// * `bond` - Bond whose future pricing cashflows, settlement convention, and
///   z-spread compounding frequency are used.
/// * `market` - Market context supplying the bond discount curve and schedule
///   dependencies.
/// * `as_of` - Valuation/trade date; the helper derives settlement internally.
/// * `z` - Annual z-spread as a decimal zero-rate shift under the bond's
///   contractual compounding convention.
///
/// # Errors
///
/// Returns an error when required curves or cashflows are unavailable, or
/// when an option-bearing bond has no quoted clean price from which to select
/// an explicit workout path.
pub fn price_from_z_spread(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    z: f64,
) -> finstack_quant_core::Result<f64> {
    BondZSpreadPricingKernel::new(bond, market, as_of)?.price(z)
}

/// Price from Option-Adjusted Spread using an explicit bond model.
///
/// The public API takes **decimal spread units** (`oas_decimal`), where
/// `0.01` corresponds to **100 basis points**. Internally, the tree
/// pricer continues to work in basis points for compatibility, so we
/// convert:
///
/// - `oas_bp = oas_decimal * 10_000.0`
///
/// This keeps all bond spread-style metrics on a consistent decimal
/// convention at the API surface while preserving existing internal
/// tree semantics.
///
/// # Arguments
///
/// * `bond` - Bond whose embedded tree-pricing configuration and contractual
///   cashflows are used for OAS valuation.
/// * `market` - Market context supplying the discount curve and tree inputs.
/// * `as_of` - Valuation date supplied to the short-rate tree pricer.
/// * `model` - Caller-selected bond model. `discounting` and `hazard_rate`
///   require a non-callable bond; `tree` and `rates_credit` value embedded
///   rights under their respective factor families.
/// * `oas_decimal` - Option-adjusted spread as a decimal, such as `0.01` for
///   100 basis points.
///
/// Deterministic return floors are first lowered into their daily effective
/// call schedule. Stochastic rates-credit pricing retains the floor
/// specification because its required redemption depends on the simulated
/// distribution path.
pub fn price_from_oas(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    model: ModelKey,
    oas_decimal: f64,
) -> finstack_quant_core::Result<f64> {
    Ok(bond
        .price_at_oas_for_model_outcome(model, market, as_of, oas_decimal)?
        .amount)
}

/// Price from Discount Margin for FRNs by adding DM (decimal) to the **discount rate**.
///
/// Cashflows are projected **unchanged** at the contractual quoted margin
/// (forward index + `spread_bp` from `FloatingCouponSpec`); the discount
/// margin is then applied as a constant spread on the discount side,
/// following the standard definition (Fabozzi; Bloomberg YAS): PV is
/// strictly **decreasing** in DM, and an FRN priced at par on a flat,
/// consistent curve has DM equal to its quoted margin.
///
/// The discounting mechanics are identical to [`price_from_z_spread`]: the
/// periodically-compounded zero rate is derived from the bond's discount
/// curve and each flow is re-discounted at `rate + dm` (see
/// `z_spread_discount_factor`). This is therefore a **curve DM** — a spread
/// over the bond's *discount* curve. If the discount curve differs from the
/// projection index curve, the solved DM includes that discount/projection
/// basis.
///
/// This helper prices against the model PV, independent of any
/// price-from-quote override on the bond. It is used by the DM metric solver
/// that seeks a DM reproducing a quoted price, so it must not short-circuit
/// via the quote.
///
/// # Arguments
///
/// * `bond` - Floating-rate bond whose contractual cashflows are projected at
///   the quoted margin and re-discounted at the shifted rate.
/// * `market` - Market context supplying discounting and floating-rate reset
///   data.
/// * `as_of` - Valuation/trade date used for projection. Settlement is derived
///   internally once, and the returned dirty price is valued at settlement.
/// * `dm` - Annual discount margin as a decimal added to the discount rate
///   (`0.01` = 100 bp).
pub fn price_from_dm(
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    dm: f64,
) -> finstack_quant_core::Result<f64> {
    let mut b = bond.clone();
    clear_price_driving_overrides(&mut b);

    // DM discounting semantics apply only to bonds with floating coupons
    // (plain FRNs and amortizing floaters).
    if !b.has_floating_coupons() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "discount margin pricing requires a floating-rate bond; bond '{}' has no floating coupons",
            b.id.as_str()
        )));
    }
    // Coupons stay at the contractual quoted margin; the DM shifts the
    // discount rate via the shared Z-spread discounting mechanics.
    price_from_z_spread(&b, market, as_of, dm)
}

/// Price from market asset swap spread (decimal) using the same
/// approximation as `AssetSwapMarketCalculator` for non-custom,
/// fixed-rate bonds:
///
/// `ASW_mkt = [(coupon - par_rate) * fixed_annuity + (1.0 - price_pct)] / float_annuity`
///
/// where `price_pct = dirty / notional` and both the fixed-annuity-weighted
/// running term and the upfront are amortized over the floating-leg annuity
/// (par-par derivation). Without a forward curve the floating leg is proxied
/// on the fixed-leg schedule with the discount curve's day count. Inverting:
///
/// `price_pct = 1.0 + (coupon - par_rate) * fixed_annuity - ASW_mkt * float_annuity`.
pub(super) fn price_from_asw_market(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    asw_market: f64,
) -> Result<f64> {
    if bond.custom_cashflows.is_some() || !matches!(bond.cashflow_spec, CashflowSpec::Fixed(_)) {
        return Err(finstack_quant_core::Error::Validation(
            "ASW market quotes require a plain fixed-coupon bond without custom cashflows"
                .to_string(),
        ));
    }
    let (coupon_pv, float_pv, float_annuity) =
        super::annuity::settlement_asw_components(bond, curves, as_of)?;
    Ok((1.0 + coupon_pv - float_pv - asw_market * float_annuity) * bond.notional.amount())
}

/// Yield-to-worst at the quoted clean price and the cashflow path it selects.
///
/// Both are measured from the bond's quote/settlement date.
#[derive(Debug, Clone)]
pub(crate) struct WorkoutPath {
    /// Yield-to-worst (Street compounding) at the quoted clean price.
    pub(crate) yield_rate: f64,
    /// Holder cashflows of the worst exit path.
    pub(crate) flows: Vec<(Date, Money)>,
}

/// Workout path of a bond with exercise rights at its quoted clean price.
///
/// Returns `None` when the effective bond (return floor lowered into its call
/// schedule) has no call/put options or the bond carries no quoted clean
/// price.
///
/// # Arguments
///
/// * `bond` - Bond whose quoted clean price and exercise schedule are used.
/// * `curves` - Market context for the cashflow schedule and floor lowering.
/// * `as_of` - Valuation date; the quote date is derived from it.
/// * `flows` - Holder cashflows entitled at the quote date.
pub(crate) fn quoted_workout_path(
    bond: &Bond,
    curves: &MarketContext,
    as_of: Date,
    flows: &[(Date, Money)],
) -> Result<Option<WorkoutPath>> {
    let effective_bond = bond.effective_for_pricing(curves, as_of)?;
    if !effective_bond
        .call_put
        .as_ref()
        .is_some_and(|cp| cp.has_options())
    {
        return Ok(None);
    }

    let Some(clean_px) = bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct
    else {
        return Ok(None);
    };

    let quote_ctx = QuoteDateContext::new(bond, curves, as_of)?;
    let dirty_now = Money::new(
        quote_ctx.dirty_from_clean_pct(clean_px, bond.notional.amount()),
        bond.notional.currency(),
    )?;
    let schedule = effective_bond.full_cashflow_schedule(curves)?;
    let (yield_rate, flows) = super::solve_ytw_from_flows(
        &effective_bond,
        curves,
        flows,
        quote_ctx.quote_date,
        dirty_now,
        &schedule,
    )?;

    Ok(Some(WorkoutPath { yield_rate, flows }))
}

/// Maturity-aware initial Brent bracket half-width, in decimal units.
///
/// The bracket scales from `base_bracket_bp` at zero maturity to twice that
/// at 30 years and beyond (`base × (1 + min(years/30, 1))`), capped at
/// `max_bracket_bp`. Years are measured ACT/365F from `as_of` to maturity;
/// this is a numerical heuristic, independent of the coupon day count.
///
/// # Arguments
///
/// * `bond` - Bond whose maturity sets the horizon.
/// * `as_of` - Quote/settlement date the horizon starts from; on or after
///   maturity the base bracket is returned.
/// * `base_bracket_bp` - Half-width in basis points for a zero-length horizon.
/// * `max_bracket_bp` - Maximum half-width in basis points after scaling.
fn maturity_scaled_bracket(
    bond: &Bond,
    as_of: Date,
    base_bracket_bp: f64,
    max_bracket_bp: f64,
) -> finstack_quant_core::Result<f64> {
    if as_of >= bond.maturity {
        return Ok(base_bracket_bp / 10_000.0);
    }
    let years = finstack_quant_core::dates::DayCount::Act365F
        .year_fraction(as_of, bond.maturity, DayCountContext::default())?
        .max(0.0);
    let maturity_scale = 1.0 + (years / 30.0).min(1.0);
    Ok((base_bracket_bp * maturity_scale).min(max_bracket_bp) / 10_000.0)
}

/// Z-spread repricing kernel: the bond's spread cashflows (the quoted workout
/// path for option-bearing bonds) with their year fractions and base discount
/// factors from the quote date, cached once for repeated spread repricing.
pub(crate) struct BondZSpreadPricingKernel {
    pub(crate) quote_date: Date,
    cached_flows: Vec<(f64, f64, f64)>,
    compounds_per_year: f64,
}

impl BondZSpreadPricingKernel {
    pub(crate) fn new(
        bond: &Bond,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Self> {
        let quote_ctx = QuoteDateContext::new(bond, curves, as_of)?;
        let flows = quote_ctx.entitled_flows(bond, curves, as_of)?;
        let quote_date = quote_ctx.quote_date;
        let spread_flows = match quoted_workout_path(bond, curves, as_of, &flows)? {
            Some(workout) => workout.flows,
            None if bond.has_exercise_rights() => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Z-spread pricing for option-bearing bond '{}' requires an explicit quoted clean price to select a workout path; use OAS for model-based optional pricing",
                    bond.id.as_str()
                )));
            }
            None => flows,
        };
        let disc = curves.get_discount(&bond.discount_curve_id)?;
        let day_count = disc.day_count();
        let periods = if day_count == finstack_quant_core::dates::DayCount::Act365L {
            crate::bond::pricing::time_basis::bond_coupon_periods(bond)?
        } else {
            Vec::new()
        };
        let cached_flows = spread_flows
            .iter()
            .filter(|(date, _)| *date > quote_date)
            .map(
                |(date, amount)| -> finstack_quant_core::Result<(f64, f64, f64)> {
                    // Supply the coupon frequency so ICMA-style curve day
                    // counts (which require a reference frequency) don't
                    // hard-error; ignored by ACT/30-360 conventions.
                    let t = if day_count == finstack_quant_core::dates::DayCount::Act365L {
                        crate::bond::pricing::time_basis::act365l_year_fraction(
                            bond.cashflow_spec.frequency(),
                            &periods,
                            quote_date,
                            *date,
                        )?
                    } else {
                        day_count.year_fraction(
                            quote_date,
                            *date,
                            DayCountContext {
                                frequency: Some(bond.cashflow_spec.frequency()),
                                coupon_period:
                                    finstack_quant_valuations_core::instruments::pricing::coupon_period::icma_reference_period(
                                        day_count,
                                        bond.cashflow_spec.frequency(),
                                        spread_flows.iter().map(|(d, _)| *d),
                                        quote_date,
                                    ),
                                ..DayCountContext::default()
                            },
                        )?
                    };
                    let df_base = disc.df_between_dates(quote_date, *date)?;
                    Ok((t, df_base, amount.amount()))
                },
            )
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        Ok(Self {
            quote_date,
            cached_flows,
            compounds_per_year: bond.cashflow_spec.frequency().compounding_frequency(),
        })
    }

    pub(crate) fn price(&self, z: f64) -> finstack_quant_core::Result<f64> {
        let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
        for (t, df_base, amount) in &self.cached_flows {
            let df_z = z_spread_discount_factor(*df_base, *t, z, self.compounds_per_year)?;
            pv.add(*amount * df_z);
        }
        Ok(pv.total())
    }
}

/// Solve `kernel.price(s) = target` for the additive spread `s` (decimal).
///
/// Brent starts at `0.0` with a maturity-aware initial bracket (see
/// [`maturity_scaled_bracket`]). A kernel pricing failure (a non-positive
/// discount factor or compounding denominator at a deeply negative spread)
/// aborts the solve and is returned as-is rather than steering the bracket.
///
/// # Arguments
///
/// * `bond` - Bond whose maturity scales the initial bracket.
/// * `kernel` - Spread repricing kernel; its PV is decreasing in the spread.
/// * `target` - Dirty target value in currency at the kernel's quote date.
/// * `bracket_from` - Date from which the bracket's maturity horizon is measured.
/// * `base_bracket_bp` - Bracket half-width in basis points at zero horizon.
/// * `max_bracket_bp` - Cap on the scaled bracket half-width, in basis points.
/// * `tolerance` - Brent tolerance on the spread axis (decimal).
pub(crate) fn solve_kernel_spread(
    bond: &Bond,
    kernel: &BondZSpreadPricingKernel,
    target: f64,
    bracket_from: Date,
    base_bracket_bp: f64,
    max_bracket_bp: f64,
    tolerance: f64,
) -> finstack_quant_core::Result<f64> {
    let bracket = maturity_scaled_bracket(bond, bracket_from, base_bracket_bp, max_bracket_bp)?;
    BrentSolver::new()
        .with_tolerance(tolerance)
        .with_initial_bracket_size(Some(bracket))
        .solve_fallible(|z| Ok(kernel.price(z)? - target), 0.0)
}
