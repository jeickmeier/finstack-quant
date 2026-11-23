//! Domain accessors, `Instrument`, and `CashflowScheduleSource` implementations for [`RevolvingCredit`].

use super::{DrawRepayEvent, DrawRepaySpec, RevolvingCredit};
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::impl_instrument_base;
use rust_decimal::prelude::ToPrimitive;

impl RevolvingCredit {
    /// Settlement date of a quote on `as_of`: `settlement_days` business days
    /// forward on `calendar_id`, or weekdays when no calendar is set.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date the settlement lag is counted from.
    ///
    /// # Errors
    ///
    /// Returns an error when the settlement-day count exceeds the signed offset
    /// range, the shifted date exceeds the calendar, `calendar_id` names an
    /// unknown calendar, or the business-day roll fails.
    pub fn settlement_date(&self, as_of: Date) -> finstack_quant_core::Result<Date> {
        crate::loan_terms::settlement_date(as_of, self.settlement_days, self.calendar_id.as_deref())
    }

    /// Drawn balance on `date` of a deterministic draw/repay schedule.
    ///
    /// Starts from `drawn`, the balance at `anchor`, and replays in
    /// order the `events` dated strictly after `anchor` and on or before
    /// `date`. Each event is checked against the commitment in force on its
    /// date and the running balance.
    ///
    /// # Arguments
    ///
    /// * `events` - Draw/repay events sorted by date ascending.
    /// * `anchor` - Date `drawn` refers to; events on or before it are
    ///   not replayed.
    /// * `date` - Date the balance is wanted for; events dated on it apply.
    ///
    /// # Errors
    ///
    /// Returns a validation error when a draw would exceed the commitment in
    /// force or a repayment exceeds the running balance.
    pub(crate) fn drawn_balance_at<'a>(
        &self,
        events: impl IntoIterator<Item = &'a DrawRepayEvent>,
        anchor: Date,
        date: Date,
    ) -> finstack_quant_core::Result<Money> {
        let mut balance = self.drawn;
        for event in events {
            if event.date > date {
                break;
            }
            if event.date > anchor {
                balance = super::super::schedule_projection::apply_draw_repay_event(
                    balance,
                    event,
                    self.commitment_at(event.date),
                )?;
            }
        }
        Ok(balance)
    }

    /// Commitment in force on `date`: the last `commitment_steps` entry
    /// dated on or before it, else the opening `commitment`.
    ///
    /// # Arguments
    ///
    /// * `date` - Date the commitment is wanted for.
    pub fn commitment_at(&self, date: Date) -> Money {
        crate::loan_terms::commitment_in_force(&self.commitment_steps, self.commitment, date)
    }

    /// Drawn balance at the simulation anchor over the commitment in force on
    /// `date`, as a decimal in `[0, 1]`.
    ///
    /// # Arguments
    ///
    /// * `date` - Date the commitment is read on, normally the simulation
    ///   anchor.
    pub fn utilization_at(&self, date: Date) -> f64 {
        let commitment = self.commitment_at(date).amount();
        if commitment <= 0.0 {
            0.0
        } else {
            (self.drawn.amount() / commitment).clamp(0.0, 1.0)
        }
    }

    /// Cumulative margin change in force on `date`, in basis points: the sum
    /// of every `margin_steps` entry dated on or before it.
    ///
    /// # Arguments
    ///
    /// * `date` - Accrual date the margin is wanted for.
    pub fn margin_delta_bp_at(&self, date: Date) -> f64 {
        self.margin_steps
            .iter()
            .filter(|step| step.date <= date)
            .map(|step| step.delta_bp.to_f64().unwrap_or_default())
            .sum()
    }

    /// Every date on which a commitment, margin or fee step takes effect,
    /// sorted and deduplicated. Both cashflow engines slice accrual on them.
    pub fn step_dates(&self) -> Vec<Date> {
        let mut dates: Vec<Date> = self
            .commitment_steps
            .iter()
            .map(|step| step.date)
            .chain(self.margin_steps.iter().map(|step| step.date))
            .chain(self.fees.steps.iter().map(|step| step.date))
            .chain(
                self.lc
                    .iter()
                    .flat_map(|lc| lc.events.iter().map(|event| event.date)),
            )
            .collect();
        dates.sort_unstable();
        dates.dedup();
        dates
    }

    /// Letter-of-credit face outstanding on `date`: the anchor `outstanding`
    /// plus every issuance, less every expiry, dated on or before it. Zero
    /// without an LC sub-facility.
    ///
    /// # Arguments
    ///
    /// * `date` - Date the LC outstanding is wanted for.
    pub fn lc_outstanding_at(&self, date: Date) -> Money {
        let ccy = self.commitment.currency();
        let Some(lc) = &self.lc else {
            return Money::from((0_i64, ccy));
        };
        let amount = lc
            .events
            .iter()
            .filter(|event| event.date <= date)
            .fold(lc.outstanding.amount(), |acc, event| {
                if event.is_issue {
                    acc + event.amount.amount()
                } else {
                    acc - event.amount.amount()
                }
            })
            .max(0.0);
        Money::new(amount, ccy).unwrap_or(Money::from((0_i64, ccy)))
    }

    /// Letter-of-credit fee in force on `date`, in basis points per annum:
    /// `lc.fee_bp` when set, else the floating spread plus the cumulative
    /// margin step delta. Zero without an LC sub-facility.
    ///
    /// # Arguments
    ///
    /// * `date` - Accrual date the fee is evaluated on.
    pub fn lc_fee_bp_at(&self, date: Date) -> f64 {
        let Some(lc) = &self.lc else { return 0.0 };
        match (lc.fee_bp, &self.rate) {
            (Some(fee_bp), _) => fee_bp,
            (None, RateSpec::Floating(spec)) => {
                spec.spread_bp.to_f64().unwrap_or(0.0) + self.margin_delta_bp_at(date)
            }
            (None, RateSpec::Fixed { .. }) => 0.0,
        }
        .max(0.0)
    }

    /// Model named by `attributes.meta["pricing_model"]`, when set.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the value is not a `ModelKey` string,
    /// instead of silently pricing with the default model.
    pub(crate) fn pricing_model_override(
        &self,
    ) -> finstack_quant_core::Result<Option<finstack_quant_valuations_core::pricer::ModelKey>> {
        self.attributes
            .get_meta("pricing_model")
            .map(|model| {
                <finstack_quant_valuations_core::pricer::ModelKey as ::std::str::FromStr>::from_str(
                    model,
                )
                .map_err(|_| {
                    finstack_quant_core::Error::Validation(format!(
                        "RevolvingCredit '{}': unknown pricing_model '{model}'",
                        self.id
                    ))
                })
            })
            .transpose()
    }

    /// Fronting fee in force, in basis points per annum (zero without an LC
    /// sub-facility).
    pub fn fronting_fee_bp(&self) -> f64 {
        self.lc.as_ref().map_or(0.0, |lc| lc.fronting_fee_bp)
    }

    /// Letter-of-credit face assumed drawn at default on `date`:
    /// `lc.leq × LC outstanding`, in the facility currency.
    ///
    /// # Arguments
    ///
    /// * `date` - Date the contingent exposure is wanted for.
    pub fn lc_exposure_at_default(&self, date: Date) -> f64 {
        self.lc
            .as_ref()
            .map_or(0.0, |lc| lc.leq * self.lc_outstanding_at(date).amount())
    }

    /// Upfront fee amount in the facility currency (zero when none), resolved
    /// against the opening commitment.
    ///
    /// # Errors
    ///
    /// Returns an error when a fractional fee is non-finite or the fee is
    /// outside the Decimal range.
    pub fn upfront_fee_amount(&self) -> finstack_quant_core::Result<Money> {
        self.fees.upfront_fee.as_ref().map_or_else(
            || Ok(Money::from((0_i64, self.commitment.currency()))),
            |fee| fee.amount(self.commitment),
        )
    }

    /// Scheduled fixed fees dated after `as_of`, as `(date, amount)`.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date; fees dated on or before it are history.
    pub fn scheduled_fees_after(&self, as_of: Date) -> Vec<(Date, Money)> {
        self.scheduled_fees
            .iter()
            .filter(|fee| fee.date > as_of && fee.amount.amount() > 0.0)
            .map(|fee| (fee.date, fee.amount))
            .collect()
    }

    /// Reduction fees payable on commitment step-downs dated after `as_of`:
    /// `(previous commitment − new commitment) × reduction_fee_bp`, one entry per step
    /// that lowers the commitment and carries a positive fee.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date; steps dated on or before it are history.
    ///
    /// # Errors
    ///
    /// Returns an error when a fee amount is not representable as `Money`.
    pub fn commitment_step_fees(
        &self,
        as_of: Date,
    ) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        let mut previous = self.commitment;
        let mut fees = Vec::new();
        for step in &self.commitment_steps {
            let reduction = previous.amount() - step.amount.amount();
            let fee_bp = step.reduction_fee_bp.to_f64().unwrap_or_default();
            if step.date > as_of && reduction > 0.0 && fee_bp > 0.0 {
                fees.push((
                    step.date,
                    Money::new(reduction * fee_bp * 1e-4, previous.currency())?,
                ));
            }
            previous = step.amount;
        }
        Ok(fees)
    }

    /// Check if the facility uses deterministic cashflows.
    pub fn is_deterministic(&self) -> bool {
        matches!(self.draw_repay_spec, DrawRepaySpec::Deterministic(_))
    }

    /// Check if the facility uses stochastic utilization.
    pub fn is_stochastic(&self) -> bool {
        matches!(self.draw_repay_spec, DrawRepaySpec::Stochastic(_))
    }
}

