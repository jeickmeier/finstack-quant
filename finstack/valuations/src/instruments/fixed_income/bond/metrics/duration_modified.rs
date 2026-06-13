use crate::instruments::Bond;
use crate::instruments::BondRiskBasis;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};

/// Calculates modified duration for bonds.
///
/// For bonds **without** embedded options, computes yield-based modified duration:
/// ```text
/// D_mod = D_mac / (1 + y/m)
/// ```
///
/// For bonds **with** embedded options (callable/putable), computes effective
/// duration via parallel curve bumps, which properly accounts for changes in
/// exercise behavior as rates shift.
///
/// # Dependencies
///
/// Requires `DurationMac` and `Ytm` metrics for straight bonds.
pub(crate) struct ModifiedDurationCalculator;

impl MetricCalculator for ModifiedDurationCalculator {
    fn dependencies(&self) -> &[MetricId] {
        // Ytm is required for straight bonds in addition to DurationMac. Declaring
        // it explicitly lets the topological resolver schedule Ytm ahead of us even
        // when no other metric in the batch depends on it.
        &[MetricId::DurationMac, MetricId::Ytm]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;

        // Callable/OAS model risk is opt-in. The default matches Bloomberg YAS
        // Workout risk: quoted-yield duration on maturity/workout cashflows.
        let has_options = bond.call_put.as_ref().is_some_and(|cp| cp.has_options());
        if has_options && super::bond_risk_basis(context) == BondRiskBasis::CallableOas {
            return super::effective::effective_duration(
                bond,
                context.curves.as_ref(),
                context.as_of,
                None,
            );
        }

        let ytm = context
            .computed
            .get(&MetricId::Ytm)
            .copied()
            .ok_or_else(|| {
                finstack_core::Error::from(finstack_core::InputError::NotFound {
                    id: "metric:Ytm".to_string(),
                })
            })?;

        let d_mac = context
            .computed
            .get(&MetricId::DurationMac)
            .copied()
            .ok_or_else(|| {
                finstack_core::Error::from(finstack_core::InputError::NotFound {
                    id: "metric:DurationMac".to_string(),
                })
            })?;

        let denominator_yield = if has_options {
            if let Some(flows) = context.cashflows.as_ref() {
                if let Some((workout_yield, _, _)) =
                    super::quoted_workout_path(bond, context.curves.as_ref(), context.as_of, flows)?
                {
                    workout_yield
                } else {
                    ytm
                }
            } else {
                ytm
            }
        } else {
            ytm
        };

        let m =
            crate::instruments::fixed_income::bond::pricing::quote_conversions::periods_per_year(
                bond.cashflow_spec.frequency(),
            )?
            .max(1.0);
        let denom = 1.0 + denominator_yield / m;
        if denom.abs() < 1e-12 {
            return Err(finstack_core::Error::Validation(
                "Modified duration undefined when 1 + ytm/m is near zero".to_string(),
            ));
        }
        Ok(d_mac / denom)
    }
}
