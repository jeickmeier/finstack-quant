//! Bloomberg CDSO pricer for [`CDSOption`].
//!
//! Pricing primitives — NPV, par spread, theta, implied volatility — that
//! flow through the [`bloomberg_quadrature`](super::bloomberg_quadrature)
//! numerical-quadrature engine. Greek metrics (delta, gamma, vega) live
//! alongside their `MetricCalculator` definitions in the metrics module;
//! the `CDSOption::{delta, gamma, vega}` methods are thin pass-throughs
//! to those canonical implementations.
//!
//! # References
//!
//! - Bloomberg L.P. Quantitative Analytics. *Pricing Credit Index Options.*
//!   DOCS 2055833 ⟨GO⟩, March 2012.
//!   `docs/REFERENCES.md#bloomberg-cdso`
//! - Bloomberg L.P. Quantitative Analytics. *The Bloomberg CDS Model.*
//!   DOCS 2057273 ⟨GO⟩, August 2024.
//!   `docs/REFERENCES.md#bloomberg-cds-model`
//! - ISDA CDS Standard Model (Markit, 2009).
//!   `docs/REFERENCES.md#isda-cds-standard-model`

use super::bloomberg_quadrature;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds::{
    CdsValuationConvention, CreditDefaultSwap, PayReceive,
};
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use crate::pricer::expect_inst;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use rust_decimal::Decimal;

/// Price the CDS option at `as_of` under the Bloomberg CDSO numerical
/// quadrature model.
#[tracing::instrument(skip(option, curves), fields(instrument_id = %option.id, as_of = %as_of))]
pub(crate) fn npv(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<Money> {
    ensure_valuation_not_after_expiry(option, as_of)?;
    option.validate_supported_configuration()?;
    let sigma = resolve_sigma(option, curves, as_of)?;
    let cds = synthetic_underlying_cds(option, as_of)?;
    bloomberg_quadrature::npv(option, &cds, curves, sigma, as_of)
}

/// Bloomberg CDSO ATM-Forward spread in basis points — the par spread
/// of the no-knockout forward CDS struck at expiry, on the bootstrapped
/// hazard curve. This is what the CDSO terminal labels *ATM Fwd*.
pub(crate) fn forward_spread_bp(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    ensure_valuation_not_after_expiry(option, as_of)?;
    let cds = synthetic_underlying_cds(option, as_of)?;
    bloomberg_quadrature::forward_par_at_expiry_bp(option, &cds, curves, as_of)
}

/// Bloomberg CDSO θ: change in option premium for a one-day decrease
/// in option maturity.
///
/// Implements DOCS 2055833 §2.5 verbatim — "shorten the exercise time
/// `t_e` by `1/365.25`" — while retaining the same calibrated forward
/// price and lognormal mean. `df_te` and `sp_te` are NOT advanced; the
/// shift is purely on the integrand's `t_expiry` argument.
#[tracing::instrument(skip(option, curves), fields(instrument_id = %option.id, as_of = %as_of))]
pub(crate) fn theta(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    ensure_valuation_not_after_expiry(option, as_of)?;
    option.validate_supported_configuration()?;
    let sigma = resolve_sigma(option, curves, as_of)?;
    let cds = synthetic_underlying_cds(option, as_of)?;
    bloomberg_quadrature::theta(option, &cds, curves, sigma, as_of)
}

fn ensure_valuation_not_after_expiry(
    option: &CDSOption,
    as_of: finstack_quant_core::dates::Date,
) -> Result<()> {
    if as_of > option.expiry {
        return Err(finstack_quant_core::Error::Validation(format!(
            "CDSOption '{}' expired on {}; post-expiry valuation requires explicit exercise and settlement state",
            option.id, option.expiry
        )));
    }
    // A physically-settled option at expiry is at its exercise lifecycle
    // boundary: exercising delivers a CDS position this pricer does not
    // model, so a cash-equivalent number here would be misleading. Fail
    // explicitly instead. Cash-settled options remain valuable at expiry
    // (t = 0 intrinsic).
    if option.settlement == crate::instruments::SettlementType::Physical && as_of >= option.expiry {
        return Err(finstack_quant_core::Error::Validation(format!(
            "CDSOption '{}' is physically settled and its expiry {} is at or \
             before the valuation date; exercise/delivery lifecycle is not \
             modelled, so a cash-equivalent value would be misleading",
            option.id, option.expiry
        )));
    }
    Ok(())
}

/// Solve for the implied lognormal volatility `σ` that reproduces
/// `target_price` under the Bloomberg CDSO pricer. Brent root-finding
/// in log-σ space (so `σ > 0` is enforced).
#[tracing::instrument(skip(option, curves), fields(instrument_id = %option.id, as_of = %as_of, target_price))]
pub(crate) fn implied_vol(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
    target_price: f64,
    initial_guess: Option<f64>,
) -> Result<f64> {
    if !target_price.is_finite() || target_price < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "implied vol target price must be finite and non-negative, got {target_price}"
        )));
    }
    if option.expiry <= as_of {
        return Ok(0.0);
    }
    option.validate_supported_configuration()?;

    let cds = synthetic_underlying_cds(option, as_of)?;
    // `BrentSolver` expects an `FnMut(f64) -> f64` and has no `Result` channel
    // for in-iteration pricing failures. We squirrel away the first NPV error
    // in a `RefCell<Option<Error>>` and return `f64::NAN` to trip Brent's
    // built-in non-finite guard (`solver.rs` rejects NaN endpoints and
    // mid-iteration NaN with its own `SolverConvergenceFailed`). On either
    // path we surface the original (more informative) NPV error after Brent
    // returns; do not "simplify" this pattern without restoring an error
    // propagation channel.
    let captured: std::cell::RefCell<Option<finstack_quant_core::Error>> =
        std::cell::RefCell::new(None);
    let f = |log_sigma: f64| -> f64 {
        let sigma = log_sigma.exp();
        match bloomberg_quadrature::npv(option, &cds, curves, sigma, as_of) {
            Ok(m) => m.amount() - target_price,
            Err(e) => {
                captured.borrow_mut().get_or_insert(e);
                f64::NAN
            }
        }
    };

    let ln_min = 1e-6_f64.ln();
    let ln_max = super::types::MAX_IMPLIED_VOL.ln();
    let f_lo = f(ln_min);
    let f_hi = f(ln_max);
    if let Some(err) = captured.borrow_mut().take() {
        return Err(err);
    }
    if !f_lo.is_finite() || !f_hi.is_finite() || f_lo * f_hi > 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
                "implied vol target outside model bounds: target={target_price}, f(σ_min)={f_lo:.3e}, f(σ_max)={f_hi:.3e}"
            )));
    }

    let initial_log_guess = initial_guess
        .filter(|vol| *vol > 0.0)
        .map(f64::ln)
        .unwrap_or((ln_min + ln_max) * 0.5)
        .clamp(ln_min, ln_max);
    let solver = BrentSolver::new()
        .tolerance(1e-10)
        .bracket_bounds(ln_min, ln_max);
    let log_sigma = solver.solve(f, initial_log_guess)?;
    if let Some(err) = captured.into_inner() {
        return Err(err);
    }
    Ok(log_sigma.exp().max(1e-6))
}

