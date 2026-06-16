//! Strongly-typed metric identifiers for compile-time validation.
//!
//! Provides a comprehensive set of metric IDs covering bond, IRS, deposit,
//! and risk metrics. Each ID is strongly-typed to prevent runtime errors
//! and enable compile-time validation of metric dependencies.
//!
//! # Metric Categories
//!
//! - **Bond metrics**: Yield, duration, convexity, pricing, credit spreads
//! - **IRS metrics**: DV01, annuity factors, par rates, present values
//! - **Deposit metrics**: Discount factors, par rates, year fractions
//! - **Risk metrics**: DV01 (standard for all parallel rate sensitivity), CS01, BucketedDV01, BucketedCS01, Theta, and all standardized "01" sensitivity metrics
//! - **Standardized sensitivity metrics**: Dividend01, Inflation01, Prepayment01, Default01, Severity01, Conversion01, CollateralHaircut01, CollateralPrice01, Nav01, Carry01, Hurdle01, Dv01Domestic, Dv01Foreign, Fx01, Npv01, SpreadDv01, Correlation01, FxVega, ConvexityAdjustmentRisk
//! - **Custom metrics**: User-defined metrics with dynamic identifiers

use finstack_quant_core::HashMap;
use serde::{Deserialize, Serialize};
use std::borrow::{Borrow, Cow};
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

/// Strongly-typed metric identifier.
///
/// Provides compile-time validation, autocomplete support, and safe refactoring
/// when metric names change. Covers bond, IRS, deposit, and risk metrics.
///
/// See unit tests and `examples/` for usage.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MetricId(Cow<'static, str>);

#[allow(non_upper_case_globals)] // PascalCase names for metric ID constants
impl MetricId {
    /// Creates a custom metric ID.
    ///
    /// Use this for user-defined metrics that aren't part of the standard set.
    /// Custom metrics are stored as strings and can have any identifier.
    pub fn custom(id: impl Into<String>) -> Self {
        MetricId(Cow::Owned(id.into()))
    }

    /// Converts to string representation for compatibility.
    ///
    /// Returns a lowercase, snake_case string that can be used for
    /// serialization, logging, or API interfaces.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Checks if this is a custom (non-standard) metric.
    ///
    /// Returns `true` if the metric was created via `custom()` and is not
    /// part of the standard set.
    pub fn is_custom(&self) -> bool {
        !metric_lookup().contains_key(self.as_str())
    }

    /// Parses a string into a MetricId with strict validation.
    ///
    /// Unlike `FromStr`, this method returns an error for unknown metric names
    /// rather than creating a custom metric. Use this for user-provided inputs
    /// where typos should be caught, not silently accepted.
    ///
    /// # Errors
    ///
    /// Returns `Error::UnknownMetric` if the string does not match any standard
    /// metric. The error includes the invalid metric name and a list of all
    /// available standard metrics.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_valuations::metrics::MetricId;
    ///
    /// // Parse known metric - succeeds
    /// let dv01 = MetricId::parse_strict("dv01").unwrap();
    /// assert_eq!(dv01, MetricId::Dv01);
    ///
    /// // Case insensitive
    /// let theta = MetricId::parse_strict("THETA").unwrap();
    /// assert_eq!(theta, MetricId::Theta);
    ///
    /// // Unknown metric - fails with error
    /// let result = MetricId::parse_strict("dv01x");
    /// assert!(result.is_err());
    /// ```
    ///
    /// # Custom metrics via FromStr
    ///
    /// To accept custom metrics, use `FromStr::from_str`
    /// or the `.parse()` method which never fails:
    ///
    /// ```
    /// use finstack_quant_valuations::metrics::MetricId;
    /// use std::str::FromStr;
    ///
    /// // FromStr allows custom metrics
    /// let custom = MetricId::from_str("my_custom_metric").unwrap();
    /// assert!(custom.is_custom());
    ///
    /// // Strict parsing rejects unknown metrics
    /// let result = MetricId::parse_strict("my_custom_metric");
    /// assert!(result.is_err());
    /// ```
    pub fn parse_strict(s: &str) -> finstack_quant_core::Result<Self> {
        let lower = s.to_lowercase();
        if let Some(id) = metric_lookup().get(&lower) {
            Ok(id.clone())
        } else {
            Err(finstack_quant_core::Error::unknown_metric(
                s,
                Self::ALL_STANDARD
                    .iter()
                    .map(|m| m.as_str().to_string())
                    .collect(),
            ))
        }
    }

    // ========================================================================
    // Core Risk Metrics
    // ========================================================================

    /// Time decay (theta) - 1D Day Time decay P&L
    pub const Theta: Self = Self(Cow::Borrowed("theta"));

    /// Theta carry component (coupon accrual, pull-to-par, funding)
    pub const ThetaCarry: Self = Self(Cow::Borrowed("theta_carry"));

    /// Theta roll-down component (PV change from moving along same curve)
    pub const ThetaRollDown: Self = Self(Cow::Borrowed("theta_roll_down"));

    /// Total carry decomposition (coupon_income + pull_to_par + roll_down - funding_cost).
    pub const CarryTotal: Self = Self(Cow::Borrowed("carry_total"));

    /// Coupon/interest income received during the carry horizon.
    pub const CouponIncome: Self = Self(Cow::Borrowed("coupon_income"));

    /// PV convergence toward par (time effect at flat yield, isolates amortization).
    pub const PullToPar: Self = Self(Cow::Borrowed("pull_to_par"));

    /// Curve shape benefit from aging along a sloped curve (includes slide).
    pub const RollDown: Self = Self(Cow::Borrowed("roll_down"));

    /// Cost of financing the position (dirty_price x funding_rate x dcf).
    pub const FundingCost: Self = Self(Cow::Borrowed("funding_cost"));

    /// Diagnostic flag for the carry decomposition: `1.0` when the
    /// pull-to-par / roll-down split is **degenerate** (the `Ytm` metric was
    /// not available, so `pull_to_par` is reported as `0.0` and `roll_down`
    /// absorbs the entire PV change), `0.0` when the split is well-defined.
    ///
    /// Consumers reading `pull_to_par` / `roll_down` should check this flag
    /// before attributing the split to genuine roll-down.
    pub const CarryDecompositionDegenerate: Self =
        Self(Cow::Borrowed("carry_decomposition_degenerate"));

    /// Realized theta/carry horizon in **calendar days**.
    ///
    /// `Theta`, `CarryTotal`, `CouponIncome`, `PullToPar`, `RollDown` and
    /// `FundingCost` are *period totals* over the producer's `theta_period`
    /// override (default `"1D"`), capped at the instrument's expiry. This
    /// metric records the days actually rolled so consumers rescaling those
    /// totals to a different window (e.g. P&L attribution multiplying by the
    /// attribution window's day count) can divide by the producer horizon
    /// instead of silently assuming it was one day.
    pub const ThetaPeriodDays: Self = Self(Cow::Borrowed("theta_period_days"));

    /// Breakeven parameter shift: how much can the configured target parameter
    /// (spread, yield, vol, correlation) move before carry + roll-down is wiped out.
    ///
    /// Requires `BreakevenConfig` on `MetricPricingOverrides` and the corresponding
    /// sensitivity metric (e.g., `Cs01` for `ZSpread`) to be computed first.
    ///
    /// **Units:** same as the sensitivity bump (typically 1bp for CS01/DV01).
    ///
    /// **Sign:** positive = parameter can move against you by this amount;
    /// negative = carry is negative, parameter must move in your favour.
    pub const Breakeven: Self = Self(Cow::Borrowed("breakeven"));

    /// Dollar value of 01 (DV01) for a parallel rates bump.
    ///
    /// Measures the change in present value for a **+1bp parallel shift** of the
    /// relevant rates curve set under the instrument's pricing convention.
    ///
    /// Units: currency per 1bp.
    ///
    /// # Sign Convention
    ///
    /// Positive means the position gains value when rates rise; negative means it
    /// loses value when rates rise.
    ///
    /// # Note
    ///
    /// Distinct from:
    /// - `Pv01`: swap-style PV change for a 1bp curve bump under its documented convention
    /// - `YieldDv01`: sensitivity to the instrument's own quoted yield, not a market-curve bump
    pub const Dv01: Self = Self(Cow::Borrowed("dv01"));

    /// Credit spread sensitivity (CS01) for a parallel quoted-spread bump.
    ///
    /// Measures the change in present value for a **+1bp parallel shift** in
    /// market credit spreads, typically by bumping par spreads and re-bootstrapping
    /// the credit curve.
    ///
    /// Units: currency per 1bp spread move.
    ///
    /// # Note
    ///
    /// Distinct from `Cs01Hazard`, which bumps hazard rates directly instead of
    /// quoted spreads.
    pub const Cs01: Self = Self(Cow::Borrowed("cs01"));

    /// Bucketed DV01 risk for pointwise or tenor-bucket rate moves.
    ///
    /// Represents rate sensitivity broken out by tenor bucket rather than as a
    /// single parallel number. Implementations typically expose the aggregate
    /// total under `bucketed_dv01` and per-bucket or per-curve components under
    /// flattened composite keys.
    ///
    /// Units: currency per 1bp bucket move.
    pub const BucketedDv01: Self = Self(Cow::Borrowed("bucketed_dv01"));

