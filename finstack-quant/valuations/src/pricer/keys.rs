//! Strongly-typed keys for pricer dispatch.
//!
//! Defines [`InstrumentType`], [`ModelKey`], and [`PricerKey`] — the enum-based
//! identifiers used by the pricing registry to route instruments to the
//! appropriate pricer implementation.

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    strum::EnumIter,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[repr(u16)]
/// Strongly-typed instrument classification for pricer dispatch.
///
/// Each variant represents a distinct instrument type with its own pricing
/// logic and risk characteristics. Used by the pricing registry to route
/// instruments to appropriate pricer implementations.
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum InstrumentType {
    /// Fixed or floating-rate bond (plain vanilla, callable, amortizing).
    Bond = 1,
    /// Credit Default Swap (single-name credit protection).
    #[serde(rename = "credit_default_swap")]
    Cds = 3,
    /// CDS Index (portfolio credit protection, e.g., CDX, iTraxx).
    CdsIndex = 4,
    /// CDS Tranche (mezzanine credit exposure via synthetic CDO).
    CdsTranche = 5,
    /// Option on Credit Default Swap.
    CdsOption = 6,
    /// Interest Rate Swap (fixed-for-floating exchange).
    #[serde(rename = "interest_rate_swap")]
    Irs = 7,
    /// Interest rate cap or floor (portfolio of caplets/floorlets).
    CapFloor = 8,
    /// Option on interest rate swap (European exercise).
    Swaption = 9,
    /// Bermudan swaption (multiple exercise dates).
    BermudanSwaption = 10,
    /// Basis swap (floating-for-floating with spread).
    BasisSwap = 11,
    /// Multi-asset basket (worst-of, best-of, or weighted).
    Basket = 12,
    /// Convertible bond (bond with embedded equity option).
    #[serde(rename = "convertible_bond")]
    Convertible = 13,
    /// Money market deposit (single-period).
    Deposit = 14,
    /// Vanilla equity option (call or put on single stock or index).
    EquityOption = 15,
    /// FX option (Garman-Kohlhagen model).
    FxOption = 16,
    /// FX spot position (currency pair).
    FxSpot = 17,
    /// FX forward or FX swap.
    FxSwap = 18,
    /// Cross-currency swap (multi-currency floating legs).
    XccySwap = 52,
    /// Inflation-linked bond (TIPS, index-linked gilts).
    InflationLinkedBond = 19,
    /// Zero-coupon inflation swap.
    InflationSwap = 20,
    /// Year-on-year inflation swap.
    #[serde(rename = "yoy_inflation_swap")]
    YoYInflationSwap = 53,
    /// Inflation cap/floor (YoY inflation options).
    InflationCapFloor = 66,
    /// Interest rate futures contract.
    InterestRateFuture = 21,
    /// Variance swap (volatility exposure).
    VarianceSwap = 22,
    /// FX variance swap (volatility exposure on FX).
    FxVarianceSwap = 67,
    /// Equity spot position (shares in single stock or fund).
    Equity = 23,
    /// Repurchase agreement (repo or reverse repo).
    Repo = 24,
    /// Forward Rate Agreement (forward-starting interest rate contract).
    #[serde(rename = "forward_rate_agreement")]
    Fra = 25,
    /// Structured credit (ABS, RMBS, CMBS, CLO with tranches and waterfall).
    StructuredCredit = 26,
    /// Private markets fund (PE/credit fund with waterfall).
    PrivateMarketsFund = 30,
    /// Revolving credit facility with drawdown/repayment.
    RevolvingCredit = 31,
    /// Asian option (payoff based on average price).
    AsianOption = 32,
    /// Barrier option (knock-in or knock-out on barrier cross).
    BarrierOption = 33,
    /// Lookback option (payoff based on path extremum).
    LookbackOption = 34,
    /// Quanto option (cross-currency option with quanto adjustment).
    QuantoOption = 35,
    /// Autocallable structured note (early redemption feature).
    Autocallable = 36,
    /// CMS option (constant maturity swap option).
    CmsOption = 37,
    /// CMS swap (constant maturity swap).
    CmsSwap = 77,
    /// Cliquet option (ratchet option with periodic resets).
    CliquetOption = 38,
    /// Range accrual note (coupon accrues when rate in range).
    RangeAccrual = 39,
    /// FX barrier option (FX option with knock-in/out barrier).
    FxBarrierOption = 40,
    /// Term loan (optionally Delayed Draw Term Loan)
    TermLoan = 41,
    /// Discounted Cash Flow (corporate valuation)
    #[serde(rename = "discounted_cash_flow")]
    Dcf = 42,
    /// Real estate asset valuation (DCF or direct cap).
    RealEstateAsset = 69,
    /// Levered real estate equity (asset minus financing).
    LeveredRealEstateEquity = 73,
    /// Equity Total Return Swap.
    #[serde(rename = "trs_equity")]
    EquityTotalReturnSwap = 50,
    /// Fixed Income Index Total Return Swap.
    #[serde(rename = "trs_fixed_income_index")]
    FiIndexTotalReturnSwap = 51,
    /// Bond future (futures on a deliverable bond basket with CTD mechanics).
    BondFuture = 54,
    /// Commodity forward or futures contract.
    CommodityForward = 55,
    /// Commodity swap (fixed-for-floating commodity price exchange).
    CommoditySwap = 56,
    /// Commodity option (option on commodity forward or spot).
    CommodityOption = 68,
    /// Commodity Asian option (option on arithmetic/geometric average of commodity prices).
    CommodityAsianOption = 72,
    /// Commodity swaption (option on a fixed-for-floating commodity swap).
    CommoditySwaption = 74,
    /// Commodity spread option (option on the spread between two commodities).
    CommoditySpreadOption = 75,
    /// Volatility index future (VIX, VXN, VSTOXX).
    VolatilityIndexFuture = 57,
    /// FX forward (outright forward, single exchange at maturity).
    FxForward = 60,
    /// Non-deliverable forward (NDF) for restricted currencies.
    Ndf = 61,
    /// Agency MBS passthrough (FNMA, FHLMC, GNMA pools).
    AgencyMbsPassthrough = 62,
    /// Agency TBA (To-Be-Announced) forward.
    AgencyTba = 63,
    /// Dollar roll (TBA financing trade).
    DollarRoll = 64,
    /// Agency CMO (Collateralized Mortgage Obligation).
    AgencyCmo = 65,
    /// FX digital (binary) option (cash-or-nothing / asset-or-nothing).
    FxDigitalOption = 70,
    /// FX touch option (one-touch / no-touch American binary).
    FxTouchOption = 71,
    /// Target Redemption Note (path-dependent coupon with knockout).
    Tarn = 78,
    /// CMS Spread Option (option on spread between two CMS rates).
    CmsSpreadOption = 80,
    /// Callable Range Accrual (range accrual with Bermudan call).
    CallableRangeAccrual = 81,
    /// Snowball / Inverse Floater structured note.
    Snowball = 82,
    /// Generic resolved basket of signed underlying instrument quantities.
    Composite = 83,
    /// Linear listed future on a single or average observed price.
    CommodityFuture = 84,
    /// Exchange-listed future on a deliverable currency pair.
    FxFuture = 85,
    /// Exchange-listed equity or equity-index future, including fixed-currency quantos.
    EquityFuture = 86,
    /// Listed equity total-return future quoted as an annualized financing spread.
    EquityTotalReturnFuture = 87,
    /// Listed or bilateral option on an interest-rate futures price.
    InterestRateFutureOption = 88,
    /// Exchange-listed option on an equity or equity-index futures contract.
    EquityFutureOption = 89,
    /// Exchange-listed option on an FX futures contract.
    FxFutureOption = 90,
    /// Exchange-listed option on a commodity futures contract.
    CommodityFutureOption = 91,
    /// Exchange-listed option on a volatility-index futures contract.
    VolatilityIndexFutureOption = 92,
    /// Asset-backed facility (warehouse line against a collateral pool).
    AssetBackedFacility = 93,
}

