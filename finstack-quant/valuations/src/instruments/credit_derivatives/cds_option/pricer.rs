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
//! - Bloomberg L.P. Quantitative Analytics. *The Bloomberg CDS Model.*
//!   DOCS 2057273 ⟨GO⟩, August 2024.

use super::bloomberg_quadrature;
use crate::instruments::common_impl::numeric::decimal_to_f64;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds::{
    CdsValuationConvention, CreditDefaultSwap, PayReceive,
};
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use rust_decimal::Decimal;

// ---------------------------------------------------------------- NPV

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

// ---------------------------------------------------------------- Par spread

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

// ---------------------------------------------------------------- Theta

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
    Ok(())
}

// ---------------------------------------------------------------- Implied volatility

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

// =====================================================================
// Helpers
// =====================================================================

/// Resolve the lognormal spread vol `σ` for the option, preferring the
/// instrument-level `pricing_overrides.market_quotes.implied_volatility`
/// override, falling back to the volatility surface lookup at
/// `(t_expiry, strike)`. Enforces the `MAX_IMPLIED_VOL` ceiling.
pub(crate) fn resolve_sigma(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    let t = option.time_to_expiry(as_of)?;
    let strike = decimal_to_f64(option.strike, "strike")?;
    let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
        &option.instrument_pricing_overrides.market_quotes,
        curves,
        option.vol_surface_id.as_str(),
        t,
        strike,
    )?;
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
pub fn synthetic_underlying_cds(
    option: &CDSOption,
    as_of: finstack_quant_core::dates::Date,
) -> Result<CreditDefaultSwap> {
    // The contractual coupon `c` of the underlying CDS — for CDX it is
    // 100 bp; for single-name SNAC it is the strike.
    let coupon_decimal = option.effective_underlying_cds_coupon();
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
        ),
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

// =====================================================================
// Registry pricer adapter
// =====================================================================

/// Registry adapter that exposes the Bloomberg CDSO pricer to the
/// instrument/model dispatcher.
pub(crate) struct BloombergCdsoPricer;

impl crate::pricer::Pricer for BloombergCdsoPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::CDSOption,
            crate::pricer::ModelKey::BloombergCdso,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        let option = instrument
            .as_any()
            .downcast_ref::<CDSOption>()
            .ok_or_else(|| {
                crate::pricer::PricingError::type_mismatch(
                    crate::pricer::InstrumentType::CDSOption,
                    instrument.key(),
                )
            })?;

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
                        model_key: format!("{:?}", crate::pricer::ModelKey::BloombergCdso),
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
    use time::Duration;

    #[test]
    fn post_expiry_valuation_requires_explicit_settlement_state() {
        let option = CDSOption::example().expect("CDS option example");
        let as_of = option.expiry + Duration::days(1);
        let err = ensure_valuation_not_after_expiry(&option, as_of)
            .expect_err("post-expiry valuation must fail closed");
        assert!(err.to_string().contains("exercise and settlement state"));
    }
}