    /// Bucketed credit spread risk for pointwise spread moves.
    ///
    /// Represents quoted-spread sensitivity decomposed by tenor bucket or pillar.
    ///
    /// Units: currency per 1bp bucket move.
    pub const BucketedCs01: Self = Self(Cow::Borrowed("bucketed_cs01"));

    /// Credit spread sensitivity via direct hazard rate bump (CS01 Hazard)
    ///
    /// Unlike `Cs01` which bumps par spreads and re-bootstraps, this metric
    /// directly shifts hazard rates. Use when par spread points are unavailable
    /// or when hazard-rate sensitivity is specifically needed.
    pub const Cs01Hazard: Self = Self(Cow::Borrowed("cs01_hazard"));

    /// Bucketed credit spread risk via direct hazard-rate bumps.
    ///
    /// Units: currency per 1bp hazard-rate bucket move.
    pub const BucketedCs01Hazard: Self = Self(Cow::Borrowed("bucketed_cs01_hazard"));

    // ========================================================================
    // FX Spot Metrics
    // ========================================================================

    /// Spot rate
    pub const SpotRate: Self = Self(Cow::Borrowed("spot_rate"));

    /// Base amount
    pub const BaseAmount: Self = Self(Cow::Borrowed("base_amount"));

    /// Quote amount
    pub const QuoteAmount: Self = Self(Cow::Borrowed("quote_amount"));

    /// Inverse rate
    pub const InverseRate: Self = Self(Cow::Borrowed("inverse_rate"));

    // ========================================================================
    // Equity Metrics
    // ========================================================================

    /// Equity price per share (spot price).
    ///
    /// This is a market data input used in equity option and forward pricing.
    /// Units: currency per share. Typically sourced from market data context.
    ///
    /// # Note
    /// While primarily an input, it is exposed as a metric ID to allow
    /// tracking and reporting alongside computed metrics.
    pub const EquityPricePerShare: Self = Self(Cow::Borrowed("equity_price_per_share"));

    /// Number of effective shares for the position.
    ///
    /// This is a position-level input representing the share count after
    /// adjusting for stock splits, corporate actions, etc.
    /// Units: shares (dimensionless count).
    ///
    /// # Note
    /// While primarily an input, it is exposed as a metric ID to allow
    /// position-level reporting and reconciliation.
    pub const EquityShares: Self = Self(Cow::Borrowed("equity_shares"));

    /// Equity dividend yield (annualized, continuous compounding).
    ///
    /// This is a market data input used in equity option pricing models.
    /// Units: decimal (0.02 = 2% per annum).
    ///
    /// # Note
    /// While primarily an input, it is exposed as a metric ID to allow
    /// tracking and reporting alongside computed metrics.
    pub const EquityDividendYield: Self = Self(Cow::Borrowed("equity_dividend_yield"));

    /// Equity forward price per share.
    ///
    /// Computed as: S * exp((r - q) * T), where S is spot, r is risk-free rate,
    /// q is dividend yield, and T is time to delivery.
    /// Units: currency per share.
    pub const EquityForwardPrice: Self = Self(Cow::Borrowed("equity_forward_price"));

    /// Exchange or model futures price for futures-style instruments.
    pub const FuturesPrice: Self = Self(Cow::Borrowed("futures_price"));

    /// Futures basis, typically futures price less spot or CTD-implied fair value.
    pub const Basis: Self = Self(Cow::Borrowed("basis"));

    // ========================================================================
    // Bond Metrics
    // ========================================================================

    /// Dirty price (includes accrued interest)
    pub const DirtyPrice: Self = Self(Cow::Borrowed("dirty_price"));

    /// Clean price (excludes accrued interest)
    pub const CleanPrice: Self = Self(Cow::Borrowed("clean_price"));

    /// Delivery conversion factor for futures with deliverable baskets.
    pub const ConversionFactor: Self = Self(Cow::Borrowed("conversion_factor"));

    /// Accrued interest since last coupon payment
    pub const Accrued: Self = Self(Cow::Borrowed("accrued"));

    /// Yield to maturity
    pub const Ytm: Self = Self(Cow::Borrowed("ytm"));

    /// Yield to worst
    pub const Ytw: Self = Self(Cow::Borrowed("ytw"));

    /// Macaulay duration
    pub const DurationMac: Self = Self(Cow::Borrowed("duration_mac"));

    /// Modified duration under the instrument's quoted yield convention.
    ///
    /// Measures first-order percentage price sensitivity to a small change in
    /// yield, approximately `-dP/P / dy`.
    ///
    /// Units: years.
    ///
    /// # Note
    ///
    /// Distinct from `Dv01` and `YieldDv01`, which convert sensitivity into
    /// currency change for a 1bp move.
    pub const DurationMod: Self = Self(Cow::Borrowed("duration_mod"));

    /// Yield-basis DV01 for bonds and other yield-quoted fixed-income instruments.
    ///
    /// Measures the dollar price change for a 1bp change in the instrument's own
    /// quoted yield convention, rather than a parallel bump of the market curve.
    pub const YieldDv01: Self = Self(Cow::Borrowed("yield_dv01"));

    /// Bond-style convexity under the instrument's yield convention.
    ///
    /// Measures the second-order sensitivity of price to changes in quoted yield.
    ///
    /// Units: years squared under standard bond-convexity conventions unless a
    /// more specific instrument doc says otherwise.
    ///
    /// # Note
    ///
    /// Distinct from `IrConvexity`, which is used for swap/rates contexts.
    pub const Convexity: Self = Self(Cow::Borrowed("convexity"));

    // ========================================================================
    // Spread Metrics
    // ========================================================================

    /// Z-spread - Zero-vol spread
    pub const ZSpread: Self = Self(Cow::Borrowed("z_spread"));

    /// OAS - Option-adjusted spread
    pub const Oas: Self = Self(Cow::Borrowed("oas"));

    /// Embedded option value for callable/putable bonds (in currency units)
    ///
    /// For callable bonds: P_callable - P_straight (negative holder value)
    /// For putable bonds: P_putable - P_straight (positive holder value)
    /// Returns 0 for bonds without embedded options.
    pub const EmbeddedOptionValue: Self = Self(Cow::Borrowed("embedded_option_value"));

    /// I-spread - Yield over interpolated swap curve
    pub const ISpread: Self = Self(Cow::Borrowed("i_spread"));

    /// Discount margin for floating-rate bonds (decimal; 0.01 = 100 bps)
    pub const DiscountMargin: Self = Self(Cow::Borrowed("discount_margin"));

    /// G-spread - Govvie spread
    pub const GSpread: Self = Self(Cow::Borrowed("g_spread"));

    /// Par asset swap spread (market-standard ASW quote)
    pub const ASWPar: Self = Self(Cow::Borrowed("asw_par"));

    /// Market (price-based) asset swap spread
    pub const ASWMarket: Self = Self(Cow::Borrowed("asw_market"));

    // ========================================================================
    // IRS Metrics
    // ========================================================================

    /// Annuity factor for fixed leg
    pub const Annuity: Self = Self(Cow::Borrowed("annuity"));

    /// Par swap rate (at-the-money fixed rate).
    ///
    /// The fixed rate that makes the swap have zero present value.
    /// Units: decimal (0.05 = 5% per annum).
    pub const ParRate: Self = Self(Cow::Borrowed("par_rate"));

    /// Present value of a basis point (PV01).
    ///
    /// Instrument-specific PV change for a one-basis-point move in the quoted
    /// or primary rate driver documented by that instrument. For FRAs, this is
    /// the signed projection-curve BR01/PV01; `ForwardPv01` exposes the same
    /// projection-curve sensitivity under an explicit multi-curve name.
    ///
    /// Units: currency per 1bp. Positive means the position gains value when
    /// the bumped driver rises; negative means it loses value.
    pub const Pv01: Self = Self(Cow::Borrowed("pv01"));

    /// Present value of fixed leg.
    ///
    /// Discounted sum of all fixed-leg cashflows.
    /// Units: currency.
    pub const PvFixed: Self = Self(Cow::Borrowed("pv_fixed"));

    /// Present value of floating leg.
    ///
    /// Discounted sum of all floating-leg cashflows (projected forward rates).
    /// Units: currency.
    pub const PvFloat: Self = Self(Cow::Borrowed("pv_float"));

    // ========================================================================
    // Deposit Metrics
    // ========================================================================
    // These metrics are used for deposit instrument valuation and curve
    // calibration. They provide transparency into the intermediate values
    // used in pricing calculations.

    /// Year fraction between start and end dates.
    ///
    /// Computed using the instrument's day-count convention.
    /// Units: years (dimensionless).
    ///
    /// Used in: deposit valuation, curve calibration bootstrap.
    pub const Yf: Self = Self(Cow::Borrowed("yf"));

    /// Discount factor at start date (from curve).
    ///
    /// DF(0, start) where 0 is the valuation date.
    /// Units: dimensionless (0 < df <= 1 for positive rates).
    ///
    /// Used in: forward-start deposit valuation, curve calibration.
    pub const DfStart: Self = Self(Cow::Borrowed("df_start"));

