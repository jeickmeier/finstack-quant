//! Schedule construction and floating-rate projection for revolving credit facilities.

mod schedule;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use schedule::build_accrual_boundary_dates;
pub(crate) use schedule::{
    RevolverFloatingProjection, apply_draw_repay_event, build_observation_dates,
    build_payment_dates, build_payment_periods, build_reset_dates, fixed_rate_at,
    floating_fixing_date, floating_params_at, index_basis_at, project_revolver_floating_rate,
    resolve_fixings, resolved_overnight_compounding, resolved_overnight_compounding_for_index,
};
