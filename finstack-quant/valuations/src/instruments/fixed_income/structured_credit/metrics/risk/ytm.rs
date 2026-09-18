//! YTM (Yield to Maturity) calculator for structured credit.

use crate::instruments::fixed_income::structured_credit::types::constants::YTM_SOLVER_TOLERANCE;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;
use serde::Deserialize;

/// Extension key for structured-credit YTM settings.
pub(crate) const STRUCTURED_CREDIT_YTM_CONFIG_KEY_V1: &str = "valuations.structured_credit.ytm.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
enum YtmCompounding {
    #[default]
    Annual,
    SemiAnnual,
    Quarterly,
    Monthly,
}

impl YtmCompounding {
    fn periods_per_year(self) -> f64 {
        match self {
            Self::Annual => 1.0,
            Self::SemiAnnual => 2.0,
            Self::Quarterly => 4.0,
            Self::Monthly => 12.0,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredCreditYtmConfigV1 {
    #[serde(default)]
    compounding: Option<YtmCompounding>,
}

/// Calculates YTM (Yield to Maturity) for structured credit.
///
/// YTM is the internal rate of return that equates the present value of
/// all future cashflows to the current price. For structured credit, this
/// is most relevant for fixed-rate tranches.
///
/// # Formula
///
/// Solve for y such that:
/// ```text
/// Σ CF_i / (1 + y)^t_i = Dirty Price
/// ```
///
/// # Compounding Convention
///
/// When the context values one tranche (the usual per-note path), the yield
/// compounds at that note's coupon frequency — quarterly for a quarterly
/// note, `(1 + y/4)^(-4t)` — and measures `t` in the note's own day count,
/// so a par note yields its coupon. The
/// `valuations.structured_credit.ytm.v1` extension's `compounding`
/// (`annual`, `semi_annual`, `quarterly`, `monthly`) overrides the frequency;
/// deal-level contexts without a tranche in scope compound annually in
/// Act/365F. Market conventions differ by sector (ABS and CMBS often quote a
/// semi-annual bond-equivalent yield, RMBS a monthly mortgage yield): set
/// the extension when a quote must follow one of them.
///
/// # Typical Yield Ranges
///
/// - **ABS (fixed)**: 4-7% typical for AAA
/// - **RMBS (fixed)**: 4-6% typical for agency
/// - **CMBS (fixed)**: 5-7% typical
/// - **CLO (floating)**: Less meaningful (use Z-spread instead)
///
/// # Note
///
/// For structured credit, **Z-spread is generally more important than YTM**
/// because it properly accounts for the term structure of rates.
///
pub struct YtmCalculator;

impl YtmCalculator {
    fn compounding_from_config(
        context: &MetricContext,
    ) -> finstack_quant_core::Result<Option<YtmCompounding>> {
        if let Some(raw) = context
            .get_config()
            .extensions
            .get(STRUCTURED_CREDIT_YTM_CONFIG_KEY_V1)
        {
            let cfg: StructuredCreditYtmConfigV1 =
                serde_json::from_value(raw.clone()).map_err(|e| {
                    finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Failed to parse extension '{}': {}",
                            STRUCTURED_CREDIT_YTM_CONFIG_KEY_V1, e
                        ),
                        category: "config".to_string(),
                    }
                })?;
            return Ok(cfg.compounding);
        }
        Ok(None)
    }

    /// Compounding periods per year and the time basis for the yield: the
    /// configured compounding when set, else the in-scope note's coupon
    /// frequency (annual when no note is in scope), with time measured in
    /// the note's day count (Act/365F without a note).
    fn compounding_basis(
        context: &MetricContext,
        deal: &crate::instruments::fixed_income::structured_credit::StructuredCredit,
    ) -> finstack_quant_core::Result<(f64, finstack_quant_core::dates::DayCount)> {
        let tranche = context
            .detailed_tranche_cashflows
            .as_ref()
            .and_then(|flows| {
                deal.tranches
                    .tranches
                    .iter()
                    .find(|t| t.id.as_str() == flows.tranche_id)
            });
        let day_count = tranche.map_or(finstack_quant_core::dates::DayCount::Act365F, |t| {
            t.day_count
        });
        let periods_per_year = match Self::compounding_from_config(context)? {
            Some(compounding) => compounding.periods_per_year(),
            None => tranche
                .and_then(|t| t.frequency.months())
                .filter(|months| *months > 0)
                .map_or(1.0, |months| 12.0 / f64::from(months)),
        };
        Ok((periods_per_year, day_count))
    }
}

impl MetricCalculator for YtmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let dirty_price = context
            .computed
            .get(&MetricId::DirtyPrice)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:DirtyPrice".to_string(),
                })
            })?;

        let quote = super::super::quote::SettlementQuote::from_context(context)?;
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        // Convert price points back to currency using original notional.
        let notional = crate::instruments::fixed_income::structured_credit::metrics::pricing::prices::quote_notional(context)?;
        let target_value = quote.external_target(context.instrument_as::<crate::instruments::fixed_income::structured_credit::StructuredCredit>()?)?.unwrap_or(notional * (dirty_price / 100.0));

        let settlement = super::super::quote::settlement_date(context.instrument_as::<crate::instruments::fixed_income::structured_credit::StructuredCredit>()?, context.as_of)?;
        if !flows
            .iter()
            .any(|(date, amount)| *date > settlement && amount.amount() > 0.0)
            || target_value <= 0.0
        {
            return Err(finstack_quant_core::Error::Validation(
                "structured-credit yield requires a positive settlement target and future cashflow"
                    .into(),
            ));
        }

        // Day count for year fractions
        let (periods_per_year, day_count) = Self::compounding_basis(
            context,
            context.instrument_as::<crate::instruments::fixed_income::structured_credit::StructuredCredit>()?,
        )?;

        // Objective function: PV(y) - target = 0
        let objective = |y: f64| -> f64 {
            let base = 1.0 + y / periods_per_year;
            if base <= 0.0 {
                return f64::INFINITY;
            }
            let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
            for (date, amount) in flows {
                if *date <= settlement {
                    continue;
                }

                // SC-m09: a failed date conversion previously became `0.0`,
                // and the `t > 0.0` guard below then DROPPED that cashflow from
                // the PV entirely — silently solving a yield against a
                // different bond. Skipping is only correct when the flow is
                // genuinely non-future; a conversion failure is a broken input
                // and must not masquerade as one.
                // SC-m09: a failed date conversion previously became `0.0`,
                // and the `t > 0.0` guard below then DROPPED that cashflow from
                // the PV entirely — silently solving a yield against a
                // different bond. Skipping is only correct for a genuinely
                // non-future flow; a conversion failure is a broken input.
                //
                // The objective must stay `f64`, so signal with NaN:
                // `BrentSolver::find_bracket` rejects a non-finite objective
                // and surfaces `SolverConvergenceFailed`, which is a loud
                // failure rather than a confident wrong number.
                let Ok(t) = day_count.year_fraction(settlement, *date, DayCountContext::default())
                else {
                    return f64::NAN;
                };

                if t > 0.0 {
                    let df = base.powf(-periods_per_year * t);
                    pv.add(amount.amount() * df);
                }
            }
            pv.total() - target_value
        };

        // Solve for YTM using Brent solver
        // Tolerance: 1e-6 = 0.01 bp precision (market standard)
        let solver = BrentSolver::new().tolerance(YTM_SOLVER_TOLERANCE);

        // Initial guess: 5% is reasonable for structured credit
        let ytm = solver.solve(objective, 0.05)?;

        Ok(ytm)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::DirtyPrice]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ytm_compounding_periods_per_year() {
        assert_eq!(YtmCompounding::Annual.periods_per_year(), 1.0);
        assert_eq!(YtmCompounding::SemiAnnual.periods_per_year(), 2.0);
        assert_eq!(YtmCompounding::Quarterly.periods_per_year(), 4.0);
        assert_eq!(YtmCompounding::Monthly.periods_per_year(), 12.0);
    }

    #[test]
    fn ytm_config_deserializes_compounding() {
        let raw = serde_json::json!({ "compounding": "quarterly" });
        let parsed: std::result::Result<StructuredCreditYtmConfigV1, _> =
            serde_json::from_value(raw);
        assert!(
            parsed.is_ok(),
            "config should deserialize, got {:?}",
            parsed.err()
        );
        if let Ok(cfg) = parsed {
            assert_eq!(cfg.compounding, Some(YtmCompounding::Quarterly));
        }
    }
}