    /// Discount factor at end date (from curve).
    ///
    /// DF(0, end) where 0 is the valuation date.
    /// Units: dimensionless (0 < df <= 1 for positive rates).
    ///
    /// Used in: deposit valuation, curve calibration.
    pub const DfEnd: Self = Self(Cow::Borrowed("df_end"));

    /// Deposit par rate (implied from curve).
    ///
    /// The rate that makes the deposit have zero present value given the
    /// current curve. Units: decimal (0.05 = 5% per annum).
    ///
    /// Distinct from `QuoteRate` which is the market-observed rate.
    pub const DepositParRate: Self = Self(Cow::Borrowed("deposit_par_rate"));

    /// Discount factor implied by the market quote.
    ///
    /// DF(start, end) = 1 / (1 + rate * yf) for simple compounding.
    /// Units: dimensionless.
    ///
    /// Used in: curve calibration as a calibration target.
    pub const DfEndFromQuote: Self = Self(Cow::Borrowed("df_end_from_quote"));

    /// Quoted market rate for the deposit.
    ///
    /// The rate observed in the market, used as input to curve calibration.
    /// Units: decimal (0.05 = 5% per annum).
    ///
    /// **Relation to DepositParRate**: `QuoteRate` is the market input;
    /// `DepositParRate` is the rate implied by the calibrated curve.
    /// After successful calibration, these should match within tolerance.
    pub const QuoteRate: Self = Self(Cow::Borrowed("quote_rate"));

    /// Forward rate implied by the projection curve for futures-style instruments.
    pub const ImpliedForward: Self = Self(Cow::Borrowed("implied_forward"));

    /// Convexity adjustment applied to a quoted or model futures rate.
    pub const ConvexityAdjustment: Self = Self(Cow::Borrowed("convexity_adjustment"));

    /// Number of fixed-leg payment cashflows in a rates instrument schedule.
    pub const FixedLegPaymentCount: Self = Self(Cow::Borrowed("fixed_leg_payment_count"));

    /// Number of floating-leg payment cashflows in a rates instrument schedule.
    pub const FloatingLegPaymentCount: Self = Self(Cow::Borrowed("floating_leg_payment_count"));

    /// First fixed-leg payment date as days since Unix epoch.
    pub const FixedFirstPaymentDate: Self = Self(Cow::Borrowed("fixed_first_payment_date"));

    /// Last fixed-leg payment date as days since Unix epoch.
    pub const FixedLastPaymentDate: Self = Self(Cow::Borrowed("fixed_last_payment_date"));

    /// First floating-leg payment date as days since Unix epoch.
    pub const FloatingFirstPaymentDate: Self = Self(Cow::Borrowed("floating_first_payment_date"));

    /// Last floating-leg payment date as days since Unix epoch.
    pub const FloatingLastPaymentDate: Self = Self(Cow::Borrowed("floating_last_payment_date"));

    /// First fixed-leg accrual factor.
    pub const FixedFirstAccrualFactor: Self = Self(Cow::Borrowed("fixed_first_accrual_factor"));

    /// First floating-leg accrual factor.
    pub const FloatingFirstAccrualFactor: Self =
        Self(Cow::Borrowed("floating_first_accrual_factor"));

    // ========================================================================
    // CDS Metrics
    // ========================================================================

    /// CDS par spread under the instrument's premium-leg convention.
    ///
    /// The running spread that makes the CDS have zero PV under the current
    /// discount and survival curves.
    ///
    /// Units: decimal spread per annum unless a quoting layer converts it to bp
    /// for display.
    pub const ParSpread: Self = Self(Cow::Borrowed("par_spread"));

    /// Risky PV01 for CDS premium-leg valuation.
    ///
    /// Present value of one basis point of running premium paid over the risky
    /// premium leg, including default-contingent survival weighting.
    ///
    /// Units: currency per 1bp running spread.
    pub const RiskyPv01: Self = Self(Cow::Borrowed("risky_pv01"));

    /// Risky annuity (premium leg PV per 1bp)
    pub const RiskyAnnuity: Self = Self(Cow::Borrowed("risky_annuity"));

    /// Protection leg present value
    pub const ProtectionLegPv: Self = Self(Cow::Borrowed("protection_leg_pv"));

    /// Premium leg present value
    pub const PremiumLegPv: Self = Self(Cow::Borrowed("premium_leg_pv"));

    /// Jump-to-default amount.
    ///
    /// Immediate P&L impact of an instantaneous default event under the
    /// instrument's loss and settlement convention.
    ///
    /// Units: currency.
    pub const JumpToDefault: Self = Self(Cow::Borrowed("jump_to_default"));

    /// Clean default exposure.
    ///
    /// Signed LGD payout net of the current mark, excluding accrued premium on
    /// default. This matches dealer-screen "default exposure" style measures
    /// more closely than accrued-premium-adjusted jump-to-default.
    ///
    /// Units: currency.
    pub const DefaultExposure: Self = Self(Cow::Borrowed("default_exposure"));

    /// Expected loss under the current credit model.
    ///
    /// Expected discounted credit loss implied by default probabilities and
    /// recovery assumptions.
    ///
    /// Units: currency.
    pub const ExpectedLoss: Self = Self(Cow::Borrowed("expected_loss"));

    /// Default probability over the documented horizon.
    ///
    /// Units: decimal probability in `[0, 1]`.
    ///
    /// # Note
    ///
    /// The horizon is instrument-specific and should be interpreted together
    /// with the API producing the measure.
    pub const DefaultProbability: Self = Self(Cow::Borrowed("default_probability"));

    /// Expected recovery rate
    pub const Recovery01: Self = Self(Cow::Borrowed("recovery_01"));

    // ========================================================================
    // Option Metrics
    // ========================================================================

    /// Cash delta with respect to the instrument's chosen spot driver.
    ///
    /// Measures first-order PV sensitivity `dPV/dS` to the relevant underlying
    /// spot or forward-style driver.
    ///
    /// Units: currency per unit of underlying move, already including instrument
    /// scaling such as notional, contract multiplier, or quantity where applicable.
    pub const Delta: Self = Self(Cow::Borrowed("delta"));

    /// Forward delta before spot/premium convention adjustments.
    pub const DeltaForward: Self = Self(Cow::Borrowed("delta_forward"));

    /// Premium-adjusted delta under FX option market quoting conventions.
    pub const DeltaPremiumAdjusted: Self = Self(Cow::Borrowed("delta_premium_adjusted"));

    /// Cash gamma with respect to the instrument's chosen spot driver.
    ///
    /// Measures second-order PV sensitivity `d²PV/dS²`.
    ///
    /// Units: currency per unit-underlying squared.
    pub const Gamma: Self = Self(Cow::Borrowed("gamma"));

    /// Cash vega for a 1 vol point move.
    ///
    /// Measures the PV change for a **0.01 absolute volatility move**
    /// (one vol point).
    ///
    /// Units: currency per 1 vol point.
    pub const Vega: Self = Self(Cow::Borrowed("vega"));

    /// Hull-White short-rate volatility sensitivity (model vega).
    ///
    /// Measures the PV change for a **0.01 absolute move in the Hull-White
    /// short-rate σ**. This is a *model-parameter* vega, not a Black/market
    /// vol vega — it lives on a different vol axis than [`MetricId::Vega`]
    /// and must not be aggregated with Black vegas.
    ///
    /// Units: currency per 0.01 HW σ.
    pub const HwSigmaVega: Self = Self(Cow::Borrowed("hw_sigma_vega"));

    /// Bucketed vega by volatility-surface point or node.
    ///
    /// Represents vega decomposed by surface location rather than as a single
    /// aggregate number.
    ///
    /// Units: currency per 1 vol point at each bucket.
    pub const BucketedVega: Self = Self(Cow::Borrowed("bucketed_vega"));

    /// Domestic rho for a 1bp move in the relevant domestic rate driver.
    ///
    /// Measures `PV(r + 1bp) - PV(r)` under the instrument's domestic discounting
    /// convention.
    ///
    /// Units: currency per 1bp.
    pub const Rho: Self = Self(Cow::Borrowed("rho"));

    /// Foreign or dividend rho for a 1bp move in the secondary carry driver.
    ///
    /// Measures sensitivity to the foreign discount rate in FX models or the
    /// dividend-yield style driver in equity models, depending on instrument type.
    ///
    /// Units: currency per 1bp.
    pub const ForeignRho: Self = Self(Cow::Borrowed("foreign_rho"));

    /// Forward-curve PV01 for a 1bp forward/projection bump.
    ///
    /// Distinct from `Dv01` when discount and forward curves are separate and
    /// only the projection curve is bumped. Prefer this explicit metric when a
    /// product exposes both discount-curve and projection-curve rate risk.
    ///
    /// Units: currency per 1bp.
    pub const ForwardPv01: Self = Self(Cow::Borrowed("forward_pv01"));

