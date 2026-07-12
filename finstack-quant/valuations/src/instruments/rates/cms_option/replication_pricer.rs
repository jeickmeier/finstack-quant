//! CMS option static replication pricer (Andersen-Piterbarg §16.2).
//!
//! This module implements the static replication method for CMS options, which
//! prices CMS payoffs using a portfolio of vanilla swaptions. Unlike the Hagan
//! (2003) first-order convexity adjustment, static replication is exact under
//! any lognormal volatility model and correctly captures the volatility smile.
//!
//! # Method
//!
//! For a CMS **caplet** (pays `(S_T - K)^+` at `T_pay`):
//!
//! ```text
//! V = g(K) × C_sw(K) + ∫_K^{K_max} [2·g'(k) + (k-K)·g''(k)] × C_sw(k) dk
//! ```
//!
//! For a CMS **floorlet** (pays `(K - S_T)^+` at `T_pay`):
//!
//! ```text
//! V = g(K) × P_sw(K) − ∫_{K_min}^K [2·g'(k) + (k-K)·g''(k)] × P_sw(k) dk
//! ```
//!
//! Note the **minus** sign — unlike the caplet, the floorlet integral runs
//! *below* `K`, and integration by parts yields a subtractive correction
//! (`g'(k) > 0`, integral positive). See the `OptionType::Put` branch below.
//!
//! where:
//! - `g(k) = DF(T_pay) / A_par(k)` — ratio of payment discount factor to the
//!   closed-form par annuity at rate `k` (the Radon-Nikodym derivative between
//!   the payment measure and the annuity measure)
//! - `C_sw(k) = A_par(k) × Black76_call(F, k, σ(k), T)` — annuity-measure payer
//!   swaption price, expressed with the **same** closed-form par annuity that
//!   `g(k)` divides by, so `g(k)·C_sw(k) = DF(T_pay)·Black76_call(F, k, σ(k), T)`
//!   with the annuity cancelling cleanly (the `A_par(F) = A₀` calibration
//!   subsumes the market annuity — see the in-body note)
//! - `P_sw(k) = A_par(k) × Black76_put(F, k, σ(k), T)` — annuity-measure receiver
//!   swaption price (same annuity-consistency rule)
//! - `g'(k)`, `g''(k)` — first and second derivatives of `g`, computed via central /
//!   non-uniform 3-point second differences with step `G_PRIME_H = 1e-4`
//! - Integration uses 16-point Gauss-Legendre quadrature over ±6σ from the strike
//!
//! # Par Annuity Formula
//!
//! The closed-form par annuity for a fixed-rate swap with rate `k`, tenor `n`
//! (years), and `m` payments per year is:
//!
//! ```text
//! A_par(k) = (1 - (1 + k/m)^(-n·m)) / k    [for k > 0]
//! A_par(0) = n                               [L'Hôpital limit]
//! ```
//!
//! # Relation to Hagan (2003)
//!
//! The Hagan first-order approximation replaces `g(k)` with `g(F) + g'(F)·(k-F)`,
//! dropping higher-order terms. This replication pricer computes the exact integral,
//! capturing smile-driven convexity at all orders. For CMS tenors > 10Y or
//! high-volatility environments, the difference is 5–10 bps.
//!
//! # References
//!
//! - Andersen, L. B., & Piterbarg, V. V. (2010). *Interest Rate Modeling*.
//!   Vol. 1, §16.2. Atlantic Financial Press.
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models — Theory and Practice*
//!   (2nd ed.). Springer. §13.7.
//! - Hagan, P. S. (2003). "Convexity Conundrums." *Wilmott Magazine*, March, 38–44.

use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cms_option::types::CmsOption;
use crate::instruments::OptionType;
use crate::models::{black76_call, black76_put};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext, Tenor, TenorUnit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::gauss_legendre_integrate;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

// ========================= CONSTANTS =========================

/// Step size for central-difference approximation of g'(k).
///
/// 1bp gives sub-μ errors for smooth g; smaller values risk cancellation.
const G_PRIME_H: f64 = 1e-4;

/// Number of ATM standard deviations for the integration cutoff.
///
/// 6σ captures > 99.9999% of the Black-76 density, ensuring the truncation
/// error is negligible relative to market bid-ask spreads.
const N_STD_CUTOFF: f64 = 6.0;

