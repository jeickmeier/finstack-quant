use super::compute::clear_price_driving_overrides;
use crate::bond::metrics::price_yield_spread::z_spread::BondZSpreadPricingKernel;
use crate::bond::{Bond, CashflowSpec};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
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
        crate::bond::metrics::price_yield_spread::asw::settlement_asw_components(
            bond, curves, as_of,
        )?;
    Ok((1.0 + coupon_pv - float_pv - asw_market * float_annuity) * bond.notional.amount())
}