use strum::IntoEnumIterator;

impl InstrumentType {
    /// Canonical snake_case wire identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            InstrumentType::Bond => "bond",
            InstrumentType::Cds => "credit_default_swap",
            InstrumentType::CdsIndex => "cds_index",
            InstrumentType::CdsTranche => "cds_tranche",
            InstrumentType::CdsOption => "cds_option",
            InstrumentType::Irs => "interest_rate_swap",
            InstrumentType::CapFloor => "cap_floor",
            InstrumentType::Swaption => "swaption",
            InstrumentType::BermudanSwaption => "bermudan_swaption",
            InstrumentType::BasisSwap => "basis_swap",
            InstrumentType::Basket => "basket",
            InstrumentType::Convertible => "convertible_bond",
            InstrumentType::Deposit => "deposit",
            InstrumentType::EquityOption => "equity_option",
            InstrumentType::FxOption => "fx_option",
            InstrumentType::FxSpot => "fx_spot",
            InstrumentType::FxSwap => "fx_swap",
            InstrumentType::XccySwap => "xccy_swap",
            InstrumentType::InflationLinkedBond => "inflation_linked_bond",
            InstrumentType::InflationSwap => "inflation_swap",
            InstrumentType::YoYInflationSwap => "yoy_inflation_swap",
            InstrumentType::InflationCapFloor => "inflation_cap_floor",
            InstrumentType::InterestRateFuture => "interest_rate_future",
            InstrumentType::VarianceSwap => "variance_swap",
            InstrumentType::FxVarianceSwap => "fx_variance_swap",
            InstrumentType::Equity => "equity",
            InstrumentType::Repo => "repo",
            InstrumentType::Fra => "forward_rate_agreement",
            InstrumentType::StructuredCredit => "structured_credit",
            InstrumentType::PrivateMarketsFund => "private_markets_fund",
            InstrumentType::RevolvingCredit => "revolving_credit",
            InstrumentType::AssetBackedFacility => "asset_backed_facility",
            InstrumentType::AsianOption => "asian_option",
            InstrumentType::BarrierOption => "barrier_option",
            InstrumentType::LookbackOption => "lookback_option",
            InstrumentType::QuantoOption => "quanto_option",
            InstrumentType::Autocallable => "autocallable",
            InstrumentType::CmsOption => "cms_option",
            InstrumentType::CmsSwap => "cms_swap",
            InstrumentType::CliquetOption => "cliquet_option",
            InstrumentType::RangeAccrual => "range_accrual",
            InstrumentType::FxBarrierOption => "fx_barrier_option",
            InstrumentType::TermLoan => "term_loan",
            InstrumentType::Dcf => "discounted_cash_flow",
            InstrumentType::RealEstateAsset => "real_estate_asset",
            InstrumentType::LeveredRealEstateEquity => "levered_real_estate_equity",
            InstrumentType::EquityTotalReturnSwap => "trs_equity",
            InstrumentType::FiIndexTotalReturnSwap => "trs_fixed_income_index",
            InstrumentType::BondFuture => "bond_future",
            InstrumentType::CommodityForward => "commodity_forward",
            InstrumentType::CommoditySwap => "commodity_swap",
            InstrumentType::CommodityOption => "commodity_option",
            InstrumentType::CommodityAsianOption => "commodity_asian_option",
            InstrumentType::CommoditySwaption => "commodity_swaption",
            InstrumentType::CommoditySpreadOption => "commodity_spread_option",
            InstrumentType::VolatilityIndexFuture => "volatility_index_future",
            InstrumentType::CommodityFuture => "commodity_future",
            InstrumentType::FxFuture => "fx_future",
            InstrumentType::EquityFuture => "equity_future",
            InstrumentType::EquityTotalReturnFuture => "equity_total_return_future",
            InstrumentType::InterestRateFutureOption => "interest_rate_future_option",
            InstrumentType::EquityFutureOption => "equity_future_option",
            InstrumentType::FxFutureOption => "fx_future_option",
            InstrumentType::CommodityFutureOption => "commodity_future_option",
            InstrumentType::VolatilityIndexFutureOption => "volatility_index_future_option",
            InstrumentType::FxForward => "fx_forward",
            InstrumentType::Ndf => "ndf",
            InstrumentType::AgencyMbsPassthrough => "agency_mbs_passthrough",
            InstrumentType::AgencyTba => "agency_tba",
            InstrumentType::DollarRoll => "dollar_roll",
            InstrumentType::AgencyCmo => "agency_cmo",
            InstrumentType::FxDigitalOption => "fx_digital_option",
            InstrumentType::FxTouchOption => "fx_touch_option",
            InstrumentType::Tarn => "tarn",
            InstrumentType::CmsSpreadOption => "cms_spread_option",
            InstrumentType::CallableRangeAccrual => "callable_range_accrual",
            InstrumentType::Snowball => "snowball",
            InstrumentType::Composite => "composite",
        }
    }
}