    /// Vanna, the mixed sensitivity to spot and volatility.
    ///
    /// Commonly interpreted as `d²PV / (dS dσ)` under the instrument's bump
    /// convention.
    ///
    /// Units: currency per unit-underlying per **unit (decimal) volatility**
    /// (i.e. per 1.00 change in σ, not per vol point). All producers — the
    /// analytic FX/equity/quanto/FX-barrier providers and `GenericFdVanna` —
    /// normalize by the absolute vol bump, so vanna is per decimal vol while
    /// `Volga` is per vol-point squared; the two second-order vol Greeks
    /// intentionally use different vol-axis units.
    pub const Vanna: Self = Self(Cow::Borrowed("vanna"));

    /// Volga, the second-order sensitivity to volatility.
    ///
    /// Commonly interpreted as `d²PV / dσ²` under the instrument's bump convention.
    ///
    /// Units: currency per vol-point squared.
    pub const Volga: Self = Self(Cow::Borrowed("volga"));

    /// Veta (theta sensitivity to volatility)
    pub const Veta: Self = Self(Cow::Borrowed("veta"));

    /// Interest-rate convexity for swap/rates contexts.
    ///
    /// Measures second-order PV sensitivity to the relevant rates driver.
    ///
    /// Units depend on the producing calculator, but the measure should be
    /// interpreted as a second-order rates sensitivity rather than bond-style
    /// quoted-yield convexity.
    pub const IrConvexity: Self = Self(Cow::Borrowed("ir_convexity"));

    /// Cross-gamma between discount and forward curves for IRS.
    ///
    /// Mixed second derivative: d²PV / (dr_disc × dr_fwd).
    /// Measures how DV01 with respect to one curve changes when the other moves.
    pub const IrCrossGamma: Self = Self(Cow::Borrowed("ir_cross_gamma"));

    // ========================================================================
    // Cross-Factor Gamma Metrics
    // ========================================================================

    /// Cross-gamma between interest rates and credit spreads.
    ///
    /// Mixed second derivative: ∂²V / (∂r × ∂s).
    /// Measures how rate sensitivity changes when credit spreads move.
    pub const CrossGammaRatesCredit: Self = Self(Cow::Borrowed("cross_gamma_rates_credit"));

    /// Cross-gamma between interest rates and implied volatility.
    ///
    /// Mixed second derivative: ∂²V / (∂r × ∂σ).
    pub const CrossGammaRatesVol: Self = Self(Cow::Borrowed("cross_gamma_rates_vol"));

    /// Cross-gamma between spot price and implied volatility.
    ///
    /// Mixed second derivative ∂²V / (∂S × ∂σ), normalised to
    /// **percentage-point** moves in both factors.
    ///
    /// Produced by `CrossFactorCalculator` (SpotVol pair) via a four-corner
    /// central finite difference whose denominators are:
    /// - spot: `spot_bump_pct × 100`  (e.g. 1.0 per 1 % spot move)
    /// - vol: `vol_bump_abs × 100`    (e.g. 1.0 per 1 vol-point move)
    ///
    /// Units: currency per (1 pct-pt spot move) per (1 vol-point move).
    ///
    /// **Attribution contract**: multiply by `avg_spot_shift_pct` (percentage-
    /// point spot change) and `avg_vol_shift_abs` (vol-point change) to obtain
    /// the cross P&L.
    ///
    /// **Do NOT confuse with `Vanna`**, which is expressed per unit-spot per
    /// decimal-vol and differs by a factor of S₀ / 10_000.
    pub const CrossGammaSpotVol: Self = Self(Cow::Borrowed("cross_gamma_spot_vol"));

    /// Cross-gamma between spot price and credit spreads.
    ///
    /// Mixed second derivative ∂²V / (∂S × ∂s), normalised to
    /// **percentage-point** spot moves and **basis-point** credit spread moves.
    ///
    /// Produced by `CrossFactorCalculator` (SpotCredit pair) via a four-corner
    /// central finite difference whose denominators are:
    /// - spot: `spot_bump_pct × 100`  (e.g. 1.0 per 1 % spot move)
    /// - credit: `credit_bump_bp`     (e.g. 1.0 per 1 bp credit move)
    ///
    /// Units: currency per (1 pct-pt spot move) per (1 bp credit spread move).
    ///
    /// **Attribution contract**: multiply by `avg_spot_shift_pct` (percentage-
    /// point spot change) and `avg_credit_shift_bp` (bp credit spread change).
    pub const CrossGammaSpotCredit: Self = Self(Cow::Borrowed("cross_gamma_spot_credit"));

    /// Cross-gamma between FX rates and implied volatility.
    ///
    /// Mixed second derivative: ∂²V / (∂FX × ∂σ).
    pub const CrossGammaFxVol: Self = Self(Cow::Borrowed("cross_gamma_fx_vol"));

    /// Cross-gamma between FX rates and interest rates.
    ///
    /// Mixed second derivative: ∂²V / (∂FX × ∂r).
    pub const CrossGammaFxRates: Self = Self(Cow::Borrowed("cross_gamma_fx_rates"));

    /// Credit spread gamma, the second derivative with respect to spreads.
    ///
    /// Units: currency per bp squared when computed under bp bump conventions.
    pub const CsGamma: Self = Self(Cow::Borrowed("cs_gamma"));

    /// Inflation convexity, the second derivative with respect to inflation moves.
    ///
    /// Units depend on the bump convention of the producing calculator and should
    /// be interpreted together with the related inflation metric docs.
    pub const InflationConvexity: Self = Self(Cow::Borrowed("inflation_convexity"));

    /// Charm (rho sensitivity to volatility)
    pub const Charm: Self = Self(Cow::Borrowed("charm"));

    /// Color (gamma sensitivity to time)
    pub const Color: Self = Self(Cow::Borrowed("color"));

    /// Speed (gamma sensitivity to underlying)
    pub const Speed: Self = Self(Cow::Borrowed("speed"));

    /// Implied volatility inferred from an observed price.
    ///
    /// Units: decimal volatility (`0.20 = 20%`) unless a normal-volatility API
    /// states a different convention.
    pub const ImpliedVol: Self = Self(Cow::Borrowed("implied_vol"));

    // ========================================================================
    // Variance Swap Metrics
    // ========================================================================

    /// Vega expressed per variance point (variance swap sensitivity)
    pub const VarianceVega: Self = Self(Cow::Borrowed("variance_vega"));

    /// Expected variance under the pricing model
    pub const ExpectedVariance: Self = Self(Cow::Borrowed("variance_expected"));

    /// Realized variance computed from observed paths
    pub const RealizedVariance: Self = Self(Cow::Borrowed("variance_realized"));

    /// Variance notional exposure (payout multiplier)
    pub const VarianceNotional: Self = Self(Cow::Borrowed("variance_notional"));

    /// Strike volatility equivalent (sqrt of strike variance)
    pub const VarianceStrikeVol: Self = Self(Cow::Borrowed("variance_strike_vol"));

    /// Time to maturity as used in the variance swap conventions
    pub const VarianceTimeToMaturity: Self = Self(Cow::Borrowed("variance_time_to_maturity"));

    // ========================================================================
    // Other Risk Metrics
    // ========================================================================

    /// Dividend yield sensitivity per basis point
    pub const Dividend01: Self = Self(Cow::Borrowed("dividend01"));

    /// Inflation curve sensitivity per basis point
    pub const Inflation01: Self = Self(Cow::Borrowed("inflation01"));

    /// Prepayment rate sensitivity per basis point
    pub const Prepayment01: Self = Self(Cow::Borrowed("prepayment01"));

    /// Default rate sensitivity per basis point
    pub const Default01: Self = Self(Cow::Borrowed("default01"));

    /// Loss severity sensitivity per 1% change
    pub const Severity01: Self = Self(Cow::Borrowed("severity01"));

    /// Conversion ratio/price sensitivity per 1% change
    pub const Conversion01: Self = Self(Cow::Borrowed("conversion01"));

    /// Collateral haircut sensitivity per basis point
    pub const CollateralHaircut01: Self = Self(Cow::Borrowed("collateral_haircut01"));

    /// Collateral price sensitivity per 1% change
    pub const CollateralPrice01: Self = Self(Cow::Borrowed("collateral_price01"));

    /// NAV sensitivity per 1% change (private markets funds)
    pub const Nav01: Self = Self(Cow::Borrowed("nav01"));

    /// GP carry sensitivity per basis point (private markets funds)
    pub const Carry01: Self = Self(Cow::Borrowed("carry01"));

    /// Hurdle rate sensitivity per basis point (private markets funds)
    pub const Hurdle01: Self = Self(Cow::Borrowed("hurdle01"));

    /// DV01 for domestic currency (FX Swap)
    pub const Dv01Domestic: Self = Self(Cow::Borrowed("dv01_domestic"));

    /// DV01 for foreign currency (FX Swap)
    pub const Dv01Foreign: Self = Self(Cow::Borrowed("dv01_foreign"));

    /// FX spot rate sensitivity per basis point
    pub const Fx01: Self = Self(Cow::Borrowed("fx01"));

    /// NPV sensitivity per basis point (inflation swaps)
    pub const Npv01: Self = Self(Cow::Borrowed("npv01"));

