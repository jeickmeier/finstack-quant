//! Margin Valuation Adjustment (MVA): funding cost of initial margin.
//!
//! Posting initial margin (IM) under the BCBS-IOSCO uncleared-margin rules ties
//! up funding for the life of the netting set. MVA prices that cost:
//!
//! ```text
//! MVA = ∫₀ᵀ λ_B(t) · E[IM(t)] · DF(t) · S(t) dt
//! ```
//!
//! where `λ_B(t)` is the bank's funding spread (decimal, from bp inputs),
//! `E[IM(t)]` the expected IM profile, `DF(t)` the risk-free discount factor,
//! and `S(t)` the survival probability (optional; `S ≡ 1` when omitted).
//! [`compute_mva`](crate::xva::mva::compute_mva) conditions on the bank's own
//! survival only; the bilateral
//! engine ([`crate::xva::cva::compute_bilateral_xva`]) uses joint
//! (first-to-default) survival `S_B(t)·S_C(t)`, since a counterparty default
//! terminates the netting set and returns the posted margin.
//!
//! `E[IM(t)]` is built deterministically from the current ISDA SIMM number and
//! a decay profile ([`im_profile_from_simm`](crate::xva::mva::im_profile_from_simm))
//! — the standard practitioner approximation. Per-path IM from a stochastic
//! exposure engine is not modeled here; callers holding a simulated IM
//! distribution supply its path mean as the profile.
//!
//! # Integration convention
//!
//! Merge dated IM and spread observations, sample each term structure on its
//! own curve clock and integrate the full product of linearly interpolated
//! factors with three-point Gauss-Legendre quadrature.
//!
//! # Aggregation
//!
//! MVA is a positive funding cost using the same sign convention as CVA and
//! FVA. [`crate::xva::cva::compute_bilateral_xva`] computes it automatically
//! whenever [`crate::xva::types::FundingConfig::im_profile`] is set and reports
//! `CVA − DVA + FVA + MVA` in
//! [`crate::xva::types::XvaResult::total_xva`].
//!
//! # Model boundaries
//!
//! Credit adjustments use expected exposure measured under the admitted
//! collateral and close-out terms. The posted-IM profile specifies funding.
//!
//! # References
//!
//! - Green, A. (2015). *XVA: Credit, Funding and Capital Valuation
//!   Adjustments*. Wiley. Chapter 10 (MVA). `docs/REFERENCES.md#green-xva`
//! - Andersen, L., Pykhtin, M., & Sokol, A. (2017). "Rethinking the margin
//!   period of risk." *Journal of Credit Risk*, 13(1).
//! - ISDA SIMM v2.6: `docs/REFERENCES.md#isda-simm`
//! - Green XVA: `docs/REFERENCES.md#green-xva`

use finstack_quant_core::Error;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;

use crate::calculators::im::SimmCalculator;
use crate::types::SimmSensitivities;

/// Deterministic decay applied to today's SIMM IM to approximate `E[IM(t)]`.
///
/// `IM(t) = IM(0) × factor(t)`. This is the standard first-order practitioner
/// approximation for MVA (Green 2015, ch. 10): IM shrinks as the portfolio's
/// remaining risk runs off.
///
/// # Variants
///
/// | Variant | `factor(t)` | Use case |
/// |---|---|---|
/// | `Constant` | `1` | Evergreen / constantly re-hedged books |
/// | `LinearToMaturity` | `max(1 − t/T, 0)` | Amortizing linear-risk books |
/// | `SqrtTime` | `sqrt(max(1 − t/T, 0))` | DV01-style risk ∝ √(remaining time) |
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ImDecayProfile {
    /// IM stays at today's level for the whole horizon.
    Constant,
    /// IM decays linearly to zero at `maturity_years`.
    LinearToMaturity {
        /// Portfolio maturity `T` in years (must be positive and finite).
        maturity_years: f64,
    },
    /// IM decays like the square root of remaining time to `maturity_years`.
    SqrtTime {
        /// Portfolio maturity `T` in years (must be positive and finite).
        maturity_years: f64,
    },
}