impl std::fmt::Display for InstrumentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for InstrumentType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::iter()
            .find(|candidate| candidate.as_str() == s)
            .ok_or_else(|| format!("Unknown instrument type: {s}"))
    }
}

/// Pricing model selection for the pricer registry.
///
/// Determines which mathematical model is used to price an instrument.
/// Each model has different computational characteristics and accuracy
/// profiles for different instrument types.
///
/// # Model Categories
///
/// ## Analytical Models
/// - [`Discounting`](Self::Discounting): Simple present value discounting
/// - [`Black76`](Self::Black76): Black-76 formula for options
/// - [`Normal`](Self::Normal): Bachelier (normal) model for rate options
///
/// ## Tree Models
/// - [`Tree`](Self::Tree): Binomial/trinomial lattice
/// - [`HullWhite1F`](Self::HullWhite1F): Hull-White one-factor short rate
///
/// ## Credit Models
/// - [`HazardRate`](Self::HazardRate): Fractional-recovery hazard-rate pricing
/// - [`RatesCredit`](Self::RatesCredit): Joint short-rate and hazard-rate pricing
///
/// ## Monte Carlo Models
/// - [`MonteCarloGBM`](Self::MonteCarloGBM): GBM simulation
/// - [`MonteCarloHeston`](Self::MonteCarloHeston): Heston stochastic vol
/// - [`MonteCarloThreeFactor`](Self::MonteCarloThreeFactor): revolver
///   utilization, short-rate and credit-spread simulation
///
/// ## Exotic Analytical
/// - [`BarrierBSContinuous`](Self::BarrierBSContinuous): Reiner-Rubinstein barriers
/// - [`AsianGeometricBS`](Self::AsianGeometricBS): Geometric Asian (exact)
/// - [`AsianTurnbullWakeman`](Self::AsianTurnbullWakeman): Arithmetic Asian (approx)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::pricer::ModelKey;
///
/// // Select appropriate model for instrument type
/// let model = ModelKey::Discounting;  // For bonds
/// let model = ModelKey::Black76;      // For caps/floors
/// let model = ModelKey::MonteCarloGBM; // For path-dependent exotics
///
/// // Parse from string
/// let model: ModelKey = "black76".parse().unwrap();
/// assert_eq!(model, ModelKey::Black76);
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    strum::EnumIter,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u16)]
pub enum ModelKey {
    /// Present value discounting of projected cashflows.
    ///
    /// Used for: bonds, swaps, deposits, repos, forwards.
    Discounting = 1,
    /// Binomial or trinomial tree for callable/putable instruments.
    ///
    /// Used for: callable bonds, Bermudan options.
    Tree = 2,
    /// Black-76 lognormal model for forward-based options.
    ///
    /// Used for: caps, floors, swaptions, FX options, commodity options.
    Black76 = 3,
    /// Hull-White one-factor short rate model.
    ///
    /// Used for: callable bonds with OAS, Bermudan swaptions.
    #[serde(rename = "hull_white_1f")]
    HullWhite1F = 4,
    /// Hazard rate model for credit instruments.
    ///
    /// Used for: CDS, CDS indices, and non-callable credit-risky bonds.
    HazardRate = 5,
    /// Joint short-rate and hazard-rate model for credit-risky bonds.
    ///
    /// Used for: callable, puttable, return-floor, and straight bonds whose
    /// rates and credit dynamics must be valued on one calibrated path.
    RatesCredit = 7,
    /// Bachelier (normal) model for rate options.
    ///
    /// Used for: inflation caps/floors, options on rates near zero.
    Normal = 6,
    /// Monte Carlo with Geometric Brownian Motion.
    ///
    /// Used for: path-dependent options, exotics requiring simulation.
    #[serde(rename = "monte_carlo_gbm")]
    MonteCarloGBM = 10,
    /// Monte Carlo with Heston stochastic volatility.
    ///
    /// Used for: options requiring volatility smile dynamics.
    MonteCarloHeston = 11,
    /// Monte Carlo with Hull-White 1F rates.
    ///
    /// Used for: Bermudan swaptions, CMS options.
    #[serde(rename = "monte_carlo_hull_white_1f")]
    MonteCarloHullWhite1F = 12,
    /// Monte Carlo with joint utilization, short-rate and credit-spread
    /// factors.
    ///
    /// Used for: revolving credit facilities with a stochastic draw/repay
    /// specification (mean-reverting utilization, a deterministic or
    /// Hull-White short rate, and a market-anchored or CIR credit spread).
    MonteCarloThreeFactor = 13,
    /// Reiner-Rubinstein continuous barrier formulas.
    ///
    /// Used for: equity/FX barrier options with continuous monitoring.
    #[serde(rename = "barrier_bs_continuous")]
    BarrierBSContinuous = 20,
    /// Kemna-Vorst exact geometric Asian formula.
    ///
    /// Used for: geometric average Asian options.
    #[serde(rename = "asian_geometric_bs")]
    AsianGeometricBS = 21,
    /// Turnbull-Wakeman approximation for arithmetic Asians.
    ///
    /// Used for: arithmetic average Asian options.
    AsianTurnbullWakeman = 22,
    /// Conze-Viswanathan lookback option formulas.
    ///
    /// Used for: lookback options with continuous monitoring.
    #[serde(rename = "lookback_bs_continuous")]
    LookbackBSContinuous = 23,
    /// Quanto BS with drift adjustment.
    ///
    /// Used for: quanto options (cross-currency).
    #[serde(rename = "quanto_bs")]
    QuantoBS = 24,
    /// FX barrier with Reiner-Rubinstein mapping.
    ///
    /// Used for: FX barrier options.
    #[serde(rename = "fx_barrier_bs_continuous")]
    FxBarrierBSContinuous = 25,
    /// Heston semi-analytical via Fourier transform.
    ///
    /// Used for: European options requiring stochastic vol.
    HestonFourier = 26,
    /// Merton structural credit Monte Carlo with PIK support.
    ///
    /// Used for: PIK bonds, credit-risky bonds with structural default model.
    MertonMc = 30,
    /// Monte Carlo with Schwartz-Smith two-factor commodity model.
    ///
    /// Used for: commodity options requiring mean-reverting short-term dynamics.
    MonteCarloSchwartzSmith = 31,
    /// Static replication via a portfolio of vanilla options.
    ///
    /// Used for: CMS options (replicates the payoff with a smile-consistent
    /// swaption portfolio, capturing all convexity orders beyond Hagan's
    /// first-order approximation).
    StaticReplication = 32,
    /// LMM/BGM Monte Carlo with predictor-corrector discretization.
    ///
    /// Used for: Bermudan swaptions, exotic rate derivatives requiring
    /// multi-factor forward rate dynamics.
    LmmMonteCarlo = 33,
    /// Structured-credit stochastic scenario waterfall model.
    ///
    /// Used for: ABS/CLO/RMBS/CMBS tranche PVs with path-level waterfall cashflows.
    StructuredCreditStochastic = 34,
    /// Bond future clean-price proxy model.
    ///
    /// Used for: bond futures when only the current CTD clean-price proxy is
    /// available. This is intentionally separate from [`Discounting`](Self::Discounting)
    /// because it does not model delivery carry, invoice economics, or delivery
    /// optionality.
    BondFutureCleanPriceProxy = 35,
    /// Monte Carlo with rough Bergomi (rBergomi) stochastic volatility.
    ///
    /// Used for: European/exotic equity options requiring rough volatility
    /// dynamics with fractional Brownian motion.
    MonteCarloRoughBergomi = 40,
    /// Monte Carlo with rough Heston stochastic volatility.
    ///
    /// Used for: European/exotic equity options requiring rough Heston
    /// dynamics with Volterra-driven variance.
    MonteCarloRoughHeston = 41,
    /// Rough Heston semi-analytical via fractional Riccati Fourier transform.
    ///
    /// Used for: European equity options with rough Heston dynamics,
    /// using Lewis (2000) single-integral Fourier inversion.
    RoughHestonFourier = 42,
    /// 1D Crank-Nicolson finite difference PDE solver.
    ///
    /// Used for: European/American equity options, barrier options,
    /// FX options. Names the scheme family: production pricers run
    /// Rannacher-damped CN (implicit start-up steps, Rannacher 1984),
    /// with a penalty method for early exercise.
    #[serde(rename = "pde_crank_nicolson_1d")]
    PdeCrankNicolson1D = 50,
    /// 2D ADI (Craig-Sneyd) finite difference PDE solver.
    ///
    /// Used for: Heston stochastic volatility, local-stochastic vol,
    /// and other 2D pricing problems. Splits the 2D PDE into
    /// directional tridiagonal sweeps with explicit cross-derivative.
    #[serde(rename = "pde_adi_2d")]
    PdeAdi2D = 51,
    /// Bloomberg CDSO numerical-quadrature model for credit-default-swap
    /// options.
    ///
    /// Reference: Bloomberg L.P. Quantitative Analytics, *Pricing Credit
    /// Index Options*, March 2012 (DOCS 2055833 ⟨GO⟩). The model
    /// preserves the lognormal-spread assumption of the original
    /// Black-on-spreads model but solves `O = P(t_e) · E[(V_te + H(K) +
    /// D)+]` by numerical integration over the lognormal density. The
    /// lognormal mean `m` is calibrated by Brent root finding so that
    /// `F_0 = E[V_te]` (the no-knockout clean forward value); `H(K)` is
    /// the strike-adjustment term `ξN(c − K)A(K)`; `D` is the realized
    /// loss settlement. The closed-form Black-on-spreads model was
    /// decommissioned on the Bloomberg CDSO terminal in 2010.
    ///
    /// Used for: CDS options (`InstrumentType::CdsOption`).
    BloombergCdso = 52,
}