    /// Running coupon sensitivity per basis point (CDS Tranche)
    pub const SpreadDv01: Self = Self(Cow::Borrowed("spread_dv01"));

    /// Correlation sensitivity per 1% change (unified for all correlation risks)
    pub const Correlation01: Self = Self(Cow::Borrowed("correlation01"));

    /// FX volatility sensitivity per 1% change (quanto options)
    pub const FxVega: Self = Self(Cow::Borrowed("fx_vega"));

    /// FX spot rate delta (sensitivity to FX rate move, typically per 1%).
    ///
    /// Distinct from `Delta` which measures sensitivity to the instrument's
    /// primary underlying (equity spot, commodity price, etc.). `FxDelta`
    /// measures sensitivity to the FX rate for FX spot, FX swap, and
    /// quanto instruments.
    ///
    /// Units: currency per 1% FX rate move.
    pub const FxDelta: Self = Self(Cow::Borrowed("fx_delta"));

    /// Volatility index delta (sensitivity to volatility index level).
    ///
    /// Measures PV sensitivity to a 1-point move in a volatility index
    /// (e.g., VIX). Used for vol index futures and options.
    ///
    /// Units: currency per 1 vol point.
    pub const DeltaVol: Self = Self(Cow::Borrowed("delta_vol"));

    /// Per-constituent delta for basket instruments.
    ///
    /// Decomposes basket delta by individual constituent, providing
    /// per-name or per-asset sensitivity attribution.
    pub const ConstituentDelta: Self = Self(Cow::Borrowed("constituent_delta"));

    /// Convexity adjustment risk (CMS options)
    pub const ConvexityAdjustmentRisk: Self = Self(Cow::Borrowed("convexity_adjustment_risk"));

    // ========================================================================
    // TRS Metrics
    // ========================================================================

    /// Financing annuity for TRS
    pub const FinancingAnnuity: Self = Self(Cow::Borrowed("financing_annuity"));

    /// Index delta for TRS (equity: dV/dS per unit, FI: duration-weighted yield sensitivity)
    pub const IndexDelta: Self = Self(Cow::Borrowed("index_delta"));

    /// Duration-based DV01 for fixed income index TRS.
    ///
    /// Measures the dollar sensitivity to a 1bp yield change using the index duration:
    /// `DurationDv01 = Notional × Duration × 0.0001`.
    ///
    /// Distinct from `IndexDelta` (which measures sensitivity to the underlying index level)
    /// and from `Dv01` (which measures sensitivity to a parallel shift in the financing curve).
    pub const DurationDv01: Self = Self(Cow::Borrowed("duration_dv01"));

    // ========================================================================
    // Basis Swap Metrics
    // ========================================================================

    /// PV of primary floating leg (includes spread)
    pub const PvPrimary: Self = Self(Cow::Borrowed("pv_primary"));

    /// PV of reference floating leg
    pub const PvReference: Self = Self(Cow::Borrowed("pv_reference"));

    /// Annuity of primary leg
    pub const AnnuityPrimary: Self = Self(Cow::Borrowed("annuity_primary"));

    /// Annuity of reference leg
    pub const AnnuityReference: Self = Self(Cow::Borrowed("annuity_reference"));

    /// DV01 of primary leg
    pub const Dv01Primary: Self = Self(Cow::Borrowed("dv01_primary"));

    /// DV01 of reference leg
    pub const Dv01Reference: Self = Self(Cow::Borrowed("dv01_reference"));

    /// Par spread for basis swap (absolute: the spread that would set NPV to zero)
    pub const BasisParSpread: Self = Self(Cow::Borrowed("basis_par_spread"));

    /// Incremental par spread for basis swap (par spread minus current spread)
    ///
    /// Returns the additional spread (in basis points) needed on top of the current
    /// spread to bring the basis swap NPV to zero. Positive values indicate the
    /// current spread is below par; negative values indicate above par.
    pub const IncrementalParSpread: Self = Self(Cow::Borrowed("incremental_par_spread"));

    // ========================================================================
    // Repo Metrics
    // ========================================================================

    /// Market value of collateral
    pub const CollateralValue: Self = Self(Cow::Borrowed("collateral_value"));

    /// Required collateral value (with haircut)
    pub const RequiredCollateral: Self = Self(Cow::Borrowed("required_collateral"));

    /// Collateral coverage ratio
    pub const CollateralCoverage: Self = Self(Cow::Borrowed("collateral_coverage"));

    /// Repo interest amount
    pub const RepoInterest: Self = Self(Cow::Borrowed("repo_interest"));

    /// Funding risk (repo rate sensitivity)
    pub const FundingRisk: Self = Self(Cow::Borrowed("funding_risk"));

    /// Effective repo rate (adjusted for special collateral)
    pub const EffectiveRate: Self = Self(Cow::Borrowed("effective_rate"));

    /// Time to maturity in years
    pub const TimeToMaturity: Self = Self(Cow::Borrowed("time_to_maturity"));

    /// Implied collateral return
    pub const ImpliedCollateralReturn: Self = Self(Cow::Borrowed("implied_collateral_return"));

    // ========================================================================
    // Basket/ETF Metrics
    // ========================================================================

    /// Net Asset Value per share
    pub const Nav: Self = Self(Cow::Borrowed("nav"));

    /// Total basket value
    pub const BasketValue: Self = Self(Cow::Borrowed("basket_value"));

    /// Number of constituents in the basket
    pub const ConstituentCount: Self = Self(Cow::Borrowed("constituent_count"));

    /// Expense ratio as percentage
    pub const ExpenseRatio: Self = Self(Cow::Borrowed("expense_ratio"));

    /// Tracking error vs benchmark
    pub const TrackingError: Self = Self(Cow::Borrowed("tracking_error"));

    /// Utilization vs creation unit size
    pub const Utilization: Self = Self(Cow::Borrowed("utilization"));

    /// Premium/discount to NAV
    pub const PremiumDiscount: Self = Self(Cow::Borrowed("premium_discount"));

    // ========================================================================
    // Structured Credit Metrics
    // ========================================================================

    /// Weighted Average Life (WAL), the expected principal repayment life.
    ///
    /// Units: years.
    pub const WAL: Self = Self(Cow::Borrowed("wal"));

    /// Weighted Average Maturity (WAM) of the underlying pool.
    ///
    /// Units: years.
    pub const WAM: Self = Self(Cow::Borrowed("wam"));

    /// Expected final payment date under base assumptions
    pub const ExpectedMaturity: Self = Self(Cow::Borrowed("expected_maturity"));

    /// Percentage of original pool balance remaining.
    ///
    /// Units: decimal fraction of original balance (`0.65 = 65%` remaining).
    pub const PoolFactor: Self = Self(Cow::Borrowed("pool_factor"));

    /// Constant Prepayment Rate (CPR), annualized.
    ///
    /// Units: decimal annual prepayment rate.
    pub const CPR: Self = Self(Cow::Borrowed("cpr"));

    /// Single Monthly Mortality (SMM), monthly prepayment rate.
    ///
    /// Units: decimal monthly rate.
    pub const SMM: Self = Self(Cow::Borrowed("smm"));

    /// Constant Default Rate (CDR), annualized.
    ///
    /// Units: decimal annual default rate.
    pub const CDR: Self = Self(Cow::Borrowed("cdr"));

    /// Loss severity, usually `1 - recovery_rate`.
    ///
    /// Units: decimal loss fraction.
    pub const LossSeverity: Self = Self(Cow::Borrowed("loss_severity"));

    /// Spread duration, a time-weighted sensitivity to spread changes.
    ///
    /// Units: years.
    pub const SpreadDuration: Self = Self(Cow::Borrowed("spread_duration"));

    /// DM01, discount-margin sensitivity for floating-rate structured credit.
    ///
    /// Units: currency per 1bp discount-margin move.
    pub const Dm01: Self = Self(Cow::Borrowed("dm01"));

    // ========================================================================
    // ABS-specific Metrics
    // ========================================================================

    /// Delinquency rate - Percentage of pool in delinquency
    pub const AbsDelinquency: Self = Self(Cow::Borrowed("abs_delinquency"));

    /// Charge-off rate - Percentage of pool charged off
    pub const AbsChargeOff: Self = Self(Cow::Borrowed("abs_charge_off"));

    /// Excess spread - Spread available to absorb losses
    pub const AbsExcessSpread: Self = Self(Cow::Borrowed("abs_excess_spread"));

    /// Credit enhancement level - Subordination as % of pool
    pub const AbsCreditEnhancement: Self = Self(Cow::Borrowed("abs_ce_level"));

    // ========================================================================
    // CLO-specific Metrics
    // ========================================================================

    /// Weighted Average Rating Factor
    pub const CloWarf: Self = Self(Cow::Borrowed("clo_warf"));

    /// Weighted Average Spread
    pub const CloWas: Self = Self(Cow::Borrowed("clo_was"));

    /// Weighted Average Coupon
    pub const CloWac: Self = Self(Cow::Borrowed("clo_wac"));

    /// Portfolio diversity score
    pub const CloDiversity: Self = Self(Cow::Borrowed("clo_diversity"));