impl ImDecayProfile {
    /// Linear decay to zero at `maturity_years`: `factor(t) = max(1 − t/T, 0)`.
    ///
    /// # Arguments
    ///
    /// * `maturity_years` - Portfolio maturity `T` in years; must be positive
    ///   and finite.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `maturity_years` is non-positive or
    /// non-finite.
    pub fn linear_to_maturity(maturity_years: f64) -> finstack_quant_core::Result<Self> {
        let profile = Self::LinearToMaturity { maturity_years };
        profile.validate()?;
        Ok(profile)
    }

    /// Square-root decay to zero at `maturity_years`:
    /// `factor(t) = sqrt(max(1 − t/T, 0))`.
    ///
    /// # Arguments
    ///
    /// * `maturity_years` - Portfolio maturity `T` in years; must be positive
    ///   and finite.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `maturity_years` is non-positive or
    /// non-finite.
    pub fn sqrt_time(maturity_years: f64) -> finstack_quant_core::Result<Self> {
        let profile = Self::SqrtTime { maturity_years };
        profile.validate()?;
        Ok(profile)
    }

    /// Decay factor at time `t` (years). Always in `[0, 1]` for `t ≥ 0`.
    ///
    /// # References
    ///
    /// - Green, A. (2015). *XVA*. Wiley. Chapter 10. `docs/REFERENCES.md#green-xva`
    pub fn factor(&self, t: f64) -> f64 {
        match self {
            Self::Constant => 1.0,
            Self::LinearToMaturity { maturity_years } => (1.0 - t / maturity_years).max(0.0),
            Self::SqrtTime { maturity_years } => (1.0 - t / maturity_years).max(0.0).sqrt(),
        }
    }

    /// Validate decay parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if `maturity_years` is non-positive or non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        match self {
            Self::Constant => Ok(()),
            Self::LinearToMaturity { maturity_years } | Self::SqrtTime { maturity_years } => {
                if !maturity_years.is_finite() || *maturity_years <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "ImDecayProfile: maturity_years {maturity_years} must be positive and finite"
                    )));
                }
                Ok(())
            }
        }
    }
}

/// Source of the expected posted initial margin on a dated grid.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImProjectionBasis {
    /// Qualified expected posted IM from a supplied risk calculation.
    External {
        /// Permanent identifier of the supplied evidence and methodology.
        source_id: String,
        /// Legal portfolio scope measured by the supplied calculation.
        netting_set: crate::types::NettingSetId,
    },
    /// Deterministic runoff from a single dated, historical SIMM calculation.
    SimmDecay {
        /// Fingerprint of the admitted inputs, calculation and projection.
        source_id: String,
        /// Legal portfolio scope supplying the opening margin.
        netting_set: crate::types::NettingSetId,
        /// Selected historical margin parameter version.
        version: crate::SimmVersion,
        /// Elected deterministic approximation applied to the opening margin.
        decay: ImDecayProfile,
    },
}

impl ImProjectionBasis {
    fn source(&self) -> (&str, &crate::types::NettingSetId) {
        match self {
            Self::External {
                source_id,
                netting_set,
            }
            | Self::SimmDecay {
                source_id,
                netting_set,
                ..
            } => (source_id, netting_set),
        }
    }
}

/// Complete dated expected posted initial margin in one currency and netting set.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ImProfile {
    /// Reporting date and first observation.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Currency of the opening and every projected amount.
    pub currency: Currency,
    /// Legal scope, source and election for the expected IM.
    pub basis: ImProjectionBasis,
    /// Calendar observations starting with the valuation date and ending at the horizon.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub dates: Vec<Date>,
    /// Expected posted IM in major units of the calculation currency, nonnegative.
    pub im_values: Vec<f64>,
}

