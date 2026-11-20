//! Fixed-income Campisi attribution on dated position snapshots.
//!
//! Carry uses cash coupon income on an ACT/365 basis. Treasury and settlement
//! Z-spread moves use opening sensitivities and matched quote bases. Residual
//! contains the differences between observed returns and the three proxies.
//! The benchmark-relative layer uses centered allocation and four active
//! component differences. Exact signed weights and intermediate reconciliation
//! precede projection to binary64.

use crate::attribution::ReconciliationReport;

use crate::error::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
#[cfg(feature = "json-schema")]
use finstack_quant_core::wire::DateWire;
use finstack_quant_core::wire::DecimalWire;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::collections::HashSet;

/// The common return, valuation, FX and fee conventions for the two sides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiReportingBasis {
    /// Identifier of the portfolio realized return series.
    pub portfolio_series_id: String,
    /// Identifier of the benchmark realized return series.
    pub benchmark_series_id: String,
    /// Identifier of the common total-return policy, including reinvestment.
    pub return_policy: String,
    /// Identifier of the common valuation clock, settlement and price basis.
    pub valuation: String,
    /// Identifier of the common FX conversion policy.
    pub fx: String,
    /// Identifier of the common fee treatment.
    pub fees: String,
}

/// Effective interval and common valuation policy for both sides of a Campisi report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiAttributionConfig {
    /// Opening valuation date whose observable holdings and rates set the attribution basis.
    #[cfg_attr(feature = "json-schema", schemars(with = "DateWire"))]
    pub start: Date,
    /// Closing valuation date strictly after the opening date.
    #[cfg_attr(feature = "json-schema", schemars(with = "DateWire"))]
    pub end: Date,
    /// Reporting currency of returns, opening marks and annual cash coupon income.
    pub currency: Currency,
    /// Declared common observation, price, FX and fee basis.
    pub basis: FiReportingBasis,
    /// Unique, explicitly ordered sector labels. Include any cash and financing buckets.
    pub sectors: Vec<String>,
}

/// Observation used to identify an opening or closing yield or spread quote.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiQuote {
    /// Identifier of the quote experiment including the qualified curve, tenor or claim,
    /// clock, compounding, and settlement convention.
    pub basis: String,
    /// Absolute annual decimal yield or spread observed on the request's endpoint date.
    pub rate: f64,
}

/// Matched observations and opening normalized price sensitivity for one Campisi leg.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiQuoteLeg {
    /// Stable matching basis of the shock used to measure opening duration.
    pub sensitivity_basis: String,
    /// Opening normalized sensitivity per decimal annual yield or spread change, in years.
    pub duration: f64,
    /// Opening level of the relevant yield or spread experiment.
    pub opening: FiQuote,
    /// Closing level under the opening experiment's unchanged definition.
    pub closing: FiQuote,
}

/// Opening holding and realized period return for one position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiPositionSnapshot {
    /// Position identifier unique on its side of the report.
    pub id: String,
    /// Sector label on the declared sector axis.
    pub sector: String,
    /// Exact signed fraction of net opening capital, summing to one per side.
    pub weight: DecimalWire,
    /// Realized total return over the request interval on the declared basis, in decimals.
    pub total_return: f64,
    /// Positive magnitude of the opening market value in reporting currency.
    pub opening_market_value: Money,
    /// Opening annual cash coupon income in the same currency as the positive mark.
    pub annual_coupon: Money,
    /// Opening-selected Treasury or government tenor experiment. Omit for an inapplicable leg.
    pub treasury: Option<FiQuoteLeg>,
    /// Qualified settlement Z-spread experiment. Omit for an inapplicable leg.
    pub spread: Option<FiQuoteLeg>,
}

/// Absolute Campisi component contributions for one side (portfolio or
/// benchmark), each `Σ_j w_j × component_j`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiComponents {
    /// Coupon income effect `Σ w · (coupon / opening mark) · elapsed days / 365`.
    pub carry: f64,
    /// Treasury/duration effect `Σ w · (−MD · Δy)`.
    pub treasury: f64,
    /// Spread effect `Σ w · (−SD · Δs)`.
    pub spread: f64,
    /// Unexplained return `Σ w · (r − explained)`.
    pub residual: f64,
    /// Sum of the four components — equals the side's total return.
    pub total: f64,
}

/// Per-sector benchmark-relative effects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiSectorEffect {
    /// Sector label (mirrors [`FiPositionSnapshot::sector`]).
    pub sector: String,
    /// Portfolio weight in the sector.
    pub portfolio_weight: f64,
    /// Benchmark weight in the sector.
    pub benchmark_weight: f64,
    /// Observed portfolio sector return per unit of signed sector weight, when held.
    pub portfolio_return: Option<f64>,
    /// Observed benchmark sector return per unit of signed sector weight, when held.
    pub benchmark_return: Option<f64>,
    /// Benchmark sector comparator or whole-benchmark return for an off-benchmark sector.
    pub benchmark_comparator: f64,
    /// Brinson-Fachler allocation `(w_p − w_b)(r_b,i − r_b)`.
    pub allocation: f64,
    /// Active income effect `w_p (carry_p,i − carry_b,i)`.
    pub active_carry: f64,
    /// Active duration positioning `w_p (treasury_p,i − treasury_b,i)`.
    pub active_treasury: f64,
    /// Active spread positioning `w_p (spread_p,i − spread_b,i)`.
    pub active_spread: f64,
    /// Unexplained active return `w_p (residual_p,i − residual_b,i)`.
    pub residual: f64,
    /// Sum of the five effects — the sector's contribution to active return.
    pub total_active: f64,
}