    /// Overcollateralization ratio
    pub const CloOcRatio: Self = Self(Cow::Borrowed("clo_oc_ratio"));

    /// Interest coverage ratio
    pub const CloIcRatio: Self = Self(Cow::Borrowed("clo_ic_ratio"));

    /// Average recovery rate on defaults
    pub const CloRecoveryRate: Self = Self(Cow::Borrowed("clo_recovery_rate"));

    // ========================================================================
    // CMBS-specific Metrics
    // ========================================================================

    /// Debt Service Coverage Ratio
    pub const CmbsDscr: Self = Self(Cow::Borrowed("cmbs_dscr"));

    /// Weighted Average Loan-to-Value
    pub const CmbsWaltv: Self = Self(Cow::Borrowed("cmbs_waltv"));

    /// Credit Enhancement Level
    pub const CmbsCreditEnhancement: Self = Self(Cow::Borrowed("cmbs_ce_level"));

    // ========================================================================
    // RMBS-specific Metrics
    // ========================================================================

    /// PSA prepayment speed (e.g., 100% PSA)
    pub const RmbsPsaSpeed: Self = Self(Cow::Borrowed("rmbs_psa_speed"));

    /// SDA default speed
    pub const RmbsSdaSpeed: Self = Self(Cow::Borrowed("rmbs_sda_speed"));

    /// Weighted Average LTV for RMBS
    pub const RmbsWaltv: Self = Self(Cow::Borrowed("rmbs_waltv"));

    /// Weighted Average FICO score
    pub const RmbsWafico: Self = Self(Cow::Borrowed("rmbs_wafico"));

    // ========================================================================
    // Inflation-Linked Bond Metrics
    // ========================================================================

    /// Real yield (inflation-adjusted)
    pub const RealYield: Self = Self(Cow::Borrowed("real_yield"));

    /// Inflation index ratio
    pub const IndexRatio: Self = Self(Cow::Borrowed("index_ratio"));

    /// Real duration (inflation-adjusted duration)
    pub const RealDuration: Self = Self(Cow::Borrowed("real_duration"));

    /// Breakeven inflation rate
    pub const BreakevenInflation: Self = Self(Cow::Borrowed("breakeven_inflation"));

    // ========================================================================
    // Private Equity / Private Markets Fund Metrics
    // ========================================================================

    /// LP (Limited Partner) internal rate of return.
    ///
    /// Units: decimal annualized IRR.
    pub const LpIrr: Self = Self(Cow::Borrowed("lp_irr"));

    /// Total GP (General Partner) carry paid through the waterfall.
    ///
    /// Units: currency (formerly misregistered as
    /// `gp_irr` despite returning a dollar amount).
    pub const GpCarryTotal: Self = Self(Cow::Borrowed("gp_carry_total"));

    /// LP multiple on invested capital.
    ///
    /// Units: ratio multiple (`1.80 = 1.8x`).
    pub const MoicLp: Self = Self(Cow::Borrowed("moic_lp"));

    /// LP distributions to paid-in capital (DPI).
    ///
    /// Units: ratio multiple (`0.75 = 0.75x`).
    pub const DpiLp: Self = Self(Cow::Borrowed("dpi_lp"));

    /// LP total value to paid-in capital (TVPI).
    ///
    /// Units: ratio multiple (`1.40 = 1.4x`).
    pub const TvpiLp: Self = Self(Cow::Borrowed("tvpi_lp"));

    /// Accrued carry amount for the GP.
    ///
    /// Units: currency.
    pub const CarryAccrued: Self = Self(Cow::Borrowed("carry_accrued"));

    // ========================================================================
    // DCF / Corporate Valuation Metrics
    // ========================================================================

    /// Enterprise value (present value of all operating cashflows + terminal value)
    pub const EnterpriseValue: Self = Self(Cow::Borrowed("enterprise_value"));

    /// Equity value (enterprise value less net debt)
    pub const EquityValue: Self = Self(Cow::Borrowed("equity_value"));

    /// Present value of terminal value
    pub const TerminalValuePV: Self = Self(Cow::Borrowed("terminal_value_pv"));

    // ========================================================================
    // VaR Metrics
    // ========================================================================

    /// Conditional second-order theta (gamma of theta)
    pub const ThetaGamma: Self = Self(Cow::Borrowed("theta_gamma"));

    /// Historical Value-at-Risk (95% confidence by default)
    pub const HVar: Self = Self(Cow::Borrowed("hvar"));

    /// Expected Shortfall / Conditional VaR (CVaR)
    pub const ExpectedShortfall: Self = Self(Cow::Borrowed("expected_shortfall"));

    // ========================================================================
    // Dollar Roll / TBA Carry Metrics
    // ========================================================================

    /// Implied financing rate from dollar roll drop (annualized, ACT/360).
    pub const ImpliedFinancingRate: Self = Self(Cow::Borrowed("implied_financing_rate"));

    /// Roll specialness vs. repo rate (basis points).
    pub const RollSpecialness: Self = Self(Cow::Borrowed("roll_specialness"));

    // ========================================================================
    // Pricer registry: spread / yield metrics on cash-equivalent cashflows
    // ========================================================================

