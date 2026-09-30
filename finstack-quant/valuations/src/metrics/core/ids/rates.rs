use super::MetricId;
use std::borrow::Cow;

#[allow(non_upper_case_globals)] // PascalCase names for metric ID constants
impl MetricId {
    // IRS Metrics

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

    // Deposit Metrics
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

    // Bermudan exercise

    /// Expected exercise time of a Bermudan swaption, conditional on exercise.
    ///
    /// Units: years from the valuation date. Computed on the Hull-White tree as
    /// `Σ tᵢ·pᵢ / Σ pᵢ`, where `pᵢ` is the probability of exercising at the
    /// `i`-th exercise date; `0` when the swaption never exercises on the tree.
    pub const ExpectedExerciseTime: Self = Self(Cow::Borrowed("expected_exercise_time"));

    // TRS Metrics

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

    // Basis Swap Metrics

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

    // Repo Metrics

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
}