impl ModelKey {
    /// Canonical snake_case wire identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            ModelKey::Discounting => "discounting",
            ModelKey::Tree => "tree",
            ModelKey::Black76 => "black76",
            ModelKey::HullWhite1F => "hull_white_1f",
            ModelKey::HazardRate => "hazard_rate",
            ModelKey::RatesCredit => "rates_credit",
            ModelKey::Normal => "normal",
            ModelKey::MonteCarloGBM => "monte_carlo_gbm",
            ModelKey::MonteCarloHeston => "monte_carlo_heston",
            ModelKey::MonteCarloHullWhite1F => "monte_carlo_hull_white_1f",
            ModelKey::MonteCarloThreeFactor => "monte_carlo_three_factor",
            ModelKey::BarrierBSContinuous => "barrier_bs_continuous",
            ModelKey::AsianGeometricBS => "asian_geometric_bs",
            ModelKey::AsianTurnbullWakeman => "asian_turnbull_wakeman",
            ModelKey::LookbackBSContinuous => "lookback_bs_continuous",
            ModelKey::QuantoBS => "quanto_bs",
            ModelKey::FxBarrierBSContinuous => "fx_barrier_bs_continuous",
            ModelKey::HestonFourier => "heston_fourier",
            ModelKey::MertonMc => "merton_mc",
            ModelKey::MonteCarloSchwartzSmith => "monte_carlo_schwartz_smith",
            ModelKey::StaticReplication => "static_replication",
            ModelKey::LmmMonteCarlo => "lmm_monte_carlo",
            ModelKey::StructuredCreditStochastic => "structured_credit_stochastic",
            ModelKey::BondFutureCleanPriceProxy => "bond_future_clean_price_proxy",
            ModelKey::MonteCarloRoughBergomi => "monte_carlo_rough_bergomi",
            ModelKey::MonteCarloRoughHeston => "monte_carlo_rough_heston",
            ModelKey::RoughHestonFourier => "rough_heston_fourier",
            ModelKey::PdeCrankNicolson1D => "pde_crank_nicolson_1d",
            ModelKey::PdeAdi2D => "pde_adi_2d",
            ModelKey::BloombergCdso => "bloomberg_cdso",
        }
    }
}