/// Resolve the lognormal forward-spread model vol `σ` for the option under
/// the strict CDS-option volatility contract:
///
/// 1. The instrument-level
///    `pricing_overrides.market_quotes.implied_volatility` override has
///    highest precedence and needs no surface.
/// 2. Otherwise the `VolSurface` under `vol_surface_id` is evaluated with
///    `models::volatility::get_surface_vol(t_expiry, native_strike_coordinate)` — the decimal
///    spread for spread strikes, the percentage clean price (e.g. `107.0`)
///    for clean-price strikes. No clamped fallback: expiry or strike
///    extrapolation is an error.
/// 3. The surface must carry `VolSurfaceAxis::Strike` and
///    `VolQuoteType::BlackLognormal`. Stored values are lognormal
///    forward-spread model vols even when the strike axis is a clean price;
///    a price-point volatility is never interchangeable with a spread
///    volatility, and provider premiums must be inverted to model vol
///    before materializing a surface.
///
/// Enforces the `MAX_IMPLIED_VOL` ceiling on both paths.
pub(crate) fn resolve_sigma(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurfaceAxis};

    let sigma = if let Some(iv) = option
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility
    {
        iv
    } else {
        let t = option.time_to_expiry(as_of)?;
        let strike = option.strike.native_surface_coordinate()?;
        let surface = curves.get_surface(option.vol_surface_id.as_str())?;
        if surface.secondary_axis() != VolSurfaceAxis::Strike {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS option '{}' volatility surface '{}' must use a strike \
                 secondary axis, got {:?}",
                option.id,
                option.vol_surface_id,
                surface.secondary_axis()
            )));
        }
        surface.require_quote_type(VolQuoteType::BlackLognormal)?;
        finstack_quant_models::volatility::get_surface_vol(&surface, t, strike)?
    };
    if sigma > super::types::MAX_IMPLIED_VOL {
        return Err(finstack_quant_core::Error::Validation(format!(
            "implied_volatility {} exceeds maximum {}",
            sigma,
            super::types::MAX_IMPLIED_VOL
        )));
    }
    Ok(sigma)
}

