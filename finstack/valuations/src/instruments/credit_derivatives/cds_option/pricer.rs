//! Bloomberg CDSO pricer for [`CDSOption`].
//!
//! All pricing — NPV, par spread, Greeks, implied volatility — flows
//! through the [`bloomberg_quadrature`](super::bloomberg_quadrature) module
//! by way of `npv()` (bump-and-reprice for sensitivities). The legacy
//! Black-on-spreads implementation was removed alongside the field clean-up
//! when the Bloomberg model became the default pricer.
//!
//! # References
//!
//! - Bloomberg L.P. Quantitative Analytics. *Pricing Credit Index Options.*
//!   DOCS 2055833 ⟨GO⟩, March 2012.
//! - Bloomberg L.P. Quantitative Analytics. *The Bloomberg CDS Model.*
//!   DOCS 2057273 ⟨GO⟩, August 2024.

use super::bloomberg_quadrature;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds::{
    CdsValuationConvention, CreditDefaultSwap, PayReceive,
};
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::solver::{BrentSolver, Solver};
use finstack_core::money::Money;
use finstack_core::Result;
use rust_decimal::Decimal;

/// Stateless namespace for the Bloomberg CDSO pricer's public surface.
///
/// All entry points delegate to [`bloomberg_quadrature::npv`] (or
/// `forward_par_at_expiry`) — the methods here add bump-and-reprice
/// scaffolding for Greeks and implied vol.
pub(crate) struct CDSOptionPricer;

impl CDSOptionPricer {
    // ---------------------------------------------------------------- NPV

