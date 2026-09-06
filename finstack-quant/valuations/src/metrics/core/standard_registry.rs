//! Standard metric registry bootstrap.

use std::sync::{Arc, OnceLock};

use super::{
    ids::MetricId,
    registry::{MetricRegistry, MetricRegistryError},
    traits::{MetricCalculator, MetricContext},
};
use crate::metrics::risk::{GenericExpectedShortfall, GenericHVar, VarConfig};
use crate::metrics::sensitivities::breakeven::BreakevenCalculator;
use crate::metrics::sensitivities::carry_decomposition::CarryDecompositionCalculator;
use crate::metrics::sensitivities::theta::GenericThetaAny;

static STANDARD_REGISTRY: OnceLock<MetricRegistry> = OnceLock::new();

struct ComputedMetricLookup {
    metric: MetricId,
    dependencies: [MetricId; 1],
}

impl ComputedMetricLookup {
    fn new(metric: MetricId, dependency: MetricId) -> Self {
        Self {
            metric,
            dependencies: [dependency],
        }
    }
}

impl MetricCalculator for ComputedMetricLookup {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        context.computed.get(&self.metric).copied().ok_or_else(|| {
            finstack_quant_core::InputError::NotFound {
                id: format!("metric:{}", self.metric),
            }
            .into()
        })
    }

    fn dependencies(&self) -> &[MetricId] {
        &self.dependencies
    }
}

/// Creates a standard metric registry with all built-in metrics.
///
/// This registry includes metrics for:
/// - **Bonds**: YTM, duration, convexity, accrued interest, credit spreads
/// - **Interest Rate Swaps**: DV01, annuity factors, par rates
/// - **Deposits**: Discount factors, par rates, year fractions
/// - **Risk**: Bucketed DV01, time decay (theta)
///
/// See unit tests and `examples/` for usage.
#[allow(clippy::expect_used)]
pub fn standard_registry() -> &'static MetricRegistry {
    STANDARD_REGISTRY.get_or_init(|| {
        build_standard_registry().expect("built-in metric registrations must be valid")
    })
}

fn build_standard_registry() -> std::result::Result<MetricRegistry, MetricRegistryError> {
    let mut registry = MetricRegistry::new();
    register_universal_metrics(&mut registry)?;
    register_equity_instrument_metrics(&mut registry)?;
    register_fixed_income_instrument_metrics(&mut registry)?;
    register_rates_instrument_metrics(&mut registry)?;
    register_credit_derivative_instrument_metrics(&mut registry)?;
    register_fx_instrument_metrics(&mut registry)?;
    register_commodity_instrument_metrics(&mut registry)?;
    register_exotic_instrument_metrics(&mut registry)?;
    Ok(registry)
}

macro_rules! register_modules {
    ($registry:expr, $($register:path),+ $(,)?) => {
        $(
            $register($registry)?;
        )+
    };
}

fn register_equity_instrument_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), MetricRegistryError> {
    register_modules!(
        registry,
        crate::instruments::equity::spot::metrics::register_equity_metrics,
        crate::instruments::equity::equity_option::metrics::register_equity_option_metrics,
        crate::instruments::equity::equity_future::metrics::register_equity_future_metrics,
        crate::instruments::equity::equity_total_return_future::metrics::register_equity_total_return_future_metrics,
        crate::instruments::equity::equity_future_option::metrics::register_equity_future_option_metrics,
        crate::instruments::equity::vol_index_future_option::metrics::register_vol_index_future_option_metrics,
        crate::instruments::equity::equity_trs::metrics::register_equity_trs_metrics,
        crate::instruments::equity::variance_swap::metrics::register_variance_swap_metrics,
        crate::instruments::equity::pe_fund::metrics::register_private_markets_fund_metrics,
        crate::instruments::equity::dcf_equity::metrics::register_dcf_metrics,
        crate::instruments::equity::vol_index_future::metrics::register_vol_index_future_metrics,
        crate::instruments::equity::real_estate::metrics::register_real_estate_metrics,
        crate::instruments::equity::real_estate::metrics::register_levered_real_estate_metrics,
        crate::instruments::exotics::basket::metrics::register_basket_metrics,
    );
    Ok(())
}