impl ImProfile {
    /// Validate the complete observation calendar, amounts and scope.
    ///
    /// # Errors
    /// Rejects unordered or unanchored dates, absent evidence, invalid terms and
    /// a non-finite or negative opening or projected amount.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let (source_id, scope) = self.basis.source();
        let valid_scope = matches!(scope, crate::types::NettingSetId::Bilateral { counterparty_id, csa_id }
            if !counterparty_id.trim().is_empty() && !csa_id.trim().is_empty());
        if source_id.trim().is_empty() || !valid_scope {
            return Err(Error::Validation(
                "IM projection requires identified bilateral scope and calculation evidence".into(),
            ));
        }
        if self.dates.len() < 2
            || self.dates.first() != Some(&self.as_of)
            || self.dates.windows(2).any(|dates| dates[0] >= dates[1])
        {
            return Err(Error::Validation(
                "IM dates must start at valuation, strictly increase and include the horizon"
                    .into(),
            ));
        }
        if self.im_values.len() != self.dates.len()
            || self
                .im_values
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(Error::Validation(
                "Expected IM values must be finite, nonnegative and aligned with the dates".into(),
            ));
        }
        Ok(())
    }

    /// Derive observation years from the opening date on ACT/365 Fixed.
    pub fn times(&self) -> Vec<f64> {
        self.dates
            .iter()
            .map(|&date| elapsed_years(self.as_of, date))
            .collect()
    }
}

fn elapsed_years(as_of: Date, date: Date) -> f64 {
    (date - as_of).whole_days() as f64 / 365.0
}

/// Forecast scope and decay election for one dated posted-margin projection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ImProjectionSpec {
    /// Day on which opening SIMM and the projected profile are measured.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Currency for aggregation and all values in the projection.
    pub currency: Currency,
    /// Distinct, qualified bilateral netting scope with legal agreement identity.
    pub netting_set: crate::types::NettingSetId,
    /// Identity of the admitted margin run or snapshot.
    pub scope_source: String,
    /// Dated projection grid, including valuation and horizon and any within-horizon runoff date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub dates: Vec<Date>,
    /// Explicitly elected deterministic margin runoff approximation.
    pub decay: ImDecayProfile,
}

/// Funding-spread election in basis points for a dated IM profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MarginFundingSpread {
    /// Apply one finite nonnegative spread at all dates.
    Flat {
        /// Spread in basis points (for example 25 represents 0.25%).
        spread_bp: f64,
    },
    /// Linear interpolation of nonnegative basis-point spreads across a complete dated interval.
    Dated {
        /// Calendar dates and spread quotes in basis points. The dates must
        /// cover the open and close of the admitted IM horizon.
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<(finstack_quant_core::wire::DateWire, f64)>")
        )]
        knots: Vec<(Date, f64)>,
    },
}

/// Project dated expected posted IM from one admitted historical SIMM calculation.
///
/// # Arguments
/// * `calculator` - Explicit parameter version and margin period of risk.
/// * `sensitivities` - Original signed SIMM sensitivities of the admitted netting set.
/// * `spec` - Dated observation grid, reporting currency, admitted scope and decay election.
///
/// # Errors
/// Rejects missing evidence, an invalid clock or decay, a missing within-horizon
/// runoff node, or a failed SIMM calculation.
pub fn im_profile_from_simm(
    calculator: &SimmCalculator,
    sensitivities: &SimmSensitivities,
    spec: &ImProjectionSpec,
) -> finstack_quant_core::Result<ImProfile> {
    spec.decay.validate()?;
    let result =
        calculator.calculate_from_sensitivities(sensitivities, spec.currency, spec.as_of)?;
    let times = spec
        .dates
        .iter()
        .map(|date| elapsed_years(spec.as_of, *date))
        .collect::<Vec<_>>();
    let maturity = match &spec.decay {
        ImDecayProfile::Constant => None,
        ImDecayProfile::LinearToMaturity { maturity_years }
        | ImDecayProfile::SqrtTime { maturity_years } => Some(*maturity_years),
    };
    if let Some(maturity) = maturity
        && times.last().is_some_and(|&horizon| maturity <= horizon)
        && !times.iter().any(|&time| time.total_cmp(&maturity).is_eq())
    {
        return Err(Error::Validation(
            "The within-horizon IM runoff date must appear on the dated grid".into(),
        ));
    }
    if spec.scope_source.trim().is_empty() {
        return Err(Error::Validation(
            "IM projection requires an identified source margin record".into(),
        ));
    }
    let source_id = finstack_quant_core::canonical::content_hash(&(
        spec,
        calculator.version(),
        crate::types::SimmSensitivitiesJson::from(sensitivities),
        &result,
    ))?;
    let profile = ImProfile {
        as_of: spec.as_of,
        currency: spec.currency,
        basis: ImProjectionBasis::SimmDecay {
            source_id,
            netting_set: spec.netting_set.clone(),
            version: calculator.version(),
            decay: spec.decay.clone(),
        },
        dates: spec.dates.clone(),
        im_values: times
            .iter()
            .map(|&time| result.amount.amount() * spec.decay.factor(time))
            .collect(),
    };
    profile.validate()?;
    Ok(profile)
}

