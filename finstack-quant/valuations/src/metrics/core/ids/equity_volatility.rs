use super::MetricId;
use std::borrow::Cow;

#[allow(non_upper_case_globals)] // PascalCase names for metric ID constants
impl MetricId {
    // FX Spot Metrics

    /// Spot rate
    pub const SpotRate: Self = Self(Cow::Borrowed("spot_rate"));

    /// Base amount
    pub const BaseAmount: Self = Self(Cow::Borrowed("base_amount"));

    /// Quote amount
    pub const QuoteAmount: Self = Self(Cow::Borrowed("quote_amount"));

    /// Inverse rate
    pub const InverseRate: Self = Self(Cow::Borrowed("inverse_rate"));

    // Equity Metrics

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

    /// Basket PV change for a one-basis-point absolute constituent-weight increase.
    ///
    /// Other weights decrease proportionally to preserve unit total weight.
    /// Units: basket currency per 0.0001 weight move, including trade notional.
    /// Bucket coordinates use constituent IDs, independently of display tickers.
    pub const WeightRisk: Self = Self(Cow::Borrowed("weight_risk"));

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

    // Option Metrics

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

    /// Premium-adjusted spot delta under the instrument's premium currency.
    pub const DeltaPremiumAdjustedSpot: Self = Self(Cow::Borrowed("delta_premium_adjusted_spot"));

    /// Premium-adjusted forward delta under the instrument's premium currency.
    pub const DeltaPremiumAdjustedForward: Self =
        Self(Cow::Borrowed("delta_premium_adjusted_forward"));

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
    /// aggregate number. Default buckets use each actual source expiry and
    /// absolute strike once. Scalar implied-volatility overrides leave the
    /// inactive surface nodes at zero; their exposure remains in [`Self::Vega`].
    ///
    /// Units: currency per 1 vol point at each bucket.
    pub const BucketedVega: Self = Self(Cow::Borrowed("bucketed_vega"));

    /// Parallel active-quote vega minus the sum of raw surface-node vegas.
    ///
    /// Units: currency per 1 vol point. This uncovered residual includes scalar
    /// override exposure and finite-bump nonlinearity; it is never redistributed
    /// into the source-node buckets.
    pub const BucketedVegaResidual: Self = Self(Cow::Borrowed("bucketed_vega_residual"));

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
    /// Units: currency per unit-underlying per **vol point** (0.01 absolute
    /// vol), consistent with `Vega` (per vol point) and `Volga` (per
    /// vol-point squared). All producers — the analytic
    /// FX/equity/quanto/FX-barrier/commodity/CMS providers and
    /// `GenericFdVanna` — normalize the σ axis in vol points; the spot axis
    /// is per unit underlying.
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

    // Cross-Factor Gamma Metrics

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
    /// vol-point and differs by a factor of S₀ / 100 (the spot axes differ:
    /// per unit spot vs per 1 pct-pt spot move).
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

    /// Cross-gamma between credit spreads and implied volatility.
    ///
    /// Mixed second derivative ∂²V / (∂s × ∂σ), normalised to
    /// **basis-point** credit spread moves and **vol-point** volatility moves.
    ///
    /// Produced by `CrossFactorCalculator` (CreditVol pair) via a four-corner
    /// central finite difference whose denominators are:
    /// - credit: `credit_bump_bp`   (e.g. 1.0 per 1 bp credit move)
    /// - vol: `vol_bump_abs × 100`  (e.g. 1.0 per 1 vol-point move)
    ///
    /// Units: currency per (1 bp credit spread move) per (1 vol-point move).
    ///
    /// **Attribution contract**: multiply by `avg_credit_shift_bp` (bp credit
    /// spread change) and `avg_vol_shift_abs` (vol-point change) to obtain the
    /// cross P&L. Material for convertibles, whose credit-vol cross-gamma is
    /// non-trivial (equity vol feeds the conversion option while the credit
    /// curve discounts the bond floor).
    pub const CrossGammaCreditVol: Self = Self(Cow::Borrowed("cross_gamma_credit_vol"));

    /// Credit spread gamma, the second derivative with respect to spreads.
    ///
    /// Units: $ per (decimal spread)². The producer normalises the central
    /// second difference by the DECIMAL bump squared (1bp = 1e-4, divisor
    /// 1e-8), so consumers must square a decimal spread move — NOT a bp move —
    /// when forming `½ × CsGamma × (Δs)²`.
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

    // Variance Swap Metrics

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
}