fn register_fixed_income_instrument_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), MetricRegistryError> {
    register_modules!(
        registry,
        crate::instruments::fixed_income::bond::metrics::register_bond_metrics,
        crate::instruments::fixed_income::bond_future::metrics::register_bond_future_metrics,
        crate::instruments::fixed_income::convertible::metrics::register_convertible_metrics,
        crate::instruments::fixed_income::mbs_passthrough::metrics::register_mbs_passthrough_metrics,
        crate::instruments::fixed_income::tba::metrics::register_tba_metrics,
        crate::instruments::fixed_income::dollar_roll::metrics::register_dollar_roll_metrics,
        crate::instruments::fixed_income::cmo::metrics::register_cmo_metrics,
        crate::instruments::fixed_income::inflation_linked_bond::metrics::register_ilb_metrics,
        crate::instruments::fixed_income::structured_credit::metrics::register_structured_credit_metrics,
        crate::instruments::fixed_income::term_loan::metrics::register_term_loan_metrics,
        crate::instruments::fixed_income::revolving_credit::metrics::register_revolving_credit_metrics,
        crate::instruments::fixed_income::fi_trs::metrics::register_fi_trs_metrics,
    );
    Ok(())
}

fn register_rates_instrument_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), MetricRegistryError> {
    register_modules!(
        registry,
        crate::instruments::rates::irs::metrics::register_irs_metrics,
        crate::instruments::rates::deposit::metrics::register_deposit_metrics,
        crate::instruments::rates::fra::metrics::register_fra_metrics,
        crate::instruments::rates::ir_future::metrics::register_ir_future_metrics,
        crate::instruments::rates::ir_future_option::metrics::register_interest_rate_future_option_metrics,
        crate::instruments::rates::inflation_swap::metrics::register_inflation_swap_metrics,
        crate::instruments::rates::inflation_cap_floor::metrics::register_inflation_cap_floor_metrics,
        crate::instruments::rates::cap_floor::metrics::register_cap_floor_metrics,
        crate::instruments::rates::swaption::metrics::register_swaption_metrics,
        crate::instruments::rates::xccy_swap::metrics::register_xccy_swap_metrics,
        crate::instruments::rates::repo::metrics::register_repo_metrics,
        crate::instruments::rates::basis_swap::metrics::register_basis_swap_metrics,
        crate::instruments::rates::cms_option::metrics::register_cms_option_metrics,
        crate::instruments::rates::cms_swap::metrics::register_cms_swap_metrics,
        crate::instruments::rates::cms_spread_option::metrics::register_cms_spread_option_metrics,
    );
    if let Ok(default_hw) =
        finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams::new(
            crate::instruments::rates::swaption::metrics::bermudan_greeks::DEFAULT_KAPPA,
            crate::instruments::rates::swaption::metrics::bermudan_greeks::DEFAULT_SIGMA,
        )
    {
        crate::instruments::rates::swaption::metrics::register_bermudan_swaption_metrics(
            registry, default_hw,
        )?;
    }
    Ok(())
}

fn register_credit_derivative_instrument_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), MetricRegistryError> {
    register_modules!(
        registry,
        crate::instruments::credit_derivatives::cds::metrics::register_cds_metrics,
        crate::instruments::credit_derivatives::cds_index::metrics::register_cds_index_metrics,
        crate::instruments::credit_derivatives::cds_tranche::metrics::register_cds_tranche_metrics,
        crate::instruments::credit_derivatives::cds_option::metrics::register_cds_option_metrics,
    );
    Ok(())
}

fn register_fx_instrument_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), MetricRegistryError> {
    register_modules!(
        registry,
        crate::instruments::fx::fx_spot::metrics::register_fx_spot_metrics,
        crate::instruments::fx::fx_swap::metrics::register_fx_swap_metrics,
        crate::instruments::fx::fx_forward::metrics::register_fx_forward_metrics,
        crate::instruments::fx::fx_future::metrics::register_fx_future_metrics,
        crate::instruments::fx::fx_future_option::metrics::register_fx_future_option_metrics,
        crate::instruments::fx::ndf::metrics::register_ndf_metrics,
        crate::instruments::fx::fx_option::metrics::register_fx_option_metrics,
        crate::instruments::fx::fx_variance_swap::metrics::register_fx_variance_swap_metrics,
        crate::instruments::fx::fx_barrier_option::metrics::register_fx_barrier_option_metrics,
        crate::instruments::fx::fx_digital_option::metrics::register_fx_digital_option_metrics,
        crate::instruments::fx::fx_touch_option::metrics::register_fx_touch_option_metrics,
    );
    Ok(())
}

