//! Validation and pool normalization that run before a deal is priced.

use super::{CreditModelConfig, StructuredCredit};
use finstack_quant_core::validation::validate_f64_unit_interval;
use finstack_quant_core::Result;

impl StructuredCredit {
    /// Validate `credit_model` before it reaches a numerical engine.
    pub(crate) fn validated_credit_model(&self) -> Result<&CreditModelConfig> {
        let model = &self.credit_model;
        // Include peak seasoning to validate an entire PSA/SDA curve, not just
        // the zero-time rate where a malformed multiplier can be hidden.
        validate_f64_unit_interval(
            model.prepayment_spec.cpr,
            "credit_model.prepayment_spec.cpr",
        )?;
        model.prepayment_spec.validate()?;
        model.default_spec.validate()?;
        model.recovery_spec.validate()?;
        Ok(model)
    }

    /// Copy the deal with a validated credit model and a normalized pool for
    /// scenario/risk mutations.
    pub(crate) fn resolved_for_pricing(&self) -> Result<Self> {
        self.validate_dates()?;
        self.validated_credit_model()?;
        let pool = self.pool.normalized(self.closing_date)?;
        self.validate_resolved_pool(&pool)?;
        let mut deal = self.clone();
        deal.pool = pool;
        Ok(deal)
    }

    /// Run every check [`Self::resolved_for_pricing`] runs without copying
    /// the deal (the pool is borrowed unless it must be normalized from
    /// representative lines or instrument collateral).
    pub(crate) fn validate_resolvable(&self) -> Result<()> {
        self.validate_dates()?;
        self.validated_credit_model()?;
        let pool = self.pool.normalized_view(self.closing_date)?;
        self.validate_resolved_pool(&pool)
    }

    fn validate_dates(&self) -> Result<()> {
        if self.closing_date >= self.maturity
            || self.first_payment_date <= self.closing_date
            || self.first_payment_date > self.maturity
        {
            return Err(finstack_quant_core::Error::Validation(
                "structured-credit dates require closing < first payment <= maturity".into(),
            ));
        }
        Ok(())
    }

    /// Checks on the normalized pool: reserve configuration, default claims,
    /// and the reinvestment period and its assumptions.
    fn validate_resolved_pool(&self, pool: &super::AssetPool) -> Result<()> {
        pool.validate_reserve_config(&self.tranches)?;
        for asset in &pool.assets {
            if asset.defaulted {
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
        if let Some(period) = &pool.reinvestment_period {
            if period.end < self.closing_date
                || period.end > self.maturity
                || !period.criteria.max_price_pct.is_finite()
                || period.criteria.max_price_pct <= 0.0
                || !period.criteria.min_yield.is_finite()
            {
                return Err(finstack_quant_core::Error::Validation(
                    "invalid reinvestment end date or price/current-yield criteria".into(),
                ));
            }
            for id in &period.amortizing_tranches {
                match self.tranches.tranches.iter().find(|t| t.id.as_str() == id) {
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
                    || assumptions.price_pct > period.criteria.max_price_pct
                    || assumptions.maturity_months == 0
                    || assumptions
                        .all_in_floor_bp
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
        Ok(())
    }
}