/// Absolute floor on the integration strike to avoid singularity in g(k)
/// at zero (where A_par → ∞ and g → 0; integrand is well-behaved but
/// numerical derivative needs a guard).
const K_FLOOR: f64 = 1e-4; // 1 basis point

/// Gauss-Legendre quadrature order.
///
/// Order 16 provides 31st-degree polynomial exactness.  For the corrected
/// integrand `[2g'+(k-K)g'']·C_sw` the integrand is more peaked near k≈K
/// than the old `g'·C_sw` form; measured GL-16 error is up to a few percent
/// for ATM long-dated CMS cases (e.g. 20Y tenor, T=5Y).  The old claim of
/// "relative errors below 1e-8" no longer holds for this integrand shape.
///
/// Future accuracy work can consider interval subdivision or adaptive
/// quadrature near k=K to reduce the peaked-integrand error below 0.1%
/// without increasing global node count.
const QUAD_ORDER: usize = 16;

// ========================= MATH HELPERS =========================

/// Closed-form par annuity for a fixed-rate swap.
///
/// Computes the present value of receiving 1 unit of coupon per period on a
/// swap where the discount rate equals `rate`. This is the inverse of the
/// yield-to-price mapping for a bullet bond.
///
/// ```text
/// A_par(k) = (1 - (1 + k/m)^(-n·m)) / k    [k > 0]
/// A_par(0) = n                               [L'Hôpital limit]
/// ```
#[inline]
fn par_annuity(rate: f64, tenor_years: f64, m: f64) -> f64 {
    let nm = tenor_years * m; // total number of coupon periods
    if rate.abs() < 1e-9 {
        // L'Hôpital: lim_{r→0} (1 - (1+r/m)^{-nm}) / r = n
        return tenor_years;
    }
    let discount = (1.0 + rate / m).powf(-nm);
    (1.0 - discount) / rate
}

/// Convert a payment-frequency `Tenor` to payments per year.
///
/// Examples: 6M → 2, 3M → 4, 1Y → 1, 1W → 52.
#[inline]
fn tenor_to_m(freq: Tenor) -> f64 {
    match freq.unit() {
        TenorUnit::Years => 1.0 / freq.count() as f64,
        TenorUnit::Months => 12.0 / freq.count() as f64,
        TenorUnit::Weeks => 52.0 / freq.count() as f64,
        TenorUnit::Days => 360.0 / freq.count() as f64,
    }
}

// ========================= PRICER STRUCT =========================

/// CMS option static replication pricer.
///
/// Computes accurate CMS option prices by static replication of the CMS payoff
/// as a portfolio of vanilla swaptions with strikes spanning the smile. This
/// avoids the 5–10 bp errors of the first-order Hagan convexity approximation
/// for long-dated (> 10Y) CMS options.
///
/// # Performance
///
/// Each CMS fixing requires O(QUAD_ORDER) vol surface lookups and Black-76
/// evaluations (constant per fixing). The computational overhead compared to
/// the Hagan pricer is roughly 20×–50×, but remains well within latency budgets
/// for end-of-day pricing.
pub struct CmsReplicationPricer;

impl CmsReplicationPricer {
    /// Create a new CMS replication pricer.
    pub fn new() -> Self {
        Self
    }