/// Dated source observations retained to check a persisted single-period result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FiAttributionSource {
    /// Common dated reporting basis and sector axis.
    pub config: FiAttributionConfig,
    /// Portfolio holdings and observed realized returns.
    pub portfolio: Vec<FiPositionSnapshot>,
    /// Benchmark holdings and observed realized returns.
    pub benchmark: Vec<FiPositionSnapshot>,
}

/// Single-period Campisi benchmark-relative attribution result.
///
/// Replays the recorded position inputs when restored through serde, rejecting
/// any change to dates, basis or computed output that fails native reconciliation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct FiAttributionResult {
    /// Source observations and reporting basis used to produce the result.
    pub source: FiAttributionSource,
    /// Per-sector effects in the declared sector order.
    pub sectors: Vec<FiSectorEffect>,
    /// Portfolio-side absolute Campisi split.
    pub portfolio_components: FiComponents,
    /// Benchmark-side absolute Campisi split.
    pub benchmark_components: FiComponents,
    /// Portfolio total return `Σ w_p,j r_p,j`.
    pub portfolio_return: f64,
    /// Benchmark total return `Σ w_b,j r_b,j`.
    pub benchmark_return: f64,
    /// Active return `portfolio_return − benchmark_return`.
    pub active_return: f64,
    /// Sum of sector allocation effects.
    pub total_allocation: f64,
    /// Sum of sector active carry effects.
    pub total_active_carry: f64,
    /// Sum of sector active treasury effects.
    pub total_active_treasury: f64,
    /// Sum of sector active spread effects.
    pub total_active_spread: f64,
    /// Sum of sector residual effects.
    pub total_residual: f64,
}

impl Serialize for FiAttributionResult {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for FiAttributionResult {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let stored = Self::deserialize(deserializer)?;
        let replayed = campisi_attribution(
            &stored.source.portfolio,
            &stored.source.benchmark,
            &stored.source.config,
        )
        .map_err(serde::de::Error::custom)?;
        if replayed != stored {
            return Err(serde::de::Error::custom(
                "Campisi result disagrees with its source observations",
            ));
        }
        Ok(replayed)
    }
}

impl FiAttributionResult {
    /// Compare displayed active return to the sum of the five represented effects.
    ///
    /// # Arguments
    /// * `tolerance` - Finite nonnegative absolute tolerance in decimal return units.
    ///
    /// # Errors
    /// Rejects invalid tolerances, nonfinite inputs and unrepresentable differences.
    pub fn reconciliation_check(&self, tolerance: f64) -> Result<ReconciliationReport> {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(Error::invalid_input(
                "Campisi reconciliation tolerance must be finite and nonnegative",
            ));
        }
        let mut residual = exact(self.active_return, "displayed active return")?;
        for term in [
            self.total_allocation,
            self.total_active_carry,
            self.total_active_treasury,
            self.total_active_spread,
            self.total_residual,
        ] {
            residual -= exact(term, "displayed effect")?;
        }
        let total_residual = project(&residual)?;
        Ok(ReconciliationReport {
            total_residual,
            is_reconciled: residual.abs() <= exact(tolerance, "reconciliation tolerance")?,
            tolerance,
        })
    }
}

fn exact(number: f64, field: &str) -> Result<BigRational> {
    BigRational::from_float(number)
        .ok_or_else(|| Error::invalid_input(format!("Campisi {field} must be finite")))
}

fn decimal(number: rust_decimal::Decimal) -> BigRational {
    BigRational::from_integer(number.mantissa().into())
        / BigRational::from_integer(10.into()).pow(number.scale() as i32)
}

fn project(number: &BigRational) -> Result<f64> {
    checked_size(number)?;
    number
        .to_f64()
        .filter(|output| output.is_finite() && (*output != 0.0 || number.is_zero()))
        .ok_or_else(|| Error::invalid_input("Campisi output cannot be represented as binary64"))
}

// Rational denominators are bounded at input and after each accumulation to
// cap CPU and storage even when many independently prime quote values occur.
fn checked_size(value: &BigRational) -> Result<()> {
    const MAX_BITS: u64 = 16_384;
    if value.numer().bits().max(value.denom().bits()) > MAX_BITS {
        return Err(Error::invalid_input(
            "Campisi exact arithmetic exceeds its size limit",
        ));
    }
    Ok(())
}

fn leg_effect(leg: Option<&FiQuoteLeg>, field: &str) -> Result<BigRational> {
    let Some(leg) = leg else {
        return Ok(BigRational::zero());
    };
    if leg.sensitivity_basis.trim().is_empty()
        || leg.sensitivity_basis != leg.opening.basis
        || leg.sensitivity_basis != leg.closing.basis
    {
        return Err(Error::invalid_input(format!(
            "Campisi {field} quote and sensitivity bases must match"
        )));
    }
    Ok(-exact(leg.duration, field)?
        * (exact(leg.closing.rate, field)? - exact(leg.opening.rate, field)?))
}

const RETURN: usize = 4;

#[derive(Default)]
struct SideAgg {
    weight: BigRational,
    has_exposure: bool,
    // Carry, Treasury, spread, residual, independently observed total return.
    components: [BigRational; 5],
}

