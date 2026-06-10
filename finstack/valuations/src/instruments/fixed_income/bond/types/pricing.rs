//! Bond pricing methods, validation, and cashflow projection.

use crate::instruments::common_impl::validation;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::Result;
use rust_decimal::prelude::ToPrimitive;

use super::definitions::Bond;
use super::CashflowSpec;

impl Bond {
    /// Pricing-oriented dated cashflows: coupons, amortization, and positive
    /// notional (redemption). Negative notionals (initial draw) and pure PIK
    /// accretion are excluded because they are not discounted receipt flows.
    ///
    /// When the bond has an ex-coupon convention and `as_of` falls inside the
    /// ex-coupon window of a coupon, that coupon is excluded: a buyer settling
    /// in the ex-window does not receive the imminent coupon (market standard,
    /// e.g. UK gilts). Accrued interest is correspondingly negative in that
    /// window (see [`crate::cashflow::accrued_interest_amount`]).
    ///
    /// Internal pricing engines (discount, hazard, spread solvers) should use
    /// this instead of the public [`CashflowProvider::dated_cashflows`] which
    /// now returns the full signed canonical schedule.
    pub(crate) fn pricing_dated_cashflows(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<Vec<(finstack_core::dates::Date, finstack_core::money::Money)>> {
        use finstack_core::cashflow::CFKind;

        let ex_coupon = self.accrual_config().ex_coupon;
        let schedule = self.full_cashflow_schedule(curves)?;
        let mut flows = Vec::with_capacity(schedule.flows.len());
        for cf in schedule.flows {
            let keep = cf.date >= as_of
                && cf.kind != CFKind::PIK
                && !(cf.kind == CFKind::Notional && cf.amount.amount() < 0.0);
            if !keep {
                continue;
            }
            // Drop interest flows whose ex-date has passed: the buyer as of
            // `as_of` is not entitled to them.
            if cf.kind.is_interest_like() {
                if let Some(rule) = &ex_coupon {
                    let ex_date = rule.ex_date(cf.date)?;
                    if as_of >= ex_date && as_of < cf.date {
                        continue;
                    }
                }
            }
            flows.push((cf.date, cf.amount));
        }
        Ok(flows)
    }

    /// Cashflow schedule enriched with discount factors, survival probabilities, and PVs.
    ///
    /// Builds the bond's full internal cashflow schedule
    /// and computes per-cashflow discount factors and (when a credit curve is configured)
    /// survival probabilities, returning a
    /// [`crate::cashflow::builder::PeriodDataFrame`] that is ready for tabular
    /// export or further analysis.
    ///
    /// # Arguments
    /// * `market` - Market context containing discount and optional hazard curves
    /// * `as_of` - Valuation date; defaults to the discount curve's base date when `None`
    ///
    /// # Returns
    /// A [`crate::cashflow::builder::PeriodDataFrame`] with `discount_factors`,
    /// optional `survival_probs`, and `pvs`.
    pub fn pricing_cashflows(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: Option<Date>,
    ) -> Result<crate::cashflow::builder::PeriodDataFrame> {
        use crate::cashflow::builder::PeriodDataFrameOptions;
        use finstack_core::dates::{Period, PeriodId};

        let schedule = self.full_cashflow_schedule(market)?;

        let periods: Vec<Period> =
            if let (Some(first), Some(last)) = (schedule.flows.first(), schedule.flows.last()) {
                vec![Period {
                    id: PeriodId::annual(first.date.year()),
                    start: first.date,
                    end: last.date,
                    is_actual: true,
                }]
            } else {
                Vec::new()
            };

        let options = PeriodDataFrameOptions {
            credit_curve_id: self.credit_curve_id.as_ref().map(|id| id.as_str()),
            as_of,
            ..Default::default()
        };

        schedule.to_period_dataframe(&periods, market, self.discount_curve_id.as_str(), options)
    }

    /// Price bond using tree-based pricing for embedded options (calls/puts).
    ///
    /// This method is automatically called by `value()` when the bond has a non-empty
    /// call/put schedule. It uses a short-rate tree model to properly value the
    /// embedded optionality via backward induction.
    ///
    /// # Arguments
    /// * `market` - Market context with discount curve (and optionally hazard curve)
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    /// Option-adjusted present value of the bond
    pub(crate) fn value_with_tree(
        &self,
        market: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        use crate::instruments::fixed_income::bond::pricing::engine::tree::{
            bond_tree_config, TreePricer,
        };

        // Use the same dispatch as OAS/quote conversions so direct callable PV
        // honors BDT, Hull-White, hazard-tree, and tree-curve overrides.
        let config = bond_tree_config(self);
        let price_amount =
            TreePricer::with_config(config).price_at_oas(self, market, as_of, 0.0)?;

        Ok(Money::new(price_amount, self.notional.currency()))
    }

    /// Validate all bond parameters.
    ///
    /// Performs comprehensive validation of the bond instrument:
    /// - Issue date must be before maturity date
    /// - Notional must be positive
    /// - Coupon rate must be non-negative (for fixed-rate bonds)
    /// - Call/put prices must be positive
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` with a descriptive message if any validation fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let bond = Bond::fixed(...)?;
    /// bond.validate()?; // Validates all parameters
    /// ```
    pub fn validate(&self) -> Result<()> {
        // Validate date ordering
        validation::validate_date_range_strict_with(
            self.issue_date,
            self.maturity,
            |start, end| {
                format!(
                    "Bond issue date ({}) must be before maturity date ({})",
                    start, end
                )
            },
        )?;

        // Validate notional is finite and positive
        validation::validate_money_finite(self.notional, "bond notional")?;
        validation::validate_money_gt_with(self.notional, 0.0, |amount| {
            format!("Bond notional must be positive, got {}", amount)
        })?;

        // Validate coupon rate for fixed-rate bonds (including amortizing with fixed base)
        Self::validate_coupon_rate(&self.cashflow_spec)?;

        // Validate call/put prices and exercise date ranges
        if let Some(ref call_put) = self.call_put {
            for call in &call_put.calls {
                if call.price_pct_of_par <= 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "Bond call price must be positive, got {} for period [{}, {}]",
                        call.price_pct_of_par, call.start_date, call.end_date
                    )));
                }
                if call.start_date < self.issue_date || call.start_date > self.maturity {
                    return Err(finstack_core::Error::Validation(format!(
                        "Call exercise start date {} is outside bond life [{}, {}]",
                        call.start_date, self.issue_date, self.maturity
                    )));
                }
                if call.end_date > self.maturity {
                    return Err(finstack_core::Error::Validation(format!(
                        "Call exercise end date {} is after maturity {}",
                        call.end_date, self.maturity
                    )));
                }
                if call.start_date > call.end_date {
                    return Err(finstack_core::Error::Validation(format!(
                        "Call exercise start date {} is after end date {}",
                        call.start_date, call.end_date
                    )));
                }
            }
            for put in &call_put.puts {
                if put.price_pct_of_par <= 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "Bond put price must be positive, got {} for period [{}, {}]",
                        put.price_pct_of_par, put.start_date, put.end_date
                    )));
                }
                if put.start_date < self.issue_date || put.start_date > self.maturity {
                    return Err(finstack_core::Error::Validation(format!(
                        "Put exercise start date {} is outside bond life [{}, {}]",
                        put.start_date, self.issue_date, self.maturity
                    )));
                }
                if put.end_date > self.maturity {
                    return Err(finstack_core::Error::Validation(format!(
                        "Put exercise end date {} is after maturity {}",
                        put.end_date, self.maturity
                    )));
                }
                if put.start_date > put.end_date {
                    return Err(finstack_core::Error::Validation(format!(
                        "Put exercise start date {} is after end date {}",
                        put.start_date, put.end_date
                    )));
                }
            }
        }

        Ok(())
    }

    /// Returns `true` when coupon cashflows depend on forward curve projection (floating FRNs).
    ///
    /// True for [`CashflowSpec::Floating`] and for [`CashflowSpec::Amortizing`] when the
    /// base specification is floating.
    pub fn has_floating_coupons(&self) -> bool {
        match &self.cashflow_spec {
            CashflowSpec::Floating(_) => true,
            CashflowSpec::Amortizing { base, .. } => {
                matches!(base.as_ref(), CashflowSpec::Floating(_))
            }
            _ => false,
        }
    }

    /// Recursively validate that fixed coupon rates are non-negative.
    ///
    /// Handles `Fixed`, `Floating` (no coupon rate to validate), and
    /// `Amortizing` (recurses into the base spec).
    fn validate_coupon_rate(spec: &CashflowSpec) -> Result<()> {
        match spec {
            CashflowSpec::Fixed(s) => {
                let rate = s.rate.to_f64().unwrap_or(0.0);
                if rate < 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "Bond fixed coupon rate must be non-negative, got {}",
                        rate
                    )));
                }
            }
            CashflowSpec::StepUp(s) => {
                let rate = s.initial_rate.to_f64().unwrap_or(0.0);
                if rate < 0.0 {
                    return Err(finstack_core::Error::Validation(format!(
                        "Bond step-up initial coupon rate must be non-negative, got {}",
                        rate
                    )));
                }
                for (_, step_rate) in &s.step_schedule {
                    let r = step_rate.to_f64().unwrap_or(0.0);
                    if r < 0.0 {
                        return Err(finstack_core::Error::Validation(format!(
                            "Bond step-up coupon rate must be non-negative, got {}",
                            r
                        )));
                    }
                }
            }
            CashflowSpec::Amortizing { base, .. } => {
                Self::validate_coupon_rate(base)?;
            }
            CashflowSpec::Floating(_) => {
                // No fixed coupon rate to validate
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::context::MarketContext;
    use time::macros::date;

    fn ex_coupon_bond() -> Bond {
        let mut bond = Bond::fixed(
            "EX-FLOWS",
            Money::new(100.0, Currency::USD),
            0.05,
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("valid bond");
        bond.settlement_convention = Some(super::super::BondSettlementConvention {
            ex_coupon_days: 7,
            ..Default::default()
        });
        bond
    }

    #[test]
    fn pricing_flows_exclude_coupon_inside_ex_window() {
        let bond = ex_coupon_bond();
        let coupon_date = date!(2025 - 07 - 01);
        let market = MarketContext::new();

        // Inside the ex-window (5 days before the coupon): the imminent coupon
        // is not a buyer flow.
        let as_of = coupon_date - time::Duration::days(5);
        let flows = bond.pricing_dated_cashflows(&market, as_of).expect("flows");
        assert!(
            !flows.iter().any(|(d, _)| *d == coupon_date),
            "ex-period coupon must be excluded from buyer flows"
        );

        // Outside the ex-window (8 days before): the coupon is still a buyer flow.
        let as_of = coupon_date - time::Duration::days(8);
        let flows = bond.pricing_dated_cashflows(&market, as_of).expect("flows");
        assert!(
            flows.iter().any(|(d, _)| *d == coupon_date),
            "cum-coupon flows must include the next coupon"
        );
    }
}