    /// Metrics computed on **cash-equivalent** cashflows when pricing with a
    /// non-discounting model (hazard, tree, Monte Carlo, etc.).
    ///
    /// This list must stay aligned with the spread/yield split in
    /// [`crate::pricer::PricerRegistry::price_with_metrics`].
    pub const SPREAD_EQUIVALENT_METRICS: &'static [MetricId] = &[
        MetricId::Ytm,
        MetricId::Ytw,
        MetricId::ZSpread,
        MetricId::ISpread,
        MetricId::DiscountMargin,
        MetricId::Oas,
        MetricId::ASWPar,
        MetricId::ASWMarket,
        MetricId::CleanPrice,
        MetricId::DirtyPrice,
        MetricId::Accrued,
        MetricId::EmbeddedOptionValue,
    ];

    // ========================================================================
    // ALL_STANDARD Array
    // ========================================================================

    /// All standard (non-custom) metric IDs, ordered by group.
    pub const ALL_STANDARD: &'static [MetricId] = &[
        // -- Pricing --
        MetricId::DirtyPrice,
        MetricId::CleanPrice,
        MetricId::ConversionFactor,
        MetricId::Accrued,
        MetricId::Ytm,
        MetricId::Ytw,
        MetricId::ZSpread,
        MetricId::Oas,
        MetricId::ISpread,
        MetricId::GSpread,
        MetricId::ASWPar,
        MetricId::ASWMarket,
        MetricId::DiscountMargin,
        MetricId::EmbeddedOptionValue,
        MetricId::DurationMac,
        MetricId::DurationMod,
        MetricId::RealDuration,
        MetricId::YieldDv01,
        MetricId::Convexity,
        MetricId::ImpliedVol,
        MetricId::TimeToMaturity,
        MetricId::FuturesPrice,
        MetricId::Basis,
        // -- Carry --
        MetricId::Theta,
        MetricId::ThetaCarry,
        MetricId::ThetaRollDown,
        MetricId::CarryTotal,
        MetricId::CouponIncome,
        MetricId::PullToPar,
        MetricId::RollDown,
        MetricId::FundingCost,
        MetricId::ImpliedFinancingRate,
        MetricId::RollSpecialness,
        MetricId::Breakeven,
        // -- Sensitivity --
        MetricId::Dv01,
        MetricId::BucketedDv01,
        MetricId::DurationDv01,
        MetricId::Pv01,
        MetricId::ForwardPv01,
        MetricId::Npv01,
        MetricId::Rho,
        MetricId::ForeignRho,
        MetricId::Dv01Domestic,
        MetricId::Dv01Foreign,
        MetricId::Dv01Primary,
        MetricId::Dv01Reference,
        MetricId::Dividend01,
        MetricId::Inflation01,
        MetricId::Dm01,
        MetricId::Conversion01,
        MetricId::CollateralHaircut01,
        MetricId::CollateralPrice01,
        MetricId::ConvexityAdjustmentRisk,
        // -- Greeks --
        MetricId::Delta,
        MetricId::DeltaForward,
        MetricId::DeltaPremiumAdjusted,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::HwSigmaVega,
        MetricId::BucketedVega,
        MetricId::Vanna,
        MetricId::Volga,
        MetricId::Veta,
        MetricId::Charm,
        MetricId::Color,
        MetricId::Speed,
        MetricId::IrConvexity,
        MetricId::IrCrossGamma,
        MetricId::InflationConvexity,
        MetricId::CsGamma,
        MetricId::CrossGammaRatesCredit,
        MetricId::CrossGammaRatesVol,
        MetricId::CrossGammaSpotVol,
        MetricId::CrossGammaSpotCredit,
        MetricId::CrossGammaFxVol,
        MetricId::CrossGammaFxRates,
        MetricId::ThetaGamma,
        MetricId::VarianceVega,
        // -- Credit --
        MetricId::Cs01,
        MetricId::BucketedCs01,
        MetricId::Cs01Hazard,
        MetricId::BucketedCs01Hazard,
        MetricId::ParSpread,
        MetricId::RiskyPv01,
        MetricId::RiskyAnnuity,
        MetricId::SpreadDv01,
        MetricId::Correlation01,
        MetricId::Default01,
        MetricId::ProtectionLegPv,
        MetricId::PremiumLegPv,
        MetricId::JumpToDefault,
        MetricId::DefaultExposure,
        MetricId::ExpectedLoss,
        MetricId::DefaultProbability,
        MetricId::Recovery01,
        // -- Rates --
        MetricId::Annuity,
        MetricId::ParRate,
        MetricId::PvFixed,
        MetricId::PvFloat,
        MetricId::PvPrimary,
        MetricId::PvReference,
        MetricId::AnnuityPrimary,
        MetricId::AnnuityReference,
        MetricId::BasisParSpread,
        MetricId::IncrementalParSpread,
        MetricId::FinancingAnnuity,
        MetricId::IndexDelta,
        MetricId::Yf,
        MetricId::DfStart,
        MetricId::DfEnd,
        MetricId::DepositParRate,
        MetricId::DfEndFromQuote,
        MetricId::QuoteRate,
        MetricId::ImpliedForward,
        MetricId::ConvexityAdjustment,
        MetricId::FixedLegPaymentCount,
        MetricId::FloatingLegPaymentCount,
        MetricId::FixedFirstPaymentDate,
        MetricId::FixedLastPaymentDate,
        MetricId::FloatingFirstPaymentDate,
        MetricId::FloatingLastPaymentDate,
        MetricId::FixedFirstAccrualFactor,
        MetricId::FloatingFirstAccrualFactor,
        // -- FX --
        MetricId::SpotRate,
        MetricId::BaseAmount,
        MetricId::QuoteAmount,
        MetricId::InverseRate,
        MetricId::Fx01,
        MetricId::FxDelta,
        MetricId::FxVega,
        // -- Equity --
        MetricId::EquityPricePerShare,
        MetricId::EquityShares,
        MetricId::EquityDividendYield,
        MetricId::EquityForwardPrice,
        MetricId::DeltaVol,
        MetricId::ConstituentDelta,
        MetricId::Nav,
        MetricId::BasketValue,
        MetricId::ConstituentCount,
        MetricId::ExpenseRatio,
        MetricId::TrackingError,
        MetricId::Utilization,
        MetricId::PremiumDiscount,
        MetricId::ExpectedVariance,
        MetricId::RealizedVariance,
        MetricId::VarianceNotional,
        MetricId::VarianceStrikeVol,
        MetricId::VarianceTimeToMaturity,
        // -- Structured Credit --
        MetricId::WAL,
        MetricId::WAM,
        MetricId::ExpectedMaturity,
        MetricId::PoolFactor,
        MetricId::CPR,
        MetricId::SMM,
        MetricId::CDR,
        MetricId::LossSeverity,
        MetricId::SpreadDuration,
        MetricId::Prepayment01,
        MetricId::Severity01,
        MetricId::AbsDelinquency,
        MetricId::AbsChargeOff,
        MetricId::AbsExcessSpread,
        MetricId::AbsCreditEnhancement,
        MetricId::CloWarf,
        MetricId::CloWas,
        MetricId::CloWac,
        MetricId::CloDiversity,
        MetricId::CloOcRatio,
        MetricId::CloIcRatio,
        MetricId::CloRecoveryRate,
        MetricId::CmbsDscr,
        MetricId::CmbsWaltv,
        MetricId::CmbsCreditEnhancement,
        MetricId::RmbsPsaSpeed,
        MetricId::RmbsSdaSpeed,
        MetricId::RmbsWaltv,
        MetricId::RmbsWafico,
        // -- Alternatives --
        MetricId::RealYield,
        MetricId::IndexRatio,
        MetricId::BreakevenInflation,
        MetricId::LpIrr,
        MetricId::GpCarryTotal,
        MetricId::MoicLp,
        MetricId::DpiLp,
        MetricId::TvpiLp,
        MetricId::CarryAccrued,
        MetricId::Nav01,
        MetricId::Carry01,
        MetricId::Hurdle01,
        MetricId::EnterpriseValue,
        MetricId::EquityValue,
        MetricId::TerminalValuePV,
        MetricId::CollateralValue,
        MetricId::RequiredCollateral,
        MetricId::CollateralCoverage,
        MetricId::RepoInterest,
        MetricId::FundingRisk,
        MetricId::EffectiveRate,
        MetricId::ImpliedCollateralReturn,
        MetricId::HVar,
        MetricId::ExpectedShortfall,
    ];
}

// ============================================================================
// Metric Groups
// ============================================================================

/// Logical grouping of standard metrics for discovery and display.
///
/// Each standard metric belongs to exactly one group. Use
/// [`MetricGroup::metrics()`] to list members and
/// [`MetricGroup::ALL`] to iterate all groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum MetricGroup {
    /// Static pricing outputs: prices, yields, spreads, durations, implied
    /// levels, convexity, embedded option value.
    Pricing,
    /// Time-driven P&L: theta decomposition, carry components, financing,
    /// dollar-roll carry.
    Carry,
    /// First-order bump sensitivities to market curves: DV01, PV01,
    /// bucketed DV01, rho, and other rates-focused "01" metrics.
    Sensitivity,
    /// Options-style Greeks and all second-order / higher-order
    /// sensitivities: delta, gamma, vega, cross-gammas, variance vega.
    Greeks,
    /// CDS/credit analytics and credit-specific sensitivities: CS01,
    /// bucketed CS01, par spread, risky PV01/annuity, spread DV01,
    /// correlation01, default metrics, recovery.
    Credit,
    /// Rates instrument decomposition: IRS legs, annuities, par rates,
    /// basis swap, TRS, deposit/calibration intermediates.
    Rates,
    /// FX instrument pricing and analytics: spot rates, amounts, FX
    /// sensitivities (FX01, FX delta, FX vega).
    Fx,
    /// Equity/basket/ETF pricing, equity-derivative analytics, and
    /// variance swap pricing outputs.
    Equity,
    /// Securitization pool and tranche analytics: WAL, WAM, CPR, CDR,
    /// prepayment/severity sensitivities, ABS/CLO/CMBS/RMBS specifics.
    StructuredCredit,
    /// PE fund metrics, DCF valuation, repo analytics,
    /// inflation-linked bond metrics, VaR.
    Alternatives,
}

impl MetricGroup {
    /// All groups in display order.
    pub const ALL: &'static [MetricGroup] = &[
        MetricGroup::Pricing,
        MetricGroup::Carry,
        MetricGroup::Sensitivity,
        MetricGroup::Greeks,
        MetricGroup::Credit,
        MetricGroup::Rates,
        MetricGroup::Fx,
        MetricGroup::Equity,
        MetricGroup::StructuredCredit,
        MetricGroup::Alternatives,
    ];

    /// Human-readable group name.
    pub const fn display_name(&self) -> &'static str {
        match self {
            MetricGroup::Pricing => "Pricing",
            MetricGroup::Carry => "Carry",
            MetricGroup::Sensitivity => "Sensitivity",
            MetricGroup::Greeks => "Greeks",
            MetricGroup::Credit => "Credit",
            MetricGroup::Rates => "Rates",
            MetricGroup::Fx => "FX",
            MetricGroup::Equity => "Equity",
            MetricGroup::StructuredCredit => "Structured Credit",
            MetricGroup::Alternatives => "Alternatives",
        }
    }

    /// Standard metrics belonging to this group.
    pub fn metrics(&self) -> &'static [MetricId] {
        let (start, end) = self.metric_range();
        &MetricId::ALL_STANDARD[start..end]
    }

    /// All groups with their metrics, for iteration.
    pub fn all_with_metrics() -> &'static [(MetricGroup, &'static [MetricId])] {
        static DATA: OnceLock<Vec<(MetricGroup, &'static [MetricId])>> = OnceLock::new();
        DATA.get_or_init(|| MetricGroup::ALL.iter().map(|g| (*g, g.metrics())).collect())
    }

    fn metric_range(&self) -> (usize, usize) {
        match self {
            MetricGroup::Pricing => (0, 23),
            MetricGroup::Carry => (23, 34),
            MetricGroup::Sensitivity => (34, 53),
            MetricGroup::Greeks => (53, 77),
            MetricGroup::Credit => (77, 94),
            MetricGroup::Rates => (94, 122),
            MetricGroup::Fx => (122, 129),
            MetricGroup::Equity => (129, 147),
            MetricGroup::StructuredCredit => (147, 176),
            MetricGroup::Alternatives => (176, 201),
        }
    }
}

impl fmt::Display for MetricGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ============================================================================

impl fmt::Display for MetricId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Borrow<str> for MetricId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

// Lazy lookup table for FromStr
static METRIC_LOOKUP: OnceLock<HashMap<String, MetricId>> = OnceLock::new();

fn metric_lookup() -> &'static HashMap<String, MetricId> {
    METRIC_LOOKUP.get_or_init(|| {
        let mut map = HashMap::default();
        map.reserve(MetricId::ALL_STANDARD.len());
        for m in MetricId::ALL_STANDARD {
            // Names are already lower snake_case
            map.insert(m.as_str().to_string(), m.clone());
        }
        map
    })
}