impl MarginFundingSpread {
    pub(super) fn add_knots(
        &self,
        start: Date,
        end: Date,
        dates: &mut Vec<Date>,
    ) -> finstack_quant_core::Result<()> {
        let valid_spread = |spread: f64| spread.is_finite() && spread >= 0.0;
        match self {
            Self::Flat { spread_bp } if valid_spread(*spread_bp) => Ok(()),
            Self::Dated { knots }
                if knots.first().is_some_and(|(date, _)| *date <= start)
                    && knots.last().is_some_and(|(date, _)| *date >= end)
                    && knots.windows(2).all(|pair| pair[0].0 < pair[1].0)
                    && knots.iter().all(|(_, spread)| valid_spread(*spread)) =>
            {
                dates.extend(knots.iter().filter_map(|&(date, _)| (start < date && date < end).then_some(date)));
                Ok(())
            }
            _ => Err(Error::Validation("MVA spread must be finite and nonnegative and any dated knots must strictly increase and cover the IM horizon".into())),
        }
    }

    fn spread_bp_at(&self, date: Date) -> f64 {
        match self {
            Self::Flat { spread_bp } => *spread_bp,
            Self::Dated { knots } => dated_interpolate(knots, date),
        }
    }
}

// Admitted schedules cover every date passed to this helper.
fn dated_interpolate(knots: &[(Date, f64)], date: Date) -> f64 {
    match knots.binary_search_by_key(&date, |&(day, _)| day) {
        Ok(index) => knots[index].1,
        Err(upper) => {
            let (start, lo) = knots[upper - 1];
            let (end, hi) = knots[upper];
            let alpha = (date - start).whole_days() as f64 / (end - start).whole_days() as f64;
            (1.0 - alpha) * lo + alpha * hi
        }
    }
}

/// Full set of observed dated factors at one end of an MVA interval.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MvaPoint {
    /// Calendar date at which each curve is sampled on its own clock.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Expected posted IM in major units of the result currency.
    pub expected_im: f64,
    /// Elected funding spread in basis points.
    pub spread_bp: f64,
    /// Risk-free discount factor relative to the valuation date.
    pub discount_factor: f64,
    /// Institution's survival probability relative to valuation, or one when excluded.
    pub own_survival: f64,
    /// Counterparty survival relative to valuation, or one when excluded.
    pub counterparty_survival: f64,
}

impl MvaPoint {
    fn factors(&self) -> [f64; 5] {
        [
            self.expected_im,
            self.spread_bp / BASIS_POINTS_PER_UNIT,
            self.discount_factor,
            self.own_survival,
            self.counterparty_survival,
        ]
    }
}

/// Replayable interval on the merged IM and funding-spread grid.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MvaInterval {
    /// Factors at the start of the interval.
    pub start: MvaPoint,
    /// Factors at the end of the interval.
    pub end: MvaPoint,
    /// Year fraction of this interval on ACT/365 Fixed.
    pub years: f64,
    /// Funding cost contributed by this interval, in profile currency.
    pub contribution: f64,
}

/// Posted-margin funding cost, coverage, and arithmetic evidence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MvaResult {
    /// Expected funding cost in reporting currency per calculation horizon.
    pub mva: f64,
    /// Mean IM weighted only by elapsed time, in reporting currency.
    pub average_im: f64,
    /// Dated and identified expected posted IM used for the cost.
    pub im_profile: ImProfile,
    /// Exact merged grid with start and end factors and costs contributed.
    pub intervals: Vec<MvaInterval>,
}