impl SideAgg {
    fn add(&mut self, weight: &BigRational, values: &[BigRational; 5]) -> Result<()> {
        self.weight += weight;
        self.has_exposure |= !weight.is_zero();
        checked_size(&self.weight)?;
        for (total, value) in self.components.iter_mut().zip(values) {
            *total += weight * value;
            checked_size(total)?;
        }
        Ok(())
    }

    fn rates(&self, sector: &str, side: &str) -> Result<Option<[BigRational; 5]>> {
        if self.has_exposure && self.weight.is_zero() {
            return Err(Error::invalid_input(format!(
                "Campisi {side} sector '{sector}' has gross exposure and exactly zero net weight"
            )));
        }
        Ok(self
            .has_exposure
            .then(|| self.components.clone().map(|value| value / &self.weight)))
    }
}

fn aggregate_side(
    snapshots: &[FiPositionSnapshot],
    config: &FiAttributionConfig,
    fraction: &BigRational,
    sectors: &mut IndexMap<&str, (SideAgg, SideAgg)>,
    portfolio: bool,
) -> Result<SideAgg> {
    let mut ids = HashSet::new();
    let mut total = SideAgg::default();
    for position in snapshots {
        if position.id.trim().is_empty() || !ids.insert(position.id.as_str()) {
            return Err(Error::invalid_input(
                "Campisi position IDs must be nonempty and unique on each side",
            ));
        }
        let (long, short) = sectors.get_mut(position.sector.as_str()).ok_or_else(|| {
            Error::invalid_input(format!("Campisi undeclared sector '{}'", position.sector))
        })?;
        if position.opening_market_value.currency() != config.currency
            || position.annual_coupon.currency() != config.currency
            || position.opening_market_value.amount_decimal() <= rust_decimal::Decimal::ZERO
        {
            return Err(Error::invalid_input(
                "Campisi requires a positive opening mark and coupon income in the reporting currency",
            ));
        }
        let carry = decimal(position.annual_coupon.amount_decimal())
            / decimal(position.opening_market_value.amount_decimal())
            * fraction;
        let treasury = leg_effect(position.treasury.as_ref(), "Treasury")?;
        let spread = leg_effect(position.spread.as_ref(), "Z-spread")?;
        let ret = exact(position.total_return, "total return")?;
        let residual = &ret - &carry - &treasury - &spread;
        let values = [carry, treasury, spread, residual, ret];
        let weight = decimal(position.weight.0);
        total.add(&weight, &values)?;
        (if portfolio { long } else { short }).add(&weight, &values)?;
    }
    if !total.weight.is_one() {
        return Err(Error::invalid_input(
            "Campisi exact signed weights must sum to one on each side",
        ));
    }
    if total.components[..RETURN].iter().sum::<BigRational>() != total.components[RETURN] {
        return Err(Error::invalid_input(
            "Campisi side components do not reconcile exactly",
        ));
    }
    Ok(total)
}

fn project_components(side: &SideAgg) -> Result<FiComponents> {
    Ok(FiComponents {
        carry: project(&side.components[0])?,
        treasury: project(&side.components[1])?,
        spread: project(&side.components[2])?,
        residual: project(&side.components[3])?,
        total: project(&side.components[RETURN])?,
    })
}

/// Attribute a portfolio's dated active return to allocation, coupon carry, Treasury, spread and residual.
///
/// # Arguments
/// * `portfolio` - Realized position returns and exact signed opening weights on the portfolio side.
/// * `benchmark` - Realized position returns and exact signed opening weights on the benchmark side.
/// * `config` - Shared reporting policy, reporting currency, ordered sector axis and valuation dates.
///
/// # Errors
/// Rejects inconsistent quote bases, unknown or duplicate identities, invalid values,
/// improperly normalized weights and an output that cannot be represented.
pub fn campisi_attribution(
    portfolio: &[FiPositionSnapshot],
    benchmark: &[FiPositionSnapshot],
    config: &FiAttributionConfig,
) -> Result<FiAttributionResult> {
    calculate_campisi(portfolio, benchmark, config).map(|(display, _)| display)
}

struct ExactCampisiResult {
    portfolio_return: BigRational,
    benchmark_return: BigRational,
    rows: Vec<[BigRational; 5]>,
}