    /// Price the CDS option at `as_of` under the Bloomberg CDSO numerical
    /// quadrature model.
    #[tracing::instrument(skip(self, option, curves), fields(instrument_id = %option.id, as_of = %as_of))]
    pub(crate) fn npv(
        &self,
        option: &CDSOption,
        curves: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<Money> {
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
        &self,
        option: &CDSOption,
        curves: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<f64> {
        let cds = synthetic_underlying_cds(option, as_of)?;
        bloomberg_quadrature::forward_par_at_expiry_bp(option, &cds, curves, as_of)
    }

    // ---------------------------------------------------------------- Greeks (bump & reprice)

    /// Bloomberg CDSO Δ: ratio of the change in option premium to the
    /// change in underlying-swap principal value when the index credit
    /// curve is bumped by `+1 bp`. Returned as a unit-less ratio (multiply
    /// by 100 for the displayed percentage).
    #[tracing::instrument(skip(self, option, curves), fields(instrument_id = %option.id, as_of = %as_of))]
    pub(crate) fn delta(
        &self,
        option: &CDSOption,
        curves: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<f64> {
        bumped_delta(option, curves, as_of, 1.0)
    }

    /// Bloomberg CDSO Γ: change in [`Self::delta`] when the credit curve
    /// is bumped by `+10 bp` rather than `+1 bp`. Returned as a unit-less
    /// number (multiply by 100 for the displayed percentage).
    pub(crate) fn gamma(
        &self,
        option: &CDSOption,
        curves: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<f64> {
        self.gamma_bbg_screen(option, curves, as_of)
    }

    /// Bloomberg-screen gamma: difference between 10bp-bumped and 1bp-bumped deltas.
    pub(crate) fn gamma_bbg_screen(
        &self,
        option: &CDSOption,
        curves: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<f64> {
        // Δ at +1bp and at +10bp; (Δ_10bp − Δ_1bp) is Bloomberg's gamma.
        let d_low = bumped_delta(option, curves, as_of, 1.0)?;
        let d_high = bumped_delta(option, curves, as_of, 10.0)?;
        Ok(d_high - d_low)
    }

    /// Bloomberg CDSO Vega(1%): change in option premium for a `+1`
    /// vol-point increase in implied volatility.
    pub(crate) fn vega(
        &self,
        option: &CDSOption,
        curves: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<f64> {
        let base_sigma = resolve_sigma(option, curves, as_of)?;
        let bumped = base_sigma + 0.01;
        let cds = synthetic_underlying_cds(option, as_of)?;
        let pv_base = bloomberg_quadrature::npv(option, &cds, curves, base_sigma, as_of)?.amount();
        let pv_bumped = bloomberg_quadrature::npv(option, &cds, curves, bumped, as_of)?.amount();
        Ok(pv_bumped - pv_base)
    }

    /// Bloomberg CDSO θ: change in option premium for a one-day decrease
    /// in option maturity.
    ///
    /// Implements DOCS 2055833 §2.5 verbatim — "shorten the exercise time
    /// `t_e` by `1/365.25`" — while retaining the same calibrated forward
    /// price and lognormal mean. `df_te` and `sp_te` are NOT advanced; the
    /// shift is purely on the integrand's `t_expiry` argument.
    ///
    /// Empirical match against the CDSO screen on `cdx_ig_46_payer_atm_jun26`
    /// is materially closer under pure-T-shift (~−$1,479 / −$1,461 after the
    /// Phase 2 bootstrap migration) than under the alternative as-of-shift
    /// formulation (~−$1,705) vs Bloomberg's −$1,499.93. The pure-T-shift
    /// path is the one tested by that golden's $40 absolute tolerance.
    #[tracing::instrument(skip(self, option, curves), fields(instrument_id = %option.id, as_of = %as_of))]
    pub(crate) fn theta(
        &self,
        option: &CDSOption,
        curves: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<f64> {
        option.validate_supported_configuration()?;
        let sigma = resolve_sigma(option, curves, as_of)?;
        let cds = synthetic_underlying_cds(option, as_of)?;
        bloomberg_quadrature::theta(option, &cds, curves, sigma, as_of)
    }

    // ---------------------------------------------------------------- Implied volatility

    /// Solve for the implied lognormal volatility `σ` that reproduces
    /// `target_price` under the Bloomberg CDSO pricer. Brent root-finding
    /// in log-σ space (so `σ > 0` is enforced).
    #[tracing::instrument(skip(self, option, curves), fields(instrument_id = %option.id, as_of = %as_of, target_price))]
    pub(crate) fn implied_vol(
        &self,
        option: &CDSOption,
        curves: &MarketContext,
        as_of: finstack_core::dates::Date,
        target_price: f64,
        initial_guess: Option<f64>,
    ) -> Result<f64> {
        if target_price < 0.0 {
            return Err(finstack_core::Error::Validation(
                "implied vol target price must be non-negative".to_string(),
            ));
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
        let captured: std::cell::RefCell<Option<finstack_core::Error>> =
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
            return Err(finstack_core::Error::Validation(format!(
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
}

// =====================================================================
// Helpers
// =====================================================================

/// Resolve the lognormal spread vol `σ` for the option, preferring the
/// instrument-level `pricing_overrides.market_quotes.implied_volatility`
/// override, falling back to the volatility surface lookup at
/// `(t_expiry, strike)`.
fn resolve_sigma(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_core::dates::Date,
) -> Result<f64> {
    use rust_decimal::prelude::ToPrimitive;
    let t = option.time_to_expiry(as_of)?;
    let strike = option.strike.to_f64().unwrap_or(0.0);
    let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
        &option.pricing_overrides.market_quotes,
        curves,
        option.vol_surface_id.as_str(),
        t,
        strike,
    )?;
    if sigma > super::types::MAX_IMPLIED_VOL {
        return Err(finstack_core::Error::Validation(format!(
            "implied_volatility {} exceeds maximum {}",
            sigma,
            super::types::MAX_IMPLIED_VOL
        )));
    }
    Ok(sigma)
}

/// Bump the index credit curve by `bump_bp` parallel, re-price both the
/// option (under [`bloomberg_quadrature`]) and the underlying CDS, and
/// return Bloomberg's CDSO Δ:
///
/// ```text
/// Δ = (option_pv_bumped − option_pv_base) / (cds_pv_bumped − cds_pv_base)
/// ```
///
/// # Bootstrap symmetry
///
/// Both legs of the forward-difference go through
/// [`bump_hazard_spreads`] — the base leg with a zero parallel bump, the
/// bumped leg with `+bump_bp`. Pricing both NPVs against rebootstrapped
/// hazard curves removes any "zero-bump bootstrap drift" (the small PV
/// gap between the as-supplied hazard knots and what the bootstrap
/// would solve from the same par spreads) from the numerator and
/// denominator. This is the same symmetric-rebootstrap convention used
/// by the canonical CS01 helper (`metrics/sensitivities/cs01.rs`) and
/// it is what Bloomberg's CDSO Spread DV01 path also does. Without
/// this, the option Δ silently absorbs the bootstrap-vs-snapshot
/// methodology delta on top of the genuine credit sensitivity. The
/// option's reported NPV (returned by [`CDSOptionPricer::npv`]) still
/// uses the as-supplied `curves`; only Δ/Γ are computed under
/// rebootstrapped legs.
fn bumped_delta(
    option: &CDSOption,
    curves: &MarketContext,
    as_of: finstack_core::dates::Date,
    bump_bp: f64,
) -> Result<f64> {
    use crate::calibration::bumps::{bump_hazard_spreads, BumpRequest};

    let pricer = CDSOptionPricer;

    let hazard = curves.get_hazard(&option.credit_curve_id)?;
    let discount_id = Some(&option.discount_curve_id);

    // Symmetric rebootstrap: both base (zero bump) and bumped legs are
    // re-bootstrapped from the same par-spread points, so the forward
    // difference captures only the genuine credit sensitivity rather than
    // a mixture of bootstrap drift + spread sensitivity.
    let base_hazard = bump_hazard_spreads(
        hazard.as_ref(),
        curves,
        &BumpRequest::Parallel(0.0),
        discount_id,
    )?;
    let bumped_hazard = bump_hazard_spreads(
        hazard.as_ref(),
        curves,
        &BumpRequest::Parallel(bump_bp),
        discount_id,
    )?;
    let base_curves = (*curves).clone().insert(base_hazard);
    let bumped_curves = (*curves).clone().insert(bumped_hazard);

    let pv_base = pricer.npv(option, &base_curves, as_of)?.amount();
    let pv_bumped = pricer.npv(option, &bumped_curves, as_of)?.amount();

    // Underlying-swap PV change uses the synthetic forward CDS, also priced
    // on the rebootstrapped legs so the ratio's denominator and numerator
    // share the same bootstrap methodology.
    let cds = synthetic_underlying_cds(option, as_of)?;
    let cds_delta = cds_with_bloomberg_protection_end_extension(&cds);
    let cds_pricer = crate::instruments::credit_derivatives::cds::pricer::CDSPricer::new();
    let disc = curves.get_discount(&option.discount_curve_id)?;
    let base_hazard_arc = base_curves.get_hazard(&option.credit_curve_id)?;
    let bumped_hazard_arc = bumped_curves.get_hazard(&option.credit_curve_id)?;

    let cds_pv_base =
        cds_pricer.npv_full(&cds_delta, disc.as_ref(), base_hazard_arc.as_ref(), as_of)?;
    let cds_pv_bumped =
        cds_pricer.npv_full(&cds_delta, disc.as_ref(), bumped_hazard_arc.as_ref(), as_of)?;

    let denom = cds_pv_bumped - cds_pv_base;
    if denom.abs() < 1e-12 {
        return Err(finstack_core::Error::Validation(format!(
            "degenerate CDS option delta denominator: id={}, bump_bp={}, cds_pv_base={cds_pv_base:.6}, cds_pv_bumped={cds_pv_bumped:.6}",
            option.id, bump_bp
        )));
    }
    Ok((pv_bumped - pv_base) / denom)
}

/// Build the synthetic underlying CDS that backs the option's forward
/// premium-leg risky annuity and protection-PV calculations. The synthetic
/// CDS uses Bloomberg CDSW conventions for the underlying (BloombergCdswClean
/// valuation convention, adjusted-to-adjusted accruals, +1-day inclusive on
/// the final ACT/360 period).
pub(crate) fn synthetic_underlying_cds(
    option: &CDSOption,
    as_of: finstack_core::dates::Date,
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
        PayReceive::PayFixed,
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
    cds.pricing_overrides.model_config = option.pricing_overrides.model_config.clone();
    cds.valuation_convention = CdsValuationConvention::BloombergCdswClean;
    Ok(cds)
}

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
        as_of: finstack_core::dates::Date,
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

        let pv = CDSOptionPricer.npv(option, market, as_of).map_err(|e| {
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