/// Integrate posted-IM funding with an explicit own-survival election.
///
/// # Arguments
/// * `im_profile` - Complete dated IM in the receiving currency, including opening IM.
/// * `funding_spread` - Explicit flat or bounded dated spread, stated in basis points.
/// * `discount_curve` - Risk-free curve in the profile currency, sampled on its own calendar clock.
/// * `own_survival` - Institution's hazard curve, or `None` for no survival weighting.
///
/// # Errors
/// Rejects unsupported input coverage, invalid curve anchors or domains and non-finite arithmetic.
pub fn compute_mva(
    im_profile: &ImProfile,
    funding_spread: &MarginFundingSpread,
    discount_curve: &DiscountCurve,
    own_survival: Option<&HazardCurve>,
) -> finstack_quant_core::Result<MvaResult> {
    compute_mva_internal(
        im_profile,
        funding_spread,
        discount_curve,
        own_survival,
        None,
    )
}

pub(super) fn compute_mva_internal(
    im_profile: &ImProfile,
    funding_spread: &MarginFundingSpread,
    discount_curve: &DiscountCurve,
    own_survival: Option<&HazardCurve>,
    counterparty_survival: Option<&HazardCurve>,
) -> finstack_quant_core::Result<MvaResult> {
    use finstack_quant_core::math::summation::NeumaierAccumulator;

    im_profile.validate()?;
    let mut dates = im_profile.dates.clone();
    let as_of = im_profile.as_of;
    let horizon = dates[dates.len() - 1];
    funding_spread.add_knots(as_of, horizon, &mut dates)?;
    dates.sort_unstable();
    dates.dedup();

    let currency = im_profile.currency;
    let discount = super::dated::discount_factors(as_of, currency, &dates, discount_curve)?;
    let own = super::dated::survival_factors(as_of, currency, &dates, own_survival)?;
    let counterparty =
        super::dated::survival_factors(as_of, currency, &dates, counterparty_survival)?;
    let im_knots: Vec<_> = im_profile
        .dates
        .iter()
        .copied()
        .zip(im_profile.im_values.iter().copied())
        .collect();
    let points = dates
        .iter()
        .enumerate()
        .map(|(i, &date)| MvaPoint {
            date,
            expected_im: dated_interpolate(&im_knots, date),
            spread_bp: funding_spread.spread_bp_at(date),
            discount_factor: discount[i],
            own_survival: own[i],
            counterparty_survival: counterparty[i],
        })
        .collect::<Vec<_>>();

    let mut funding_cost = NeumaierAccumulator::new();
    let mut time_weighted_im = NeumaierAccumulator::new();
    let mut intervals = Vec::with_capacity(points.len() - 1);
    let mut points = points.into_iter();
    let mut start = points
        .next()
        .ok_or_else(|| Error::Validation("MVA requires an opening point".into()))?;
    for end in points {
        let years = elapsed_years(start.date, end.date);
        let contribution = integral_factor_product(start.factors(), end.factors(), years)?;
        funding_cost.add(contribution)?;
        time_weighted_im.add((0.5 * start.expected_im + 0.5 * end.expected_im) * years)?;
        let interval = MvaInterval {
            start,
            end,
            years,
            contribution,
        };
        start = interval.end.clone();
        intervals.push(interval);
    }
    let average_im = time_weighted_im.get_total()? / elapsed_years(as_of, horizon);
    if !average_im.is_finite() {
        return Err(Error::Validation("MVA average IM must be finite".into()));
    }
    Ok(MvaResult {
        mva: funding_cost.get_total()?,
        average_im,
        im_profile: im_profile.clone(),
        intervals,
    })
}