fn calculate_campisi(
    portfolio: &[FiPositionSnapshot],
    benchmark: &[FiPositionSnapshot],
    config: &FiAttributionConfig,
) -> Result<(FiAttributionResult, ExactCampisiResult)> {
    let days = (config.end - config.start).whole_days();
    if days <= 0 {
        return Err(Error::invalid_input(
            "Campisi closing date must be after the opening date",
        ));
    }
    if [
        &config.basis.portfolio_series_id,
        &config.basis.benchmark_series_id,
        &config.basis.return_policy,
        &config.basis.valuation,
        &config.basis.fx,
        &config.basis.fees,
    ]
    .iter()
    .any(|basis| basis.trim().is_empty())
    {
        return Err(Error::invalid_input(
            "Campisi reporting bases must be explicit",
        ));
    }
    let mut sectors: IndexMap<&str, (SideAgg, SideAgg)> = IndexMap::new();
    for sector in &config.sectors {
        if sector.trim().is_empty()
            || sectors
                .insert(sector.as_str(), Default::default())
                .is_some()
        {
            return Err(Error::invalid_input(
                "Campisi sector labels must be nonempty and unique",
            ));
        }
    }

    let source = FiAttributionSource {
        config: config.clone(),
        portfolio: portfolio.to_vec(),
        benchmark: benchmark.to_vec(),
    };
    let fraction = BigRational::new(days.into(), 365.into());
    let portfolio = aggregate_side(portfolio, config, &fraction, &mut sectors, true)?;
    let benchmark = aggregate_side(benchmark, config, &fraction, &mut sectors, false)?;
    let active_return = &portfolio.components[RETURN] - &benchmark.components[RETURN];
    let mut totals = std::array::from_fn::<_, 5, _>(|_| BigRational::zero());
    let mut sector_effects = Vec::with_capacity(sectors.len());
    let mut exact_rows = Vec::with_capacity(sectors.len());

    for (sector, (p, b)) in sectors {
        let p_rates = p.rates(sector, "portfolio")?;
        let b_rates = b.rates(sector, "benchmark")?;
        let effective_b = b_rates.as_ref().unwrap_or(&benchmark.components);
        let allocation =
            (&p.weight - &b.weight) * (&effective_b[RETURN] - &benchmark.components[RETURN]);
        let active_components =
            std::array::from_fn::<_, 4, _>(|i| &p.components[i] - &p.weight * &effective_b[i]);
        let effects = [
            allocation,
            active_components[0].clone(),
            active_components[1].clone(),
            active_components[2].clone(),
            active_components[3].clone(),
        ];
        let total_active = effects.iter().sum::<BigRational>();
        if total_active
            != &p.components[RETURN]
                - &b.components[RETURN]
                - (&p.weight - &b.weight) * &benchmark.components[RETURN]
        {
            return Err(Error::invalid_input(
                "Campisi centered sector return does not reconcile exactly",
            ));
        }
        for (accumulator, effect) in totals.iter_mut().zip(&effects) {
            checked_size(effect)?;
            *accumulator += effect;
            checked_size(accumulator)?;
        }
        sector_effects.push(FiSectorEffect {
            sector: sector.to_owned(),
            portfolio_weight: project(&p.weight)?,
            benchmark_weight: project(&b.weight)?,
            portfolio_return: p_rates
                .as_ref()
                .map(|rates| project(&rates[RETURN]))
                .transpose()?,
            benchmark_return: b_rates
                .as_ref()
                .map(|rates| project(&rates[RETURN]))
                .transpose()?,
            benchmark_comparator: project(&effective_b[RETURN])?,
            allocation: project(&effects[0])?,
            active_carry: project(&effects[1])?,
            active_treasury: project(&effects[2])?,
            active_spread: project(&effects[3])?,
            residual: project(&effects[4])?,
            total_active: project(&total_active)?,
        });
        exact_rows.push(effects);
    }
    if totals.iter().sum::<BigRational>() != active_return {
        return Err(Error::invalid_input(
            "Campisi total active return does not reconcile exactly",
        ));
    }
    let projected = FiAttributionResult {
        source,
        sectors: sector_effects,
        portfolio_components: project_components(&portfolio)?,
        benchmark_components: project_components(&benchmark)?,
        portfolio_return: project(&portfolio.components[RETURN])?,
        benchmark_return: project(&benchmark.components[RETURN])?,
        active_return: project(&active_return)?,
        total_allocation: project(&totals[0])?,
        total_active_carry: project(&totals[1])?,
        total_active_treasury: project(&totals[2])?,
        total_active_spread: project(&totals[3])?,
        total_residual: project(&totals[4])?,
    };
    Ok((
        projected,
        ExactCampisiResult {
            portfolio_return: portfolio.components[RETURN].clone(),
            benchmark_return: benchmark.components[RETURN].clone(),
            rows: exact_rows,
        },
    ))
}

/// Carino-linked per-sector FI effects summed across periods.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct FiLinkedSectorEffect {
    /// Sector label.
    pub sector: String,
    /// Linked allocation effect.
    pub allocation: f64,
    /// Linked active carry effect.
    pub active_carry: f64,
    /// Linked active treasury effect.
    pub active_treasury: f64,
    /// Linked active spread effect.
    pub active_spread: f64,
    /// Linked residual effect.
    pub residual: f64,
    /// Sum of the five linked effects.
    pub total_active: f64,
}

/// Multi-period Carino-linked Campisi attribution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct FiCarinoLinkedResult {
    /// Per-period single-period results, in chronological order.
    pub periods: Vec<FiAttributionResult>,
    /// Geometrically compounded portfolio return `∏(1 + r_p,t) − 1`.
    pub portfolio_return_compounded: f64,
    /// Geometrically compounded benchmark return.
    pub benchmark_return_compounded: f64,
    /// Independently compounded active return, computed before projecting the two side returns.
    pub active_return_compounded: f64,
    /// Per-sector linked effects; their grand total reconstructs the
    /// compounded active return exactly.
    pub linked_sectors: Vec<FiLinkedSectorEffect>,
    /// Sum of linked allocation effects.
    pub linked_allocation: f64,
    /// Sum of linked active carry effects.
    pub linked_active_carry: f64,
    /// Sum of linked active treasury effects.
    pub linked_active_treasury: f64,
    /// Sum of linked active spread effects.
    pub linked_active_spread: f64,
    /// Sum of linked residual effects.
    pub linked_residual: f64,
}