fn register_commodity_instrument_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), MetricRegistryError> {
    register_modules!(
        registry,
        crate::instruments::commodity::commodity_forward::metrics::register_commodity_forward_metrics,
        crate::instruments::commodity::commodity_future::metrics::register_commodity_future_metrics,
        crate::instruments::commodity::commodity_future_option::metrics::register_commodity_future_option_metrics,
        crate::instruments::commodity::commodity_swap::metrics::register_commodity_swap_metrics,
        crate::instruments::commodity::commodity_option::metrics::register_commodity_option_metrics,
        crate::instruments::commodity::commodity_asian_option::metrics::register_commodity_asian_option_metrics,
        crate::instruments::commodity::commodity_swaption::metrics::register_commodity_swaption_metrics,
        crate::instruments::commodity::commodity_spread_option::metrics::register_commodity_spread_option_metrics,
    );
    Ok(())
}

fn register_exotic_instrument_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), MetricRegistryError> {
    register_modules!(
        registry,
        crate::instruments::exotics::lookback_option::metrics::register_lookback_option_metrics,
        crate::instruments::exotics::asian_option::metrics::register_asian_option_metrics,
        crate::instruments::equity::autocallable::metrics::register_autocallable_metrics,
        crate::instruments::exotics::barrier_option::metrics::register_barrier_option_metrics,
        crate::instruments::equity::cliquet_option::metrics::register_cliquet_option_metrics,
        crate::instruments::fx::quanto_option::metrics::register_quanto_option_metrics,
        crate::instruments::exotics::range_accrual::metrics::register_range_accrual_metrics,
        crate::instruments::exotics::tarn::metrics::register_tarn_metrics,
        crate::instruments::exotics::snowball::metrics::register_snowball_metrics,
        crate::instruments::exotics::callable_range_accrual::metrics::register_callable_range_accrual_metrics,
    );
    Ok(())
}
fn register_universal_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), super::registry::MetricRegistryError> {
    registry.register_metric(MetricId::Theta, Arc::new(GenericThetaAny), &[])?;
    registry.register_metric(
        MetricId::ThetaCarry,
        Arc::new(ComputedMetricLookup::new(
            MetricId::ThetaCarry,
            MetricId::Theta,
        )),
        &[],
    )?;
    registry.register_metric(
        MetricId::ThetaRollDown,
        Arc::new(ComputedMetricLookup::new(
            MetricId::ThetaRollDown,
            MetricId::Theta,
        )),
        &[],
    )?;
    registry.register_metric(
        MetricId::CarryTotal,
        Arc::new(CarryDecompositionCalculator),
        &[],
    )?;
    registry.register_metric(
        MetricId::CouponIncome,
        Arc::new(ComputedMetricLookup::new(
            MetricId::CouponIncome,
            MetricId::CarryTotal,
        )),
        &[],
    )?;
    registry.register_metric(
        MetricId::PullToPar,
        Arc::new(ComputedMetricLookup::new(
            MetricId::PullToPar,
            MetricId::CarryTotal,
        )),
        &[],
    )?;
    registry.register_metric(
        MetricId::RollDown,
        Arc::new(ComputedMetricLookup::new(
            MetricId::RollDown,
            MetricId::CarryTotal,
        )),
        &[],
    )?;
    registry.register_metric(
        MetricId::FundingCost,
        Arc::new(ComputedMetricLookup::new(
            MetricId::FundingCost,
            MetricId::CarryTotal,
        )),
        &[],
    )?;
    registry.register_metric(MetricId::Breakeven, Arc::new(BreakevenCalculator), &[])?;
    registry.register_metric(
        MetricId::HVar,
        Arc::new(GenericHVar::new(VarConfig::var_95())),
        &[],
    )?;
    registry.register_metric(
        MetricId::ExpectedShortfall,
        Arc::new(GenericExpectedShortfall::new(VarConfig::var_95())),
        &[],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn built_in_metric_owners_are_unique() {
        super::build_standard_registry().expect("each built-in metric slot must have one owner");
    }
}