/// Average of the product of five factors linearly interpolated inside a bucket.
///
/// Three-point Gauss-Legendre integrates the resulting polynomial (degree at
/// most five) exactly. Each nonnegative factor retains its curve's dated
/// endpoint values.
fn integral_factor_product(
    start: [f64; 5],
    end: [f64; 5],
    years: f64,
) -> finstack_quant_core::Result<f64> {
    if !years.is_finite()
        || years <= 0.0
        || start
            .iter()
            .chain(&end)
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(finstack_quant_core::Error::Validation(
            "MVA interval requires finite nonnegative endpoints and positive year fraction".into(),
        ));
    }
    let offset = 0.5 * (3.0_f64 / 5.0).sqrt();
    let nodes = [0.5 - offset, 0.5, 0.5 + offset];
    let weights = [5.0 / 18.0, 4.0 / 9.0, 5.0 / 18.0];
    let mut integral = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    for (node, weight) in nodes.into_iter().zip(weights) {
        let product = start.iter().zip(end).fold(1.0, |product, (&lo, hi)| {
            product * ((1.0 - node) * lo + node * hi)
        });
        integral.add(weight * years * product)?;
    }
    integral.get_total()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimmVersion;
    use crate::types::NettingSetId;
    use finstack_quant_core::dates::DayCount;
    use time::macros::date;

    const AS_OF: Date = date!(2025 - 01 - 01);
    const END: Date = date!(2026 - 01 - 01);
    const HALFWAY: Date = date!(2025 - 07 - 02);

    fn profile(dates: Vec<Date>, im_values: Vec<f64>) -> ImProfile {
        ImProfile {
            as_of: AS_OF,
            currency: Currency::USD,
            basis: ImProjectionBasis::External {
                source_id: "established-expected-IM".into(),
                netting_set: NettingSetId::bilateral("dealer", "agreement"),
            },
            dates,
            im_values,
        }
    }

    #[test]
    fn checked_decay_elects_and_preserves_the_opening_simm_amount() {
        let calculator = SimmCalculator::new(SimmVersion::V2_6).expect("calculator");
        let mut sensitivities = SimmSensitivities::new(Currency::USD);
        sensitivities.add_ir_delta(Currency::USD, "5Y", 50_000.0);
        let base = calculator
            .calculate_from_sensitivities(&sensitivities, Currency::USD, AS_OF)
            .expect("SIMM fixture")
            .amount
            .amount();
        let spec = ImProjectionSpec {
            as_of: AS_OF,
            currency: Currency::USD,
            netting_set: NettingSetId::bilateral("dealer", "agreement"),
            scope_source: "simm-execution".into(),
            dates: vec![AS_OF, HALFWAY, END],
            decay: ImDecayProfile::linear_to_maturity(1.0).expect("one year"),
        };
        let projected =
            im_profile_from_simm(&calculator, &sensitivities, &spec).expect("dated profile");
        assert!(base > 0.0);
        assert_eq!(projected.im_values[0], base);
        assert!((projected.im_values[1] - base * (1.0 - 182.0 / 365.0)).abs() < 1e-9);
        assert_eq!(projected.im_values[2], 0.0);
        let bad = ImProjectionSpec {
            dates: vec![AS_OF, HALFWAY, date!(2027 - 01 - 01)],
            ..spec
        };
        assert!(im_profile_from_simm(&calculator, &sensitivities, &bad).is_err());
    }

    #[test]
    fn mva_integrates_crossing_profile_and_spread_as_one_product() {
        let discount =
            DiscountCurve::flat("usd", AS_OF, 0.0, DayCount::Act365F).expect("unit discount");
        // All three observations follow the same linear spread. The interior
        // funding knot is absent from the IM observations and must split the result.
        let spread = MarginFundingSpread::Dated {
            knots: vec![(AS_OF, 0.0), (HALFWAY, 100.0 * 182.0 / 365.0), (END, 100.0)],
        };
        let im = profile(vec![AS_OF, END], vec![100.0, 0.0]);
        let result = compute_mva(&im, &spread, &discount, None).expect("qualified cost");
        assert!((result.mva - 1.0 / 6.0).abs() < 1e-14);
        assert!((result.average_im - 50.0).abs() < 1e-12);
        assert_eq!(result.intervals.len(), 2);
        assert_eq!(result.intervals[0].end.date, HALFWAY);
        assert!(
            (result
                .intervals
                .iter()
                .map(|row| row.contribution)
                .sum::<f64>()
                - result.mva)
                .abs()
                < 1e-15
        );
    }

    #[test]
    fn mva_curve_sampling_converges_with_a_finer_dated_grid() {
        let origin = date!(2024 - 12 - 20);
        let discount =
            DiscountCurve::flat("usd", origin, 0.04, DayCount::Act365F).expect("discount");
        let own = HazardCurve::flat("own", origin, 0.07, 0.4, DayCount::Act360).expect("hazard");
        let spread = MarginFundingSpread::Flat { spread_bp: 50.0 };
        let coarse = profile(vec![AS_OF, END], vec![10_000.0; 2]);
        let dense = profile(
            (0..=365)
                .map(|days| AS_OF + time::Duration::days(days))
                .collect(),
            vec![10_000.0; 366],
        );
        let coarse = compute_mva(&coarse, &spread, &discount, Some(&own)).expect("one year");
        let dense = compute_mva(&dense, &spread, &discount, Some(&own)).expect("daily");
        let rate: f64 = 0.04 + 0.07 * 365.0 / 360.0;
        let reference = 50.0 * (-(-rate).exp_m1()) / rate;
        assert!((dense.mva - reference).abs() < 1e-5);
        assert!((dense.mva - reference).abs() < (coarse.mva - reference).abs());
        assert!((dense.average_im - 10_000.0).abs() < 1e-11);
        assert_eq!(dense.intervals[0].start.discount_factor, 1.0);
        assert_eq!(dense.intervals[0].start.own_survival, 1.0);
    }

    #[test]
    fn mva_rejects_missing_opening_and_spread_coverage() {
        let discount = DiscountCurve::flat("usd", AS_OF, 0.0, DayCount::Act365F).expect("discount");
        let short_spread = MarginFundingSpread::Dated {
            knots: vec![(AS_OF, 10.0), (HALFWAY, 50.0)],
        };
        let flat = MarginFundingSpread::Flat { spread_bp: 50.0 };
        assert!(
            compute_mva(
                &profile(vec![HALFWAY, END], vec![2.0, 1.0]),
                &flat,
                &discount,
                None
            )
            .is_err()
        );
        assert!(
            compute_mva(
                &profile(vec![AS_OF, END], vec![2.0, -1.0]),
                &flat,
                &discount,
                None
            )
            .is_err()
        );
        assert!(
            compute_mva(
                &profile(vec![AS_OF, END], vec![2.0, 1.0]),
                &short_spread,
                &discount,
                None
            )
            .is_err()
        );
    }
    #[test]
    fn decay_factors_match_definitions() {
        assert!((ImDecayProfile::Constant.factor(7.3) - 1.0).abs() < 1e-15);
        let lin = ImDecayProfile::LinearToMaturity {
            maturity_years: 2.0,
        };
        assert!((lin.factor(0.0) - 1.0).abs() < 1e-15);
        assert!((lin.factor(1.0) - 0.5).abs() < 1e-15);
        assert!(lin.factor(3.0).abs() < 1e-15); // clamped at 0 past maturity
        let sq = ImDecayProfile::SqrtTime {
            maturity_years: 2.0,
        };
        assert!((sq.factor(1.0) - 0.5f64.sqrt()).abs() < 1e-15);
        assert!(sq.factor(2.0).abs() < 1e-15);
    }

    #[test]
    fn decay_validate_rejects_bad_maturity() {
        assert!(
            ImDecayProfile::LinearToMaturity {
                maturity_years: 0.0
            }
            .validate()
            .is_err()
        );
        assert!(
            ImDecayProfile::SqrtTime {
                maturity_years: f64::NAN
            }
            .validate()
            .is_err()
        );
        assert!(ImDecayProfile::Constant.validate().is_ok());
    }

    #[test]
    fn decay_profile_constructors_validate_the_maturity() {
        assert_eq!(
            ImDecayProfile::linear_to_maturity(5.0).expect("positive maturity"),
            ImDecayProfile::LinearToMaturity {
                maturity_years: 5.0
            }
        );
        assert_eq!(
            ImDecayProfile::sqrt_time(4.0)
                .expect("positive maturity")
                .factor(3.0),
            0.5
        );
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(ImDecayProfile::linear_to_maturity(bad).is_err());
            assert!(ImDecayProfile::sqrt_time(bad).is_err());
        }
    }
}