impl Serialize for FiCarinoLinkedResult {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for FiCarinoLinkedResult {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let stored = Self::deserialize(deserializer)?;
        let replay = campisi_carino_link(&stored.periods).map_err(serde::de::Error::custom)?;
        if replay != stored {
            return Err(serde::de::Error::custom(
                "Campisi linked result disagrees with its source periods",
            ));
        }
        Ok(replay)
    }
}
/// Apply Carino (1999) smoothing to the verified exact observations of a
/// contiguous sequence of fixed-income attribution runs.
///
/// Both sides must have positive growth in every interval. The method keeps
/// cumulative growth and each period's effects in widened rational arithmetic,
/// using stable log secants for the Carino coefficients.
///
/// # Arguments
/// * `periods` - Chronological, contiguous periods on the same portfolio and
///   benchmark series, currency, reporting basis and ordered sector taxonomy.
///
/// # Errors
/// Rejects an empty sequence, inconsistent source evidence, invalid dates or
/// basis, nonpositive growth, arithmetic beyond the supported precision
/// envelope, and unrepresentable or unreconciled effects.
pub fn campisi_carino_link(periods: &[FiAttributionResult]) -> Result<FiCarinoLinkedResult> {
    let first = periods.first().ok_or_else(|| {
        Error::invalid_input("Campisi Carino linking requires at least one period")
    })?;
    let first_config = &first.source.config;
    let mut previous_end = first_config.start;
    let mut portfolio_growth = BigRational::one();
    let mut benchmark_growth = BigRational::one();
    let mut checked = Vec::with_capacity(periods.len());
    for period in periods {
        let source = &period.source;
        let config = &source.config;
        if config.start != previous_end {
            return Err(Error::invalid_input(
                "Campisi Carino periods must be chronological and contiguous",
            ));
        }
        if config.currency != first_config.currency || config.basis != first_config.basis {
            return Err(Error::invalid_input(
                "Campisi Carino periods require the same series, currency and reporting basis",
            ));
        }
        if config.sectors != first_config.sectors {
            return Err(Error::invalid_input(
                "Campisi Carino requires the same ordered sector taxonomy",
            ));
        }
        previous_end = config.end;
        let (restored, exact) = calculate_campisi(&source.portfolio, &source.benchmark, config)?;
        if &restored != period {
            return Err(Error::invalid_input(
                "Campisi result disagrees with its source observations",
            ));
        }
        let portfolio_period_growth = BigRational::one() + exact.portfolio_return;
        let benchmark_period_growth = BigRational::one() + exact.benchmark_return;
        let coefficient = carino_exact_secant(&portfolio_period_growth, &benchmark_period_growth)?;
        let period_active = &portfolio_period_growth - &benchmark_period_growth;
        portfolio_growth *= portfolio_period_growth;
        benchmark_growth *= benchmark_period_growth;
        checked_size(&portfolio_growth)?;
        checked_size(&benchmark_growth)?;
        checked.push((coefficient, period_active, exact.rows));
    }
    let horizon = carino_exact_secant(&portfolio_growth, &benchmark_growth)?;
    let active = &portfolio_growth - &benchmark_growth;
    let zero = || std::array::from_fn::<_, 5, _>(|_| BigRational::zero());
    let mut rows = vec![zero(); first_config.sectors.len()];
    let mut period_active_magnitude = BigRational::zero();
    for (coefficient, period_active, effects) in checked {
        let scale = coefficient / &horizon;
        period_active_magnitude += (period_active * &scale).abs();
        checked_size(&period_active_magnitude)?;
        for (row, period) in rows.iter_mut().zip(effects) {
            for (aggregate, effect) in row.iter_mut().zip(period) {
                *aggregate += effect * &scale;
                checked_size(aggregate)?;
            }
        }
    }

    let mut totals = zero();
    let mut linked_sectors = Vec::with_capacity(rows.len());
    for (sector, effects) in first_config.sectors.iter().zip(rows) {
        let total_active = effects.iter().sum::<BigRational>();
        for (total, effect) in totals.iter_mut().zip(&effects) {
            *total += effect;
            checked_size(total)?;
        }
        linked_sectors.push(FiLinkedSectorEffect {
            sector: sector.clone(),
            allocation: project(&effects[0])?,
            active_carry: project(&effects[1])?,
            active_treasury: project(&effects[2])?,
            active_spread: project(&effects[3])?,
            residual: project(&effects[4])?,
            total_active: project(&total_active)?,
        });
    }
    // The sole inexact calculation before final projection is the logarithm
    // ratio. Its rounding envelope scales with the observed compressed return
    // difference, rather than with unrelated offsetting linked effects.
    let reconstructed = totals.iter().sum::<BigRational>();
    let error = (&reconstructed - &active).abs();
    let scale = period_active_magnitude.max(active.abs());
    if error > scale * exact(32.0 * f64::EPSILON, "Carino log rounding envelope")? {
        return Err(Error::invalid_input(
            "Campisi Carino effects do not reconcile to compounded active return",
        ));
    }
    Ok(FiCarinoLinkedResult {
        periods: periods.to_vec(),
        portfolio_return_compounded: project(&(portfolio_growth - BigRational::one()))?,
        benchmark_return_compounded: project(&(benchmark_growth - BigRational::one()))?,
        active_return_compounded: project(&active)?,
        linked_sectors,
        linked_allocation: project(&totals[0])?,
        linked_active_carry: project(&totals[1])?,
        linked_active_treasury: project(&totals[2])?,
        linked_active_spread: project(&totals[3])?,
        linked_residual: project(&totals[4])?,
    })
}