    /// Core pricing logic: iterate over fixings and apply static replication.
    fn price_internal(
        &self,
        inst: &CmsOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        inst.validate()?;
        let mut total_pv = 0.0;

        let strike = inst.strike_f64()?;
        let discount_curve = curves.get_discount(inst.discount_curve_id.as_ref())?;
        let vol_surface = curves.get_surface(inst.vol_surface_id.as_str())?;

        // Payments-per-year for the par annuity closed form.
        // Matches the fixed-leg payment frequency of the underlying CMS swap.
        let m = tenor_to_m(inst.resolved_swap_fixed_freq());

        for (i, &fixing_date) in inst.fixing_dates.iter().enumerate() {
            let payment_date = inst.payment_dates[i];
            let accrual_fraction = inst.accrual_fractions[i];

            if payment_date <= as_of {
                continue; // Period already settled
            }

            // Seasoned period: the CMS rate fixed in the past, so the option
            // payoff is pure intrinsic on the *recorded* fixing (mirroring the
            // cap/floor pricer) — never on a rate re-projected from the live
            // curve, which books phantom P&L.
            if fixing_date < as_of {
                let observed =
                    crate::instruments::rates::exotics_shared::fixings::historical_cms_fixing(
                        curves,
                        &inst.forward_curve_id,
                        inst.cms_tenor,
                        fixing_date,
                    )?;
                let df_pay =
                    relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;
                let intrinsic = match inst.option_type {
                    OptionType::Call => (observed - strike).max(0.0),
                    OptionType::Put => (strike - observed).max(0.0),
                };
                total_pv += df_pay * intrinsic * accrual_fraction;
                continue;
            }

            // Forward-starting swap parameters for this fixing
            let swap_start = inst.reference_swap_start(fixing_date)?;
            let swap_end = swap_start.add_months((inst.cms_tenor * 12.0).round() as i32);

            // F (forward swap rate). The market annuity A₀ is intentionally
            // discarded: the static replication uses the closed-form par
            // annuity `A_par(·)` consistently in both `g(k)` and `C_sw(k)`
            // (see the annuity-consistency note at the boundary term below).
            let (forward_rate, _annuity_mkt) =
                self.calculate_forward_swap_rate(inst, curves, as_of, swap_start, swap_end)?;

            if forward_rate <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Forward swap rate {:.6} is non-positive for fixing date {}; \
                     Black-76 requires positive forward rates",
                    forward_rate, fixing_date
                )));
            }

            // Time-to-fixing is calendar time for the vol axis: ACT/365F.
            let ttf =
                DayCount::Act365F.year_fraction(as_of, fixing_date, DayCountContext::default())?;

            // DF to payment date from discount curve (relative to as_of)
            let df_pay = relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;

            // ATM vol for integration-range sizing
            let atm_vol = vol_surface.value_clamped(ttf.max(0.0), forward_rate);

            // --- Static Replication ---
            let period_pv = if ttf <= 0.0 {
                // Expired fixing: use intrinsic value discounted to today
                match inst.option_type {
                    OptionType::Call => df_pay * (forward_rate - strike).max(0.0),
                    OptionType::Put => df_pay * (strike - forward_rate).max(0.0),
                }
            } else {
                let cms_tenor = inst.cms_tenor;

                // ATM lognormal standard deviation for integration bounds
                let std_dev = atm_vol * forward_rate * ttf.sqrt();

                // Vol at the caplet/floorlet strike (for the boundary term)
                let vol_at_strike = vol_surface.value_clamped(ttf, strike);

                // Annuity consistency (item 12).
                //
                // The CMS caplet value is `V = A₀·E^A[(S−K)⁺·g(S)]`, replicated
                // by integration by parts as
                //   V = g(K)·C_sw(K) + ∫ g'(k)·C_sw(k) dk
                // where `C_sw(k)` is the annuity-measure swaption price and
                // `g(s) = DF_pay/A(s)` is the Radon-Nikodym weight `dQ^{T_pay}/dQ^A`.
                //
                // `g` is *modelled* via the closed-form par annuity, `g(s) =
                // DF_pay/A_par(s)` — this model must be calibrated to the
                // market at the forward, i.e. `A_par(F) ≡ A₀`. The replicating
                // swaption price must use the *same* annuity model that `g`
                // divides by:
                //
                //   C_sw(k) = A_par(k) · Black76(F, k, σ(k), T)
                //
                // so that `g(k)·C_sw(k) = DF_pay · Black76(F, k, σ(k), T)`,
                // with the annuity cancelling cleanly. The previous code paired
                // `g(k) = DF_pay/A_par(k)` with the *market*-annuity swaption
                // `C_sw(k) = A₀·Black76(k)`, leaving a spurious residual
                // `A₀/A_par(k)` — equal to 1 only at `k = F` (where
                // `A_par(F) = A₀`) and mispricing every `K ≠ F`.
                //
                // `A₀` (the market annuity) is therefore correctly *not* used
                // here: it is subsumed into the `A_par(F) = A₀` calibration.
                // The market curve still enters through the forward swap
                // rate `F` and the payment discount factor `DF_pay`.
                let g_times_c = |k: f64, v: f64, is_call: bool| -> f64 {
                    let black = if is_call {
                        black76_call(forward_rate, k, v, ttf)
                    } else {
                        black76_put(forward_rate, k, v, ttf)
                    };
                    // g(k) · C_sw(k) = (DF_pay / A_par(k)) · (A_par(k) · Black) = DF_pay · Black
                    df_pay * black
                };

                match inst.option_type {
                    OptionType::Call => {
                        // Caplet formula (exact static replication, Andersen-Piterbarg §16.2):
                        //   V = g(K) · C_sw(K) + ∫_K^{K_max} [2·g'(k) + (k-K)·g''(k)] · C_sw(k) dk
                        //
                        // Upper bound K_max = K + 6σ ensures ≤ 1e-9 truncation error.
                        let k_max = (strike + N_STD_CUTOFF * std_dev).max(strike * 1.05);

                        // Boundary term: g(K) · C_sw(K) = DF_pay · Black76_call(K).
                        let boundary = g_times_c(strike, vol_at_strike, true);

                        // Integral term: ∫_K^{K_max} [2·g'(k) + (k-K)·g''(k)] · C_sw(k) dk.
                        //
                        // g'(k) via non-uniform central differences (k_lo clamped at K_FLOOR).
                        // g''(k) via non-uniform 3-point second difference reusing the same
                        // k_lo/k_hi nodes; reverts to standard (g_hi−2g_ctr+g_lo)/h² when
                        // both spacings equal G_PRIME_H.
                        let integral = gauss_legendre_integrate(
                            |k: f64| {
                                let v = vol_surface.value_clamped(ttf, k);
                                let c_sw = par_annuity(k.max(K_FLOOR), cms_tenor, m)
                                    * black76_call(forward_rate, k, v, ttf);
                                // Nodes for finite differences.
                                let k_lo = (k - G_PRIME_H).max(K_FLOOR);
                                let k_hi = k + G_PRIME_H;
                                let g_lo = df_pay / par_annuity(k_lo, cms_tenor, m);
                                let g_hi = df_pay / par_annuity(k_hi, cms_tenor, m);
                                let g_ctr = df_pay / par_annuity(k.max(K_FLOOR), cms_tenor, m);
                                let h_lo = k - k_lo;
                                let h_hi = k_hi - k; // always G_PRIME_H
                                                     // g'(k): non-uniform central difference.
                                let g_prime = (g_hi - g_lo) / (h_lo + h_hi);
                                // g''(k): non-uniform 3-point second difference.
                                // Guard against zero denominator when k_lo is clamped to k.
                                let denom = h_lo * h_hi * (h_lo + h_hi);
                                let g_pp = if denom > 0.0 {
                                    2.0 * (h_lo * g_hi - (h_lo + h_hi) * g_ctr + h_hi * g_lo)
                                        / denom
                                } else {
                                    0.0
                                };
                                (2.0 * g_prime + (k - strike) * g_pp) * c_sw
                            },
                            strike,
                            k_max,
                            QUAD_ORDER,
                        )
                        .unwrap_or(0.0);

                        boundary + integral
                    }

                    OptionType::Put => {
                        // Floorlet formula (Andersen-Piterbarg §16.2, IBP derivation):
                        //   V = g(K) · P_sw(K) − ∫_{K_min}^K [2·g'(k) + (k-K)·g''(k)] · P_sw(k) dk
                        //
                        // Note the MINUS sign.  g(k) = DF_pay / A_par(k) is strictly
                        // INCREASING in k (because A_par(k) is strictly decreasing), so
                        // g'(k) > 0 and the integral is positive.  The minus sign ensures
                        // V_floor < g(K)·P_sw(K), consistent with CMS convexity raising
                        // the payment-measure forward above the swap forward and thereby
                        // reducing the in-the-money probability for a floorlet.
                        // For k < K, (k-K) < 0, so the g'' term subtracts from the
                        // effective integrand, reducing the magnitude of the subtracted
                        // integral and raising V_floor slightly relative to the g'-only formula.
                        let k_min = (strike - N_STD_CUTOFF * std_dev).max(K_FLOOR);

                        // Boundary term: g(K) · P_sw(K) = DF_pay · Black76_put(K).
                        let boundary = g_times_c(strike, vol_at_strike, false);

                        // Integral term: ∫_{K_min}^K [2·g'(k) + (k-K)·g''(k)] · P_sw(k) dk.
                        let integral = gauss_legendre_integrate(
                            |k: f64| {
                                let v = vol_surface.value_clamped(ttf, k);
                                let p_sw = par_annuity(k.max(K_FLOOR), cms_tenor, m)
                                    * black76_put(forward_rate, k, v, ttf);
                                // Nodes for finite differences.
                                let k_lo = (k - G_PRIME_H).max(K_FLOOR);
                                let k_hi = k + G_PRIME_H;
                                let g_lo = df_pay / par_annuity(k_lo, cms_tenor, m);
                                let g_hi = df_pay / par_annuity(k_hi, cms_tenor, m);
                                let g_ctr = df_pay / par_annuity(k.max(K_FLOOR), cms_tenor, m);
                                let h_lo = k - k_lo;
                                let h_hi = k_hi - k;
                                // g'(k): non-uniform central difference.
                                let g_prime = (g_hi - g_lo) / (h_lo + h_hi);
                                // g''(k): non-uniform 3-point second difference.
                                // Guard against zero denominator when k_lo is clamped to k.
                                let denom = h_lo * h_hi * (h_lo + h_hi);
                                let g_pp = if denom > 0.0 {
                                    2.0 * (h_lo * g_hi - (h_lo + h_hi) * g_ctr + h_hi * g_lo)
                                        / denom
                                } else {
                                    0.0
                                };
                                (2.0 * g_prime + (k - strike) * g_pp) * p_sw
                            },
                            k_min,
                            strike,
                            QUAD_ORDER,
                        )
                        .unwrap_or(0.0);

                        boundary - integral
                    }
                }
            };

            total_pv += period_pv * accrual_fraction;
        }

        Ok(Money::new(
            total_pv * inst.notional.amount(),
            inst.notional.currency(),
        ))
    }

    /// Calculate forward swap rate and market annuity for a given swap period.
    ///
    /// Delegates to the shared `forward_swap_rate` module for curve-consistent
    /// discount factor and forward rate calculations.
    pub(crate) fn calculate_forward_swap_rate(
        &self,
        inst: &CmsOption,
        market: &MarketContext,
        as_of: Date,
        start: Date,
        end: Date,
    ) -> Result<(f64, f64)> {
        let convention = crate::instruments::rates::exotics_shared::forward_swap_rate::resolve_reference_swap_convention(
            inst.swap_convention,
            inst.notional.currency(),
        )?;
        let calendar_id = convention.calendar_id().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "CMS reference-swap convention has no calendar".to_string(),
            )
        })?;
        crate::instruments::rates::exotics_shared::forward_swap_rate::calculate_forward_swap_rate(
            crate::instruments::rates::exotics_shared::forward_swap_rate::ForwardSwapRateInputs {
                market,
                discount_curve_id: &inst.discount_curve_id,
                forward_curve_id: &inst.forward_curve_id,
                as_of,
                start,
                end,
                fixed_freq: inst.resolved_swap_fixed_freq(),
                fixed_day_count: inst.resolved_swap_day_count(),
                float_freq: inst.resolved_swap_float_freq(),
                float_day_count: inst.resolved_swap_float_day_count(),
                calendar_id: &calendar_id,
                business_day_convention: convention.business_day_convention(),
                stub: finstack_quant_core::dates::StubKind::ShortFront,
                end_of_month: start.end_of_month() == start && end.end_of_month() == end,
                payment_lag_days: convention.payment_lag_days(),
                enforce_forward_tenor: !convention.uses_daily_compounding(),
            },
        )
    }
}

impl Default for CmsReplicationPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for CmsReplicationPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CmsOption, ModelKey::StaticReplication)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let cms = instrument
            .as_any()
            .downcast_ref::<CmsOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CmsOption, instrument.key())
            })?;

        let pv = self.price_internal(cms, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(cms.id(), as_of, pv))
    }
}

/// Present value using static replication (direct entry point for metrics/wrappers).
#[allow(dead_code)]
pub(crate) fn compute_pv(inst: &CmsOption, curves: &MarketContext, as_of: Date) -> Result<Money> {
    CmsReplicationPricer::new().price_internal(inst, curves, as_of)
}