impl FromStr for MetricId {
    type Err = (); // Never fails since we have a catch-all Custom variant

    /// Parses a string into a MetricId (permissive mode).
    ///
    /// This method never fails - any unrecognized string becomes a custom metric.
    /// Standard metrics are matched case-insensitively in snake_case format.
    ///
    /// **For user-provided inputs**, prefer `MetricId::parse_strict()` which
    /// rejects unknown metrics instead of silently creating custom metrics.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_valuations::metrics::MetricId;
    /// use std::str::FromStr;
    ///
    /// // Known metric - parsed as standard
    /// let dv01 = MetricId::from_str("dv01").unwrap();
    /// assert_eq!(dv01, MetricId::Dv01);
    /// assert!(!dv01.is_custom());
    ///
    /// // Unknown metric - becomes custom (no error)
    /// let custom = MetricId::from_str("my_metric").unwrap();
    /// assert!(custom.is_custom());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        if let Some(id) = metric_lookup().get(&lower) {
            Ok(id.clone())
        } else {
            Ok(MetricId::custom(lower))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_strict_known_metric() {
        // Test lowercase
        let dv01 = MetricId::parse_strict("dv01").unwrap();
        assert_eq!(dv01, MetricId::Dv01);
        assert!(!dv01.is_custom());

        // Test uppercase (case insensitive)
        let theta = MetricId::parse_strict("THETA").unwrap();
        assert_eq!(theta, MetricId::Theta);
        assert!(!theta.is_custom());

        // Test mixed case
        let cs01 = MetricId::parse_strict("Cs01").unwrap();
        assert_eq!(cs01, MetricId::Cs01);
        assert!(!cs01.is_custom());

        // Test various standard metrics
        let delta = MetricId::parse_strict("delta").unwrap();
        assert_eq!(delta, MetricId::Delta);

        let ytm = MetricId::parse_strict("ytm").unwrap();
        assert_eq!(ytm, MetricId::Ytm);

        let convexity = MetricId::parse_strict("convexity").unwrap();
        assert_eq!(convexity, MetricId::Convexity);
    }

    #[test]
    fn test_parse_strict_unknown_metric() {
        // Unknown metric should fail
        let result = MetricId::parse_strict("dv01x");
        assert!(result.is_err());

        // Check error contains metric name
        let err = result.unwrap_err();
        let err_msg = format!("{}", err);
        assert!(err_msg.to_lowercase().contains("dv01x"));

        // Test other typos
        assert!(MetricId::parse_strict("theta2").is_err());
        assert!(MetricId::parse_strict("cs_01").is_err());
        assert!(MetricId::parse_strict("unknown_metric").is_err());
    }

    #[test]
    fn test_parse_strict_error_includes_available_metrics() {
        let result = MetricId::parse_strict("invalid_metric");
        assert!(result.is_err());

        // The error should be UnknownMetric variant
        match result.unwrap_err() {
            finstack_quant_core::Error::UnknownMetric {
                metric_id,
                available,
            } => {
                assert_eq!(metric_id, "invalid_metric");
                // Should include standard metrics
                assert!(!available.is_empty());
                assert!(available.contains(&"dv01".to_string()));
                assert!(available.contains(&"theta".to_string()));
                assert!(available.contains(&"cs01".to_string()));
            }
            _ => panic!("Expected UnknownMetric error"),
        }
    }

    #[test]
    fn test_from_str_still_permissive() {
        // FromStr should still accept unknown metrics
        let known = MetricId::from_str("dv01").unwrap();
        assert_eq!(known, MetricId::Dv01);
        assert!(!known.is_custom());

        // Unknown metric becomes custom (no error)
        let custom = MetricId::from_str("my_custom_metric").unwrap();
        assert!(custom.is_custom());
        assert_eq!(custom.as_str(), "my_custom_metric");

        // Another unknown metric
        let custom2 = MetricId::from_str("user_defined_123").unwrap();
        assert!(custom2.is_custom());
    }

    #[test]
    fn test_parse_strict_vs_from_str_behavior() {
        // Known metric: both work the same
        let strict = MetricId::parse_strict("theta").unwrap();
        let permissive = MetricId::from_str("theta").unwrap();
        assert_eq!(strict, permissive);

        // Unknown metric: strict fails, permissive creates custom
        let strict_result = MetricId::parse_strict("custom_metric");
        assert!(strict_result.is_err());

        let permissive_result = MetricId::from_str("custom_metric").unwrap();
        assert!(permissive_result.is_custom());
    }

    #[test]
    fn test_custom_metric_creation() {
        let custom = MetricId::custom("my_metric");
        assert!(custom.is_custom());
        assert_eq!(custom.as_str(), "my_metric");

        // Custom metrics not in ALL_STANDARD
        assert!(!MetricId::ALL_STANDARD.contains(&custom));
    }

    #[test]
    fn test_all_standard_metrics_parseable_strict() {
        // Every standard metric should be parseable via parse_strict
        for metric in MetricId::ALL_STANDARD {
            let parsed = MetricId::parse_strict(metric.as_str()).unwrap();
            assert_eq!(&parsed, metric);
            assert!(!parsed.is_custom());
        }
    }

    #[test]
    fn test_carry_decomposition_metrics_are_standard_and_parseable() {
        for name in [
            "carry_total",
            "coupon_income",
            "pull_to_par",
            "roll_down",
            "funding_cost",
        ] {
            assert!(MetricId::ALL_STANDARD
                .iter()
                .any(|metric| metric.as_str() == name));

            let parsed = MetricId::parse_strict(name).unwrap();
            assert_eq!(parsed.as_str(), name);
            assert!(!parsed.is_custom());
        }
    }

    #[test]
    fn spread_equivalent_metrics_are_unique_and_standard() {
        let mut seen = std::collections::HashSet::new();
        for m in MetricId::SPREAD_EQUIVALENT_METRICS {
            assert!(
                seen.insert(m.as_str()),
                "duplicate spread-equivalent metric: {}",
                m.as_str()
            );
            assert!(
                !m.is_custom(),
                "spread-equivalent metric must be standard: {}",
                m.as_str()
            );
            assert!(
                MetricId::ALL_STANDARD.contains(m),
                "spread-equivalent metric missing from ALL_STANDARD: {}",
                m.as_str()
            );
        }
    }

    #[test]
    fn test_cross_gamma_metric_ids_exist_and_parse() {
        let pairs = [
            (MetricId::CrossGammaRatesCredit, "cross_gamma_rates_credit"),
            (MetricId::CrossGammaRatesVol, "cross_gamma_rates_vol"),
            (MetricId::CrossGammaSpotVol, "cross_gamma_spot_vol"),
            (MetricId::CrossGammaSpotCredit, "cross_gamma_spot_credit"),
            (MetricId::CrossGammaFxVol, "cross_gamma_fx_vol"),
            (MetricId::CrossGammaFxRates, "cross_gamma_fx_rates"),
        ];
        for (id, expected_str) in &pairs {
            assert_eq!(id.as_str(), *expected_str);
            let parsed = MetricId::parse_strict(expected_str).unwrap();
            assert_eq!(&parsed, id);
            assert!(!parsed.is_custom());
        }
    }

    #[test]
    fn test_case_insensitivity() {
        // Strict parsing is case insensitive
        let lower = MetricId::parse_strict("dv01").unwrap();
        let upper = MetricId::parse_strict("DV01").unwrap();
        let mixed = MetricId::parse_strict("Dv01").unwrap();

        assert_eq!(lower, upper);
        assert_eq!(lower, mixed);
        assert_eq!(lower, MetricId::Dv01);
    }

    #[test]
    fn test_every_standard_metric_in_exactly_one_group() {
        let mut grouped: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for group in MetricGroup::ALL {
            for metric in group.metrics() {
                assert!(
                    grouped.insert(metric.as_str()),
                    "metric '{}' appears in multiple groups (duplicate found in {:?})",
                    metric.as_str(),
                    group,
                );
            }
        }
        for metric in MetricId::ALL_STANDARD {
            assert!(
                grouped.contains(metric.as_str()),
                "metric '{}' from ALL_STANDARD is not assigned to any MetricGroup",
                metric.as_str(),
            );
        }
    }

    #[test]
    fn test_group_union_equals_all_standard() {
        let mut from_groups: Vec<&str> = MetricGroup::ALL
            .iter()
            .flat_map(|g| g.metrics().iter().map(|m| m.as_str()))
            .collect();
        from_groups.sort();
        let mut from_all: Vec<&str> = MetricId::ALL_STANDARD.iter().map(|m| m.as_str()).collect();
        from_all.sort();
        assert_eq!(
            from_groups, from_all,
            "union of all MetricGroup arrays must equal ALL_STANDARD"
        );
    }

    #[test]
    fn test_metric_group_all_with_metrics() {
        let grouped = MetricGroup::all_with_metrics();
        assert_eq!(grouped.len(), MetricGroup::ALL.len());
        for (group, metrics) in grouped {
            assert!(!metrics.is_empty(), "group {:?} has no metrics", group);
        }
    }
}