fn carino_exact_secant(a: &BigRational, b: &BigRational) -> Result<BigRational> {
    if !a.is_positive() || !b.is_positive() {
        return Err(Error::invalid_input(
            "Campisi Carino requires strictly positive growth in each interval",
        ));
    }
    if a == b {
        return Ok(a.recip());
    }
    let (low, high) = if a < b { (a, b) } else { (b, a) };
    let gap = high - low;
    let relative = (&gap / high)
        .to_f64()
        .ok_or_else(|| Error::invalid_input("Carino relative growth gap is unrepresentable"))?;
    if relative < 0.5 {
        let log_ratio = if relative == 0.0 {
            1.0
        } else {
            -libm::log1p(-relative) / relative
        };
        return Ok(exact(log_ratio, "Carino log1p coefficient")? / high);
    }

    // Evaluate the log of the ratio without converting the full-sized ratio
    // itself to binary64. This branch works even when one side is near zero.
    let ratio = high / low;
    checked_size(&ratio)?;
    let exponent = ratio.numer().bits().saturating_sub(ratio.denom().bits()) as usize;
    let mantissa = BigRational::new(ratio.numer().clone(), ratio.denom() << exponent);
    let ln_ratio = libm::log(project(&mantissa)?) + (exponent as f64) * std::f64::consts::LN_2;
    Ok(exact(ln_ratio, "Carino log coefficient")? / gap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use time::macros::date;

    fn config() -> FiAttributionConfig {
        FiAttributionConfig {
            start: date!(2025 - 01 - 01),
            end: date!(2025 - 03 - 15),
            currency: Currency::USD,
            basis: FiReportingBasis {
                portfolio_series_id: "portfolio".into(),
                benchmark_series_id: "benchmark".into(),
                return_policy: "reinvested_net_total_return".into(),
                valuation: "USD-2025-close-dirty-settlement".into(),
                fx: "USD-mark".into(),
                fees: "net-of-fees".into(),
            },
            sectors: ["CORE", "PORTFOLIO", "BENCHMARK"]
                .map(str::to_owned)
                .to_vec(),
        }
    }

    fn position(
        id: &str,
        sector: &str,
        weight: rust_decimal::Decimal,
        total_return: f64,
    ) -> FiPositionSnapshot {
        FiPositionSnapshot {
            id: id.into(),
            sector: sector.into(),
            weight: DecimalWire(weight),
            total_return,
            opening_market_value: Money::from((80_i64, Currency::USD)),
            annual_coupon: Money::from((0_i64, Currency::USD)),
            treasury: None,
            spread: None,
        }
    }

    fn quote_leg(basis: &str, duration: f64, start: f64, end: f64) -> FiQuoteLeg {
        FiQuoteLeg {
            sensitivity_basis: basis.into(),
            duration,
            opening: FiQuote {
                basis: basis.into(),
                rate: start,
            },
            closing: FiQuote {
                basis: basis.into(),
                rate: end,
            },
        }
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-13, "{actual} != {expected}");
    }

    #[test]
    fn campisi_cash_coupon_and_independent_shock_fixture() {
        let mut portfolio = position("BOND", "CORE", dec!(1), 0.02);
        portfolio.annual_coupon = Money::from((5_i64, Currency::USD));
        portfolio.treasury = Some(quote_leg("USD-TREASURY:5Y:ACT365", 5.0, 0.03, 0.031));
        portfolio.spread = Some(quote_leg("USD-OIS:Z:ACT365:MATURITY", 4.0, -0.0015, 0.001));
        let comparison = position("CASH", "CORE", dec!(1), 0.0);

        let result = campisi_attribution(
            std::slice::from_ref(&portfolio),
            std::slice::from_ref(&comparison),
            &config(),
        )
        .expect("qualified positions");
        assert_close(result.portfolio_components.carry, 0.0125);
        assert_close(result.portfolio_components.treasury, -0.005);
        assert_close(result.portfolio_components.spread, -0.01);
        assert_close(result.portfolio_components.residual, 0.0225);
        assert!(
            result
                .reconciliation_check(1e-13)
                .expect("valid tolerance")
                .is_reconciled
        );
        assert!(result.reconciliation_check(f64::INFINITY).is_err());

        portfolio.spread.as_mut().expect("spread leg").closing.basis = "wrong".into();
        assert!(campisi_attribution(&[portfolio], &[comparison], &config()).is_err());
    }

    #[test]
    fn campisi_preserves_one_sided_rates_and_report_order() {
        let portfolio = [
            position("p2", "PORTFOLIO", dec!(0.25), 0.098),
            position("p1", "CORE", dec!(0.75), 0.03),
        ];
        let benchmark = [
            position("b2", "BENCHMARK", dec!(0.25), 0.01),
            position("b1", "CORE", dec!(0.75), 0.014),
        ];
        let report =
            campisi_attribution(&portfolio, &benchmark, &config()).expect("both sides normalized");
        assert_close(report.active_return, 0.034);
        let extra = &report.sectors[1];
        assert_eq!(extra.sector, "PORTFOLIO");
        assert_eq!(extra.benchmark_return, None);
        assert_close(extra.benchmark_comparator, 0.013);
        assert_eq!(extra.allocation, 0.0);
        let benchmark_only = &report.sectors[2];
        assert_eq!(benchmark_only.portfolio_return, None);
        assert_eq!(benchmark_only.residual, 0.0);
        assert!(
            report
                .reconciliation_check(1e-13)
                .expect("valid tolerance")
                .is_reconciled
        );
    }

    #[test]
    fn campisi_exact_signed_weights_admit_a_small_nonzero_net() {
        let mut positions = [
            position("p1", "CORE", dec!(0.5), 0.03),
            position("p2", "CORE", dec!(-0.499999999999), 0.02),
            position("p3", "PORTFOLIO", dec!(0.999999999999), 0.04),
        ];
        let benchmark = [position("b1", "CORE", dec!(1), 0.03)];
        let report =
            campisi_attribution(&positions, &benchmark, &config()).expect("exact nonzero net");
        assert_close(report.sectors[0].portfolio_weight, 1e-12);
        assert!(report.sectors[0].portfolio_return.is_some());
        positions[1].weight = DecimalWire(dec!(-0.5));
        positions[2].weight = DecimalWire(dec!(1));
        assert!(campisi_attribution(&positions, &benchmark, &config()).is_err());
    }
    /// Hand-worked golden fixture: 2 sectors × 2 positions per side,
    /// with coupon income scaled into a 73-day window.
    fn golden_portfolio() -> Vec<FiPositionSnapshot> {
        vec![
            snap("CORE", 0.30, 0.0155, 0.040, 5.0, 0.0, 0.0, -0.0010, 0.0),
            snap("CORE", 0.20, 0.0190, 0.045, 8.0, 0.0, 0.0, -0.0010, 0.0),
            snap(
                "PORTFOLIO",
                0.30,
                0.0120,
                0.060,
                4.0,
                3.8,
                0.0150,
                -0.0010,
                0.0020,
            ),
            snap(
                "PORTFOLIO",
                0.20,
                0.0118,
                0.070,
                6.0,
                5.5,
                0.0250,
                -0.0010,
                0.0020,
            ),
        ]
    }

    fn golden_benchmark() -> Vec<FiPositionSnapshot> {
        vec![
            snap("CORE", 0.45, 0.0155, 0.038, 6.0, 0.0, 0.0, -0.0010, 0.0),
            snap("CORE", 0.15, 0.0195, 0.042, 9.0, 0.0, 0.0, -0.0010, 0.0),
            snap(
                "PORTFOLIO",
                0.25,
                0.0090,
                0.055,
                5.0,
                4.8,
                0.0120,
                -0.0010,
                0.0020,
            ),
            snap(
                "PORTFOLIO",
                0.15,
                0.0100,
                0.065,
                7.0,
                6.5,
                0.0200,
                -0.0010,
                0.0020,
            ),
        ]
    }

    #[allow(clippy::too_many_arguments)]
    fn snap(
        sector: &str,
        weight: f64,
        ret: f64,
        carry_equivalent: f64,
        treasury_duration: f64,
        spread_duration: f64,
        spread: f64,
        treasury_move: f64,
        spread_move: f64,
    ) -> FiPositionSnapshot {
        let to_decimal = |n: f64| {
            n.to_string()
                .parse::<rust_decimal::Decimal>()
                .expect("finite fixture decimal")
        };
        let mut result = position(
            &format!("{sector}/{weight}/{ret}"),
            sector,
            to_decimal(weight),
            ret,
        );
        result.opening_market_value = Money::from((80_i64, Currency::USD));
        // 80 × 1.25 × yield reproduces the independently calculated quarter-equivalent coupon carry in 73 days.
        result.annual_coupon =
            Money::from_decimal(dec!(100) * to_decimal(carry_equivalent), Currency::USD);
        result.treasury = Some(quote_leg(
            "USD-TREASURY",
            treasury_duration,
            0.04,
            0.04 + treasury_move,
        ));
        if spread_duration != 0.0 {
            result.spread = Some(quote_leg(
                "QUALIFIED-Z",
                spread_duration,
                spread,
                spread + spread_move,
            ));
        }
        result
    }

    fn assert_fixture_close(actual: f64, expected: f64, label: &str) {
        assert!(
            (actual - expected).abs() < 1e-12,
            "{label}: {actual} != {expected}"
        );
    }
    #[test]
    fn campisi_single_period_matches_hand_worked_golden() {
        let mut config = config();
        config.sectors.truncate(2);
        let r = campisi_attribution(&golden_portfolio(), &golden_benchmark(), &config)
            .expect("valid golden inputs");

        // Headline returns.
        assert_fixture_close(r.portfolio_return, 0.01441, "portfolio_return");
        assert_fixture_close(r.benchmark_return, 0.01365, "benchmark_return");
        assert_fixture_close(r.active_return, 0.00076, "active_return");

        // Absolute Campisi split, portfolio side.
        assert_fixture_close(r.portfolio_components.carry, 0.01325, "p carry");
        assert_fixture_close(r.portfolio_components.treasury, 0.0055, "p treasury");
        assert_fixture_close(r.portfolio_components.spread, -0.00448, "p spread");
        assert_fixture_close(r.portfolio_components.residual, 0.00014, "p residual");
        assert_fixture_close(r.portfolio_components.total, 0.01441, "p total");

        // Absolute Campisi split, benchmark side.
        assert_fixture_close(r.benchmark_components.carry, 0.011725, "b carry");
        assert_fixture_close(r.benchmark_components.treasury, 0.00635, "b treasury");
        assert_fixture_close(r.benchmark_components.spread, -0.00435, "b spread");
        assert_fixture_close(r.benchmark_components.residual, -0.000075, "b residual");

        // Sector effects, in portfolio-first-seen order.
        assert_eq!(r.sectors.len(), 2);
        let govt = &r.sectors[0];
        assert_eq!(govt.sector, "CORE");
        assert_fixture_close(govt.portfolio_weight, 0.50, "govt w_p");
        assert_fixture_close(govt.benchmark_weight, 0.60, "govt w_b");
        assert_fixture_close(govt.portfolio_return.expect("present"), 0.0169, "govt r_p");
        assert_fixture_close(govt.benchmark_return.expect("present"), 0.0165, "govt r_b");
        assert_fixture_close(govt.allocation, -0.000285, "govt allocation");
        assert_fixture_close(govt.active_carry, 0.000375, "govt active_carry");
        assert_fixture_close(govt.active_treasury, -0.000275, "govt active_treasury");
        assert_fixture_close(govt.active_spread, 0.0, "govt active_spread");
        assert_fixture_close(govt.residual, 0.0001, "govt residual");
        assert_fixture_close(govt.total_active, -0.000085, "govt total_active");

        let corp = &r.sectors[1];
        assert_eq!(corp.sector, "PORTFOLIO");
        assert_fixture_close(corp.allocation, -0.0004275, "corp allocation");
        assert_fixture_close(corp.active_carry, 0.00065625, "corp active_carry");
        assert_fixture_close(corp.active_treasury, -0.000475, "corp active_treasury");
        assert_fixture_close(corp.active_spread, 0.0009575, "corp active_spread");
        assert_fixture_close(corp.residual, 0.00013375, "corp residual");
        assert_fixture_close(corp.total_active, 0.000845, "corp total_active");

        // Totals.
        assert_fixture_close(r.total_allocation, -0.0007125, "total_allocation");
        assert_fixture_close(r.total_active_carry, 0.00103125, "total_active_carry");
        assert_fixture_close(r.total_active_treasury, -0.00075, "total_active_treasury");
        assert_fixture_close(r.total_active_spread, 0.0009575, "total_active_spread");
        assert_fixture_close(r.total_residual, 0.00023375, "total_residual");

        // Reconciliation by construction.
        let recon = r.reconciliation_check(1e-10).expect("finite tolerance");
        assert!(recon.is_reconciled, "residual {}", recon.total_residual);
    }

    #[test]
    fn checked_link_preserves_compounded_active_below_side_display_precision() {
        let first_config = FiAttributionConfig {
            sectors: vec!["Core".into()],
            ..config()
        };
        let second_config = FiAttributionConfig {
            start: first_config.end,
            end: date!(2025 - 05 - 27),
            ..first_config.clone()
        };
        let low = -1.0 + 2.0_f64.powi(-53);
        let higher = -1.0 + 2.0_f64.powi(-52);
        let first = campisi_attribution(
            &[position("P", "Core", dec!(1), low)],
            &[position("B", "Core", dec!(1), low)],
            &first_config,
        )
        .expect("positive first period");
        let second = campisi_attribution(
            &[position("P", "Core", dec!(1), low)],
            &[position("B", "Core", dec!(1), higher)],
            &second_config,
        )
        .expect("positive second period");
        let linked = campisi_carino_link(&[first, second]).expect("positive compounded growth");
        assert_eq!(linked.portfolio_return_compounded, -1.0);
        assert_eq!(linked.benchmark_return_compounded, -1.0);
        assert_eq!(linked.active_return_compounded, -2.0_f64.powi(-106));
        assert!((linked.linked_residual / linked.active_return_compounded - 1.0).abs() < 1e-14);
        let saved = serde_json::to_string(&linked).expect("save report");
        let restored: FiCarinoLinkedResult =
            serde_json::from_str(&saved).expect("restore and verify");
        assert_eq!(restored, linked);
    }

    #[test]
    fn checked_link_preserves_offsetting_effects_at_equal_growth() {
        let first_config = FiAttributionConfig {
            sectors: vec!["Core".into()],
            ..config()
        };
        let second_config = FiAttributionConfig {
            start: first_config.end,
            end: date!(2025 - 05 - 27),
            ..first_config.clone()
        };
        let mut carry = position("P", "Core", dec!(1), 1.0);
        carry.annual_coupon = Money::from((200_i64, Currency::USD));
        let first = campisi_attribution(
            &[carry],
            &[position("B", "Core", dec!(1), 0.0)],
            &first_config,
        )
        .expect("first period");
        let second = campisi_attribution(
            &[position("P", "Core", dec!(1), 0.0)],
            &[position("B", "Core", dec!(1), 1.0)],
            &second_config,
        )
        .expect("second period");
        let linked = campisi_carino_link(&[first, second]).expect("positive growth");
        assert_eq!(linked.active_return_compounded, 0.0);
        assert!((linked.linked_active_carry - std::f64::consts::LN_2).abs() < 1e-14);
        assert!((linked.linked_residual + std::f64::consts::LN_2).abs() < 1e-14);
    }

    #[test]
    fn display_reconciliation_handles_finite_offsetting_effects() {
        let holding = position("P", "CORE", dec!(1), 0.0);
        let mut report = campisi_attribution(
            std::slice::from_ref(&holding),
            std::slice::from_ref(&holding),
            &config(),
        )
        .expect("valid source");
        report.total_allocation = 1e308;
        report.total_active_carry = 1e308;
        report.total_active_treasury = -1e308;
        report.total_active_spread = -1e308;
        let display = report
            .reconciliation_check(0.0)
            .expect("finite exact residual");
        assert_eq!(display.total_residual, 0.0);
        assert!(display.is_reconciled);
    }
}