impl std::fmt::Display for ModelKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ModelKey {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::iter()
            .find(|candidate| candidate.as_str() == s)
            .ok_or_else(|| format!("Unknown model key: {s}"))
    }
}

/// Composite key identifying a specific (instrument, model) pricing combination.
///
/// The pricer registry uses `PricerKey` to dispatch instruments to the
/// appropriate pricer implementation. Each unique combination of instrument
/// type and pricing model maps to exactly one pricer.
///
/// # Layout
///
/// Uses `#[repr(C)]` for stable memory layout (4 bytes total: 2 for each enum).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::pricer::{PricerKey, InstrumentType, ModelKey};
///
/// // Create key for bond discounting
/// let key = PricerKey::new(InstrumentType::Bond, ModelKey::Discounting);
///
/// // Create key for equity option Black-76
/// let key = PricerKey::new(InstrumentType::EquityOption, ModelKey::Black76);
///
/// // Keys are hashable for registry lookup
/// assert_eq!(key.instrument, InstrumentType::EquityOption);
/// assert_eq!(key.model, ModelKey::Black76);
/// ```
#[repr(C)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PricerKey {
    /// The instrument type being priced.
    pub instrument: InstrumentType,
    /// The pricing model to use.
    pub model: ModelKey,
}

impl PricerKey {
    /// Create a new pricer key from instrument type and model.
    ///
    /// This is a const function, so it can be used in static contexts.
    ///
    /// # Arguments
    ///
    /// * `instrument` - The type of instrument to price
    /// * `model` - The pricing model to use
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::pricer::{PricerKey, InstrumentType, ModelKey};
    ///
    /// const BOND_DISCOUNT_KEY: PricerKey = PricerKey::new(
    ///     InstrumentType::Bond,
    ///     ModelKey::Discounting,
    /// );
    /// ```
    pub const fn new(instrument: InstrumentType, model: ModelKey) -> Self {
        Self { instrument, model }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn abi_is_stable() {
        use core::mem::size_of;
        assert_eq!(size_of::<InstrumentType>(), 2);
        assert_eq!(size_of::<ModelKey>(), 2);
        assert_eq!(size_of::<PricerKey>(), 4);
    }

    #[test]
    fn test_instrument_type_bond_future() {
        let inst_type = InstrumentType::BondFuture;
        assert_eq!(format!("{}", inst_type), "bond_future");
        assert_eq!(inst_type as u16, 54);
    }

    #[test]
    fn instrument_type_display_from_str_roundtrip() {
        for variant in InstrumentType::iter() {
            let s = variant.to_string();
            let parsed: InstrumentType = s
                .parse()
                .unwrap_or_else(|e| panic!("{variant:?} Display=\"{s}\" failed FromStr: {e}"));
            assert_eq!(parsed, variant, "round-trip failed for {variant:?}");
        }
    }

    #[test]
    fn model_key_display_from_str_roundtrip() {
        for variant in ModelKey::iter() {
            let s = variant.to_string();
            let parsed: ModelKey = s
                .parse()
                .unwrap_or_else(|e| panic!("{variant:?} Display=\"{s}\" failed FromStr: {e}"));
            assert_eq!(parsed, variant, "round-trip failed for {variant:?}");
        }
    }

    #[test]
    fn instrument_type_rejects_noncanonical_values() {
        for value in [
            "swap",
            "cds",
            "irs",
            "convertible",
            "fra",
            "dcf",
            "equity_total_return_swap",
            "fi_index_total_return_swap",
            "cdsindex",
            "cross_currency_swap",
            "CdsIndex",
            "bond-future",
            " bond",
        ] {
            assert!(
                value.parse::<InstrumentType>().is_err(),
                "accepted {value:?}"
            );
        }
    }

    #[test]
    fn model_key_rejects_noncanonical_values() {
        for value in [
            "lattice",
            "black_76",
            "hw1f",
            "bachelier",
            "mc_gbm",
            "HullWhite1F",
            "hull-white-1f",
            " hull_white_1f",
        ] {
            assert!(value.parse::<ModelKey>().is_err(), "accepted {value:?}");
        }
    }
}
