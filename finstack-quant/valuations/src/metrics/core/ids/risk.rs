use super::MetricId;
use std::borrow::Cow;

#[allow(non_upper_case_globals)] // PascalCase names for metric ID constants
impl MetricId {
    // Core Risk Metrics

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

    // Other Risk Metrics

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

    /// FX spot rate sensitivity per 1% (percentage point) move of the FX rate
    ///
    /// The producer (`fx01.rs`) uses a ±1% relative spot bump (`BUMP_PCT = 1.0`)
    /// and rescales the central difference to per percentage point — NOT per
    /// basis point.
    pub const Fx01: Self = Self(Cow::Borrowed("fx01"));

    /// NPV sensitivity per basis point (inflation swaps)
    pub const Npv01: Self = Self(Cow::Borrowed("npv01"));

    /// Running coupon sensitivity per basis point (CDS Tranche)
    pub const SpreadDv01: Self = Self(Cow::Borrowed("spread_dv01"));

    /// Listed total-return-future value sensitivity to a +1bp financing-spread move.
    ///
    /// Units: settlement currency per 1bp change in the contract's quoted
    /// annualized financing spread.
    pub const Spread01: Self = Self(Cow::Borrowed("spread01"));

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
}