// Implement the Instrument trait
impl finstack_quant_valuations_core::instruments::Instrument for RevolvingCredit {
    fn includes_valuation_date_cashflows(&self) -> bool {
        false
    }
    fn cashflow_export_terms(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<
        finstack_quant_valuations_core::instruments::cashflow_export::CashflowExportTerms,
    > {
        use finstack_quant_cashflows::CashflowProvider;
        finstack_quant_valuations_core::instruments::cashflow_export::CashflowExportTerms::from_schedule(
                self.cashflow_schedule(market, as_of)?,
            )
            .with_schedule_balances()
    }

    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::RevolvingCredit,
        crate::FIXED_INCOME
    );

    fn contractual_accrual_config(
        &self,
    ) -> Option<finstack_quant_cashflows::accrual::AccrualConfig> {
        Some(finstack_quant_cashflows::accrual::AccrualConfig {
            method: finstack_quant_cashflows::accrual::AccrualMethod::Linear,
            ex_coupon: None,
            include_pik: false,
            frequency: Some(self.frequency),
        })
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(credit_curve_id) = &self.credit_curve_id {
            deps.add_credit_curve(credit_curve_id.clone());
        }
        if let RateSpec::Floating(spec) = &self.rate {
            deps.add_forward_curve(spec.forward_curve_id.clone());
            deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
                spec.index_id.as_str(),
            ));
        }
        Ok(deps)
    }

    fn default_model(&self) -> finstack_quant_valuations_core::pricer::ModelKey {
        // `validate` rejects an unknown `pricing_model`, so only a valid
        // override or none reaches here.
        self.pricing_model_override()
            .ok()
            .flatten()
            .unwrap_or(finstack_quant_valuations_core::pricer::ModelKey::Discounting)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        // Optional model override via attributes metadata (e.g., meta["pricing_model"] = "monte_carlo_three_factor")
        if let Some(model) = self.pricing_model_override()? {
            let registry =
                finstack_quant_valuations_core::instruments::Instrument::family(self).pricers();
            let result = registry
                .price_with_metrics(self, model, curves, as_of, &[], Default::default())
                .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
            return Ok(result.value);
        }

        // Use the same automatic deterministic/Monte Carlo dispatch as the
        // registry and host-language JSON entry points. A stochastic facility
        // must have one canonical value regardless of which public lifecycle
        // invokes it.
        crate::revolving_credit::RevolvingCreditPricer::price(self, curves, as_of)
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

// Implement CashflowProvider for standard cashflow interface
impl finstack_quant_cashflows::CashflowScheduleSource for RevolvingCredit {
    fn notional(&self) -> finstack_quant_core::Result<Option<finstack_quant_core::money::Money>> {
        Ok(Some(self.commitment))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        // Stochastic facilities expose their Monte Carlo expected schedule
        // (path-averaged flow by flow) so theta carry and the exporters work.
        if self.is_stochastic() {
            return crate::revolving_credit::RevolvingCreditPricer::expected_cashflows(
                self, curves, as_of,
            )
            .map(|schedule| {
                schedule.with_representation(
                    finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
                )
            });
        }

        self.cashflow_schedule_with_postings(curves, as_of, &[])
    }
}

impl RevolvingCredit {
    /// Recompile the deterministic contractual schedule with principal
    /// postings applied.
    ///
    /// The schedule is rebuilt from the facility's own terms: interest
    /// accrues on the dated drawn balance, commitment and usage fees follow
    /// the changed drawn and undrawn amounts, and the terminal repayment is
    /// what remains drawn. The commitment is untouched, so a repaid amount
    /// can be redrawn under the facility's own draw events. With no postings
    /// this is the facility's ordinary deterministic schedule.
    ///
    /// # Arguments
    ///
    /// * `market` - Market the facility's floating coupons and fixings are
    ///   read from; a fixed-rate facility needs none of it.
    /// * `as_of` - Valuation date. `drawn` is the balance at the later of
    ///   this date and the commitment date, and cashflows paid on or before
    ///   it are excluded.
    /// * `postings` - Principal changes already settled outside the
    ///   schedule, in the facility currency. Each takes effect at the start
    ///   of its `effective_date`, which must be after the later of `as_of`
    ///   and the commitment date, and appears as a zero-cash `PrePayment`
    ///   row carrying the principal change.
    ///
    /// # Errors
    ///
    /// Returns a validation error for a stochastic facility, a posting in
    /// another currency, a posting effective on or before the balance
    /// anchor, a repayment above the drawn balance or a capitalization above
    /// the commitment in force, and propagates schedule errors.
    pub fn cashflow_schedule_with_postings(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        postings: &[crate::loan_terms::PrincipalPosting],
    ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        use crate::revolving_credit::cashflow_engine::CashflowEngine;
        let path_schedule = CashflowEngine::deterministic(self, market, as_of, postings)?;
        Ok(path_schedule.schedule.with_representation(
            finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
        ))
    }
}