/// Build the synthetic underlying CDS that backs the option's forward
/// premium-leg risky annuity and protection-PV calculations. The synthetic
/// CDS uses Bloomberg CDSW conventions for the underlying (BloombergCdswClean
/// valuation convention, adjusted-to-adjusted accruals, +1-day inclusive on
/// the final ACT/360 period).
#[doc(hidden)]
pub(crate) fn synthetic_underlying_cds(
    option: &CDSOption,
    as_of: finstack_quant_core::dates::Date,
) -> Result<CreditDefaultSwap> {
    // The contractual coupon `c` of the underlying CDS — for CDX it is
    // 100 bp; for single-name SNAC it is the strike spread. Clean-price
    // strikes require an explicit coupon.
    let coupon_decimal = option.effective_underlying_cds_coupon()?;
    let spread_bp = coupon_decimal * Decimal::new(10_000, 0);

    let notional_scale = if option.underlying_is_index {
        option.index_factor.unwrap_or(1.0)
    } else {
        1.0
    };

    let mut cds = CreditDefaultSwap::new_isda(
        option.id.to_owned(),
        Money::new(
            option.notional.amount() * notional_scale,
            option.notional.currency(),
        )?,
        PayReceive::Pay,
        option.underlying_convention,
        spread_bp,
        option.effective_underlying_effective_date(as_of),
        option.cds_maturity,
        option.recovery_rate,
        option.discount_curve_id.to_owned(),
        option.credit_curve_id.to_owned(),
    )?;

    // Bloomberg CDSO ATM Fwd uses Default_Leg(0, T_mat) — the spot
    // protection PV from valuation date to underlying CDS maturity, NOT a
    // forward-start protection leg from option expiry. Per the published
    // CDSO methodology (Bloomberg Help: "Calculating ATM Forward Spread for
    // CDSO"): "Default Leg: Present value of expected loss from the
    // valuation date (today) to the underlying CDS maturity." We therefore
    // leave `protection_effective_date` unset; with `premium.start = prior
    // IMM` (≤ as_of), `protection_start()` returns `premium.start` and
    // `pv_protection_leg` integrates over `[as_of, T_mat]` — i.e., spot
    // protection.
    cds.instrument_pricing_overrides.model_config =
        option.instrument_pricing_overrides.model_config.clone();
    cds.valuation_convention = CdsValuationConvention::BloombergCdswClean;
    Ok(cds)
}

/// Apply the CDSO-scoped `+1-day` inclusive extension to the synthetic
/// CDS's premium-end date so the ACT/360 protection-leg integral matches
/// Bloomberg's inclusive-end convention.
pub(crate) fn cds_with_bloomberg_protection_end_extension(
    cds: &CreditDefaultSwap,
) -> CreditDefaultSwap {
    let mut extended = cds.clone();
    extended.premium.end += time::Duration::days(1);
    extended
}

// Registry pricer adapter

/// Registry adapter that exposes the Bloomberg CDSO pricer to the
/// instrument/model dispatcher.
pub(crate) struct BloombergCdsoPricer;

impl crate::pricer::Pricer for BloombergCdsoPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::CdsOption,
            crate::pricer::ModelKey::BloombergCdso,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        let option =
            expect_inst::<CDSOption>(instrument, crate::pricer::InstrumentType::CdsOption)?;

        let pv = npv(option, market, as_of).map_err(|e| {
            crate::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                crate::pricer::PricingErrorContext::default(),
            )
        })?;

        Ok(
            crate::results::ValuationResult::stamped(option.id(), as_of, pv).with_details(
                crate::results::ValuationDetails::CreditDerivative(
                    crate::results::CreditDerivativeValuationDetails {
                        model_key: crate::pricer::ModelKey::BloombergCdso,
                        integration_method: None,
                    },
                ),
            ),
        )
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::instruments::SettlementType;
    use time::Duration;

    #[test]
    fn post_expiry_valuation_requires_explicit_settlement_state() {
        let option = CDSOption::example().expect("CDS option example");
        let as_of = option.expiry + Duration::days(1);
        let err = ensure_valuation_not_after_expiry(&option, as_of)
            .expect_err("post-expiry valuation must fail closed");
        assert!(err.to_string().contains("exercise and settlement state"));
    }

    #[test]
    fn physical_settlement_fails_at_and_after_exercise_boundary() {
        let mut option = CDSOption::example().expect("CDS option example");
        option.settlement = SettlementType::Physical;

        // Strictly before expiry: allowed.
        ensure_valuation_not_after_expiry(&option, option.expiry - Duration::days(1))
            .expect("pre-expiry physical valuation is supported");

        // At the exercise boundary: rejected (delivery is not modelled).
        let err = ensure_valuation_not_after_expiry(&option, option.expiry)
            .expect_err("physical valuation at expiry must fail closed");
        assert!(err.to_string().contains("physically settled"));

        // Cash settlement at expiry remains valid (t = 0 intrinsic).
        let cash = CDSOption::example().expect("CDS option example");
        ensure_valuation_not_after_expiry(&cash, cash.expiry)
            .expect("cash valuation at expiry is supported");
    }
}

