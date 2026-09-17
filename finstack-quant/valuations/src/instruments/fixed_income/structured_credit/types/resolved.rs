//! Canonical precedence for effective deal behavior.

use super::{CreditModelConfig, DefaultModelSpec, PrepaymentModelSpec, StructuredCredit};
use finstack_quant_core::validation::validate_f64_unit_interval;
use finstack_quant_core::Result;

impl StructuredCredit {
    /// Resolve explicit behavioral overrides into the model consumed by pricing.
    /// A rate/curve override fixes that leg's behavior, including in a stochastic
    /// run; stochastic model specifications apply only to unoverridden legs.
    pub(crate) fn effective_credit_model(&self) -> CreditModelConfig {
        let mut model = self.credit_model.clone();
        let overrides = &self.behavior_overrides;
        let prepayment = if let Some(cpr) = overrides.cpr_annual {
            Some(PrepaymentModelSpec::constant_cpr(cpr))
        } else {
            overrides.psa_speed_multiplier.map(PrepaymentModelSpec::psa)
        };
        if let Some(spec) = prepayment {
            model.stochastic_prepay_spec = None;
            model.prepayment_spec = spec;
        }
        let default = if let Some(cdr) = overrides.cdr_annual {
            Some(DefaultModelSpec::constant_cdr(cdr))
        } else {
            overrides.sda_speed_multiplier.map(DefaultModelSpec::sda)
        };
        if let Some(spec) = default {
            model.stochastic_default_spec = None;
            model.default_spec = spec;
        }
        if let Some(rate) = overrides.recovery_rate {
            model.recovery_spec.rate = rate;
        }
        if let Some(months) = overrides.recovery_lag_months {
            model.recovery_spec.recovery_lag = months;
        }
        model
    }

    /// Validate the effective model before it reaches a numerical engine.
    pub(crate) fn resolved_credit_model(&self) -> Result<CreditModelConfig> {
        let overrides = &self.behavior_overrides;
        if let Some(cpr) = overrides.cpr_annual {
            validate_f64_unit_interval(cpr, "structured-credit annual CPR override")?;
        }
        if let Some(cdr) = overrides.cdr_annual {
            validate_f64_unit_interval(cdr, "structured-credit annual CDR override")?;
        }
        let model = self.effective_credit_model();
        // Include peak seasoning to validate an entire PSA/SDA curve, not just
        // the zero-time rate where a malformed multiplier can be hidden.
        validate_f64_unit_interval(model.prepayment_spec.cpr, "canonical CPR")?;
        model.prepayment_spec.validate()?;
        model.default_spec.validate()?;
        model.recovery_spec.validate()?;
        Ok(model)
    }

    /// Copy the deal with effective assumptions made explicit for scenario/risk
    /// mutations. Clearing resolved rate overrides prevents them masking shocks.
    pub(crate) fn resolved_for_pricing(&self) -> Result<Self> {
        if self.closing_date >= self.maturity
            || self.first_payment_date <= self.closing_date
            || self.first_payment_date > self.maturity
        {
            return Err(finstack_quant_core::Error::Validation(
                "structured-credit dates require closing < first payment <= maturity".into(),
            ));
        }
        let mut deal = self.clone();
        deal.credit_model = self.resolved_credit_model()?;
        deal.pool = self.pool.normalized(self.closing_date)?;
        deal.pool.validate_reserve_config(&deal.tranches)?;
        for asset in &deal.pool.assets {
            if asset.is_defaulted {
                if asset.default_date.is_none() || asset.recovery_amount.is_none() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "defaulted asset {} requires default_date and outstanding recovery_amount",
                        asset.id
                    )));
                }
                let recovery = asset.recovery_amount.unwrap_or(asset.balance);
                if recovery.currency() != asset.balance.currency()
                    || recovery.amount() < 0.0
                    || recovery.amount() > asset.balance.amount()
                {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "invalid recovery claim for {}",
                        asset.id
                    )));
                }
            } else if asset.default_date.is_some() || asset.recovery_amount.is_some() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "performing asset {} cannot carry a default claim",
                    asset.id
                )));
            }
        }
        if let Some(price) = deal.behavior_overrides.reinvestment_price {
            if !price.is_finite() || price <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(
                    "reinvestment price must be finite and positive percent of par".into(),
                ));
            }
        }
        if let Some(period) = &deal.pool.reinvestment_period {
            if period.end_date < deal.closing_date
                || period.end_date > deal.maturity
                || !period.criteria.max_price.is_finite()
                || period.criteria.max_price <= 0.0
                || !period.criteria.min_yield.is_finite()
            {
                return Err(finstack_quant_core::Error::Validation(
                    "invalid reinvestment end date or price/current-yield criteria".into(),
                ));
            }
            for id in &period.amortizing_tranches {
                match deal.tranches.tranches.iter().find(|t| t.id.as_str() == id) {
                    None => {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "amortizing tranche {id} is not a tranche of the deal"
                        )));
                    }
                    Some(tranche) if tranche.seniority == super::TrancheSeniority::Equity => {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "amortizing tranche {id} is the equity class"
                        )));
                    }
                    Some(_) => {}
                }
            }
            if let Some(assumptions) = &period.assumptions {
                if !assumptions.spread_bp.is_finite()
                    || !assumptions.price_pct.is_finite()
                    || assumptions.price_pct <= 0.0
                    || assumptions.price_pct > period.criteria.max_price
                    || assumptions.maturity_months == 0
                    || assumptions
                        .coupon_floor
                        .is_some_and(|floor| !floor.is_finite())
                {
                    return Err(finstack_quant_core::Error::Validation(
                        "reinvestment assumptions need a finite spread, a positive price within \
                         the eligibility maximum, a positive maturity and a finite floor"
                            .into(),
                    ));
                }
            }
        }
        deal.behavior_overrides.cpr_annual = None;
        deal.behavior_overrides.psa_speed_multiplier = None;
        deal.behavior_overrides.cdr_annual = None;
        deal.behavior_overrides.sda_speed_multiplier = None;
        deal.behavior_overrides.recovery_rate = None;
        deal.behavior_overrides.recovery_lag_months = None;
        Ok(deal)
    }
}
