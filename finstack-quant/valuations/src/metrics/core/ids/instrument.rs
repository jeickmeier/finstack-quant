use super::MetricId;
use std::borrow::Cow;

#[allow(non_upper_case_globals)] // PascalCase names for metric ID constants
impl MetricId {
    // Bond Metrics

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

    /// Japanese simple yield (単利) using ACT/365F remaining life.
    pub const JapaneseSimpleYield: Self = Self(Cow::Borrowed("japanese_simple_yield"));

    /// Moosmüller yield to maturity (simple first period, then periodic).
    pub const MoosmullerYtm: Self = Self(Cow::Borrowed("moosmuller_ytm"));

    /// Money multiple (MOIC) to maturity: total distributions / invested capital.
    pub const Moic: Self = Self(Cow::Borrowed("moic"));

    /// Minimum MOIC across exit paths (guarantee check).
    pub const MoicToWorst: Self = Self(Cow::Borrowed("moic_to_worst"));

    /// XIRR to maturity over realized cashflows.
    pub const Xirr: Self = Self(Cow::Borrowed("xirr"));

    /// Minimum XIRR across exit paths (guarantee check).
    pub const XirrToWorst: Self = Self(Cow::Borrowed("xirr_to_worst"));

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

    // Spread Metrics

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

    /// Discount margin for floating-rate bonds (decimal; 0.01 = 100 bp)
    pub const DiscountMargin: Self = Self(Cow::Borrowed("discount_margin"));

    /// Par asset swap spread (market-standard ASW quote)
    pub const ASWPar: Self = Self(Cow::Borrowed("asw_par"));

    /// Market (price-based) asset swap spread
    pub const ASWMarket: Self = Self(Cow::Borrowed("asw_market"));

    // Basket/ETF Metrics

    /// Number of constituents in the basket
    pub const ConstituentCount: Self = Self(Cow::Borrowed("constituent_count"));

    /// Expense ratio as percentage
    pub const ExpenseRatio: Self = Self(Cow::Borrowed("expense_ratio"));

    // Inflation-Linked Bond Metrics

    /// Real yield (inflation-adjusted)
    pub const RealYield: Self = Self(Cow::Borrowed("real_yield"));

    /// Inflation index ratio
    pub const IndexRatio: Self = Self(Cow::Borrowed("index_ratio"));

    /// Real duration (inflation-adjusted duration)
    pub const RealDuration: Self = Self(Cow::Borrowed("real_duration"));

    /// Breakeven inflation rate
    pub const BreakevenInflation: Self = Self(Cow::Borrowed("breakeven_inflation"));

    // Private Equity / Private Markets Fund Metrics

    /// LP (Limited Partner) internal rate of return.
    ///
    /// Units: decimal annualized IRR.
    pub const LpIrr: Self = Self(Cow::Borrowed("lp_irr"));

    /// Total GP (General Partner) carry paid through the waterfall.
    ///
    /// Units: currency (formerly misregistered as
    /// `gp_irr` despite returning a dollar amount).
    pub const GpCarryTotal: Self = Self(Cow::Borrowed("gp_carry_total"));

    /// LP multiple on invested capital, net of GP carry:
    /// `(realized LP distributions + residual value) / paid-in capital`.
    /// On this net LP basis MOIC equals TVPI by definition.
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

    // DCF / Corporate Valuation Metrics

    /// Enterprise value (present value of all operating cashflows + terminal value)
    pub const EnterpriseValue: Self = Self(Cow::Borrowed("enterprise_value"));

    /// Equity value (enterprise value less net debt)
    pub const EquityValue: Self = Self(Cow::Borrowed("equity_value"));

    /// Present value of terminal value
    pub const TerminalValuePV: Self = Self(Cow::Borrowed("terminal_value_pv"));

    // VaR Metrics

    /// Historical Value-at-Risk (95% confidence by default)
    pub const HVar: Self = Self(Cow::Borrowed("hvar"));

    /// Expected Shortfall / Conditional VaR (CVaR)
    pub const ExpectedShortfall: Self = Self(Cow::Borrowed("expected_shortfall"));

    // Dollar Roll / TBA Carry Metrics

    /// Implied financing rate from dollar roll drop (annualized, ACT/360).
    pub const ImpliedFinancingRate: Self = Self(Cow::Borrowed("implied_financing_rate"));

    /// Roll specialness vs. repo rate (basis points).
    pub const RollSpecialness: Self = Self(Cow::Borrowed("roll_specialness"));
}