#[cfg(test)]
mod settlement_and_sigma_tests {
    use super::*;
    use crate::instruments::credit_derivatives::cds_option::{CDSOptionParams, CDSOptionStrike};
    use crate::instruments::{CreditParams, OptionType, SettlementType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DateExt};
    use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurface, VolSurfaceAxis};
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use rust_decimal::Decimal;
    use time::macros::date;

    const BP: f64 = 10_000.0;

    fn flat_discount(base: Date) -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03_f64).exp()),
                (5.0, (-0.15_f64).exp()),
                (10.0, (-0.30_f64).exp()),
            ])
            .build()
            .expect("flat discount curve")
    }

    fn flat_hazard(base: Date) -> HazardCurve {
        let hazard_rate = 0.02;
        let par = hazard_rate * BP * 0.6;
        HazardCurve::builder("HZ-SN")
            .base_date(base)
            .recovery_rate(0.4)
            .knots([(1.0, hazard_rate), (5.0, hazard_rate), (10.0, hazard_rate)])
            .par_spreads([(1.0, par), (5.0, par), (10.0, par)])
            .build()
            .expect("flat hazard curve")
    }

    fn market(as_of: Date) -> MarketContext {
        MarketContext::new()
            .insert(flat_discount(as_of))
            .insert(flat_hazard(as_of))
    }

    fn spread_option(as_of: Date, settlement: SettlementType, vol: Option<f64>) -> CDSOption {
        let params = CDSOptionParams::new(
            CDSOptionStrike::Spread(Decimal::new(1, 2)),
            as_of.add_months(12),
            as_of.add_months(60),
            Money::from((10_000_000_i64, Currency::USD)),
            OptionType::Call,
        )
        .expect("valid params")
        .with_settlement(settlement);
        let credit = CreditParams::corporate_standard("SN", "HZ-SN");
        let mut option = CDSOption::new("CDSO-SETTLE", &params, &credit, "USD-OIS", "CDSO-VOL")
            .expect("valid option");
        option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = vol;
        option
    }

    fn price_strike_option(as_of: Date) -> CDSOption {
        let params = CDSOptionParams::new(
            CDSOptionStrike::CleanPricePct(Decimal::new(1070, 1)),
            as_of.add_months(12),
            as_of.add_months(60),
            Money::from((10_000_000_i64, Currency::USD)),
            OptionType::Call,
        )
        .expect("valid params")
        .as_index(1.0)
        .expect("valid factor")
        .with_strike_index_factor(1.0)
        .expect("valid strike factor")
        .with_underlying_cds_coupon(Decimal::new(5, 2));
        let credit = CreditParams::corporate_standard("HY", "HZ-SN");
        CDSOption::new("CDSO-HY-SIGMA", &params, &credit, "USD-OIS", "CDSO-VOL")
            .expect("valid option")
    }

    fn price_surface() -> VolSurface {
        // Clean-price strike axis in percentage points; stored values are
        // lognormal forward-spread model vols.
        VolSurface::builder("CDSO-VOL")
            .expiries(&[0.25, 2.0])
            .strikes(&[100.0, 110.0])
            .row(&[0.30, 0.40])
            .row(&[0.30, 0.40])
            .build()
            .expect("price-coordinate surface")
    }

    fn spread_surface() -> VolSurface {
        VolSurface::builder("CDSO-VOL")
            .expiries(&[0.25, 2.0])
            .strikes(&[0.005, 0.015])
            .row(&[0.32, 0.36])
            .row(&[0.32, 0.36])
            .build()
            .expect("spread-coordinate surface")
    }

    #[test]
    fn cash_and_physical_pre_expiry_npvs_match() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of);
        let cash = spread_option(as_of, SettlementType::Cash, Some(0.35));
        let physical = spread_option(as_of, SettlementType::Physical, Some(0.35));

        let cash_npv = npv(&cash, &mkt, as_of).expect("cash npv");
        let physical_npv = npv(&physical, &mkt, as_of).expect("physical npv");
        assert_eq!(
            cash_npv.amount(),
            physical_npv.amount(),
            "pre-expiry cash and physical settlement must share the same \
             cash-equivalent model NPV"
        );
        assert!(cash_npv.amount() > 0.0);
    }

    #[test]
    fn price_strike_lookup_uses_percent_coordinate_not_fraction_or_spread() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of).insert_surface(price_surface());
        let option = price_strike_option(as_of);

        // 107.0 interpolates between the 100 → 0.30 and 110 → 0.40 columns.
        let sigma = resolve_sigma(&option, &mkt, as_of).expect("sigma at 107.0");
        assert!(
            (sigma - 0.37).abs() < 1e-12,
            "expected interpolation at the native 107.0 coordinate, got {sigma}"
        );

        // A spread-struck option against the SAME price-coordinate surface
        // queries 0.01 — far outside [100, 110] — and must fail rather than
        // clamp, proving the native coordinate is used.
        let spread = spread_option(as_of, SettlementType::Cash, None);
        let err = resolve_sigma(&spread, &mkt, as_of)
            .expect_err("decimal spread coordinate must be out of bounds");
        let msg = err.to_string();
        assert!(
            msg.contains("0.01") || msg.to_lowercase().contains("out of"),
            "error should reflect the out-of-bounds strike lookup: {msg}"
        );
    }

    #[test]
    fn spread_strike_lookup_uses_decimal_spread_coordinate() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of).insert_surface(spread_surface());
        let option = spread_option(as_of, SettlementType::Cash, None);
        let sigma = resolve_sigma(&option, &mkt, as_of).expect("sigma at 0.01");
        assert!(
            (sigma - 0.34).abs() < 1e-12,
            "expected interpolation at the 0.01 spread coordinate, got {sigma}"
        );
    }

    #[test]
    fn strict_surface_contract_rejects_wrong_axis_quote_type_and_missing_surface() {
        let as_of = date!(2025 - 01 - 01);
        let option = price_strike_option(as_of);

        // Missing surface.
        let err =
            resolve_sigma(&option, &market(as_of), as_of).expect_err("missing surface must fail");
        assert!(!err.to_string().is_empty());

        // Wrong secondary axis.
        let tenor_surface = VolSurface::builder("CDSO-VOL")
            .expiries(&[0.25, 2.0])
            .strikes(&[100.0, 110.0])
            .row(&[0.30, 0.40])
            .row(&[0.30, 0.40])
            .secondary_axis(VolSurfaceAxis::Tenor)
            .build()
            .expect("tenor surface");
        let err = resolve_sigma(&option, &market(as_of).insert_surface(tenor_surface), as_of)
            .expect_err("tenor axis must fail");
        assert!(err.to_string().contains("strike"), "{err}");

        // Wrong quote type.
        let normal_surface = VolSurface::builder("CDSO-VOL")
            .expiries(&[0.25, 2.0])
            .strikes(&[100.0, 110.0])
            .row(&[0.30, 0.40])
            .row(&[0.30, 0.40])
            .quote_type(VolQuoteType::Normal)
            .build()
            .expect("normal surface");
        let err = resolve_sigma(
            &option,
            &market(as_of).insert_surface(normal_surface),
            as_of,
        )
        .expect_err("normal quote type must fail");
        assert!(
            err.to_string().to_lowercase().contains("lognormal"),
            "{err}"
        );

        // Out-of-bounds expiry (surface stops before the option expiry).
        let short_surface = VolSurface::builder("CDSO-VOL")
            .expiries(&[0.05, 0.10])
            .strikes(&[100.0, 110.0])
            .row(&[0.30, 0.40])
            .row(&[0.30, 0.40])
            .build()
            .expect("short surface");
        resolve_sigma(&option, &market(as_of).insert_surface(short_surface), as_of)
            .expect_err("expiry extrapolation must fail");
    }

    #[test]
    fn explicit_implied_vol_bypasses_surface_lookup() {
        let as_of = date!(2025 - 01 - 01);
        // No surface registered at all: the override must be sufficient.
        let option = spread_option(as_of, SettlementType::Cash, Some(0.31));
        let sigma =
            resolve_sigma(&option, &market(as_of), as_of).expect("override needs no surface");
        assert_eq!(sigma, 0.31);
    }
}
