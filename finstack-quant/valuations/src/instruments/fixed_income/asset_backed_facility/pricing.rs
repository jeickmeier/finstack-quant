//! Facility projection through the structured-credit engine: the synthetic
//! two-class deal, the facility note's flows, the unused-commitment fee and
//! the residual, plus the `Instrument` / `CashflowScheduleSource` impls.

use finstack_quant_cashflows::builder::FloatingRateSpec;
use finstack_quant_core::cashflow::{CFKind, CashFlow, Discountable};
use finstack_quant_core::dates::{Date, DateExt, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::fixings::fixing_series_id;
use finstack_quant_core::money::Money;
use serde::{Deserialize, Serialize};

use super::types::{AmortizationEvent, AssetBackedFacility};
use crate::cashflow::traits::{
    schedule_from_classified_flows, CashflowProvider, ScheduleBuildOpts,
};
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, CallAssumption, CoverageRules, CoverageTestSpec,
    CreditFactors, EarlyAmortizationSpec, LossAllocationPolicy, MarketConditions, Metadata,
    Overrides, ReinvestmentCriteria, ReinvestmentPeriod, SimulationDiagnostics, StructuredCredit,
    Tranche, TrancheCashflows, TrancheCoupon, TrancheSeniority, TrancheStructure, WaterfallRules,
};
use crate::instruments::fixed_income::structured_credit::{TrancheDraw, TrancheReadvance};

/// Identifier of the synthetic facility note.
pub const FACILITY_TRANCHE_ID: &str = "FACILITY";
/// Identifier of the synthetic residual class.
pub const RESIDUAL_TRANCHE_ID: &str = "RESIDUAL";

/// Projected cashflows of a facility and its residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FacilityProjection {
    /// Interest and principal paid to the facility note.
    pub facility: TrancheCashflows,
    /// Cash paid to the residual class.
    pub residual: TrancheCashflows,
    /// Unused-commitment fee accrued on `commitment − opening facility
    /// balance` per accrual period while the line revolves, paid on the
    /// period's payment date.
    pub unused_fees: Vec<(Date, Money)>,
    /// Lender draws applied (scheduled draws and re-advances) per payment
    /// date: cash the lender advances, an outflow in the lender's IRR.
    #[serde(default)]
    pub draws: Vec<(Date, Money)>,
    /// Per-period deal record of the synthetic deal.
    pub diagnostics: SimulationDiagnostics,
}

impl FacilityProjection {
    /// Every cashflow to the lender (interest, principal and unused fees)
    /// in date order, summed per date.
    pub fn lender_cashflows(&self) -> Vec<(Date, Money)> {
        let mut by_date: std::collections::BTreeMap<Date, Money> =
            std::collections::BTreeMap::new();
        let draws = self
            .draws
            .iter()
            .map(|(date, amount)| (*date, amount.checked_neg()));
        for (date, amount) in self
            .facility
            .cashflows
            .iter()
            .chain(&self.unused_fees)
            .map(|(date, amount)| (*date, *amount))
            .chain(draws)
        {
            by_date
                .entry(date)
                .and_modify(|total| {
                    if let Ok(sum) = total.checked_add(amount) {
                        *total = sum;
                    }
                })
                .or_insert(amount);
        }
        by_date.into_iter().collect()
    }
}

impl AssetBackedFacility {
    /// The two-class structured-credit deal the engine runs for this
    /// facility.
    ///
    /// The facility note is a senior class sized at `drawn` paying the
    /// margin over the index (or the fixed all-in rate); the residual is an
    /// equity class sized at the collateral balance less `drawn`
    /// (attachment points derived from the balances). A
    /// `CoverageTestType::BorrowingBase` test at 1.0 with a `PayDownSenior`
    /// action sits after the note's interest so a deficiency repays the
    /// facility ahead of the residual and suspends reinvestment. Collateral
    /// principal is reinvested until the effective revolving end (asset and
    /// rep-line pools; instrument collateral amortizes from closing), loss
    /// and excess-spread amortization events map onto the early-amortization
    /// rules, and a term-out ends in a deal call on [`Self::repayment_date`]
    /// that liquidates the collateral at `liquidation_price_pct` and repays
    /// the facility.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the facility fails [`Self::validate`]
    /// or the synthetic deal fails the deal validation.
    pub fn synthesized_deal(&self) -> finstack_quant_core::Result<StructuredCredit> {
        self.validate()?;
        let currency = self.collateral.get_base_currency();
        let residual = self.collateral.total_balance()?.checked_sub(self.drawn)?;
        let repayment_date = self.repayment_date();

        let coupon = match &self.index_id {
            Some(index_id) => {
                let dec = |value: f64| {
                    rust_decimal::Decimal::try_from(value).map_err(|_| {
                        finstack_quant_core::Error::Validation(format!(
                            "margin_bp {value} cannot be represented as a decimal"
                        ))
                    })
                };
                TrancheCoupon::Floating(FloatingRateSpec {
                    index_id: index_id.clone(),
                    spread_bp: dec(self.margin_bp)?,
                    gearing: dec(1.0)?,
                    gearing_includes_spread: true,
                    index_floor_bp: None,
                    all_in_floor_bp: None,
                    all_in_cap_bp: None,
                    index_cap_bp: None,
                    overnight_index_constraints: Default::default(),
                    reset_frequency: self.frequency,
                    index_tenor: None,
                    reset_lag_days: 0,
                    fixing_calendar_id: None,
                    overnight_compounding: None,
                    overnight_basis: None,
                    fallback: Default::default(),
                })
            }
            None => TrancheCoupon::Fixed {
                rate: self.margin_bp / 10_000.0,
            },
        };
        let mut note = Tranche::from_balance(
            FACILITY_TRANCHE_ID,
            TrancheSeniority::Senior,
            self.drawn,
            coupon,
            self.maturity,
        )?;
        note.frequency = self.frequency;
        note.day_count = self.day_count;
        let mut residual_class = Tranche::from_balance(
            RESIDUAL_TRANCHE_ID,
            TrancheSeniority::Equity,
            residual,
            TrancheCoupon::Fixed { rate: 0.0 },
            self.maturity,
        )?;
        residual_class.frequency = self.frequency;
        residual_class.day_count = self.day_count;
        // Residual first: the stochastic validator wants the first-loss class
        // declared first.
        let tranches = TrancheStructure::new(vec![residual_class, note])?;

        let mut pool = self.collateral.clone();
        pool.reinvestment_period = if pool.instruments.is_some() {
            None
        } else {
            Some(ReinvestmentPeriod {
                end_date: self.effective_revolving_end(),
                is_active: true,
                criteria: ReinvestmentCriteria::default(),
                amortizing_tranches: Vec::new(),
                assumptions: None,
            })
        };

        let max_loss = self
            .amortization_events
            .iter()
            .find_map(|event| match event {
                AmortizationEvent::CumulativeLoss { max_pct } => Some(*max_pct),
                _ => None,
            });
        let min_excess_spread = self
            .amortization_events
            .iter()
            .find_map(|event| match event {
                AmortizationEvent::ExcessSpread { min_3m } => Some(*min_3m),
                _ => None,
            });
        let waterfall_rules =
            (max_loss.is_some() || min_excess_spread.is_some()).then(|| WaterfallRules {
                early_amortization: Some(EarlyAmortizationSpec {
                    // Percent at the facility boundary, fraction in the engine.
                    max_cumulative_loss: max_loss.map(|pct| pct / 100.0),
                    min_excess_spread_3m: min_excess_spread,
                }),
                ..WaterfallRules::default()
            });

        let months = i32::try_from(self.frequency.months().unwrap_or(1)).unwrap_or(1);
        let mut builder = StructuredCredit::builder()
            .id(self.id.clone())
            .deal_type(pool.deal_type)
            .pool(pool)
            .tranches(tranches)
            .closing_date(self.closing_date)
            .first_payment_date(self.closing_date.add_months(months))
            .maturity(self.maturity)
            .frequency(self.frequency)
            .discount_curve_id(self.discount_curve_id.clone())
            .credit_model(self.credit_model.clone())
            .market_conditions(MarketConditions::default())
            .credit_factors(CreditFactors::default())
            .deal_metadata(Metadata::default())
            .behavior_overrides(Overrides::default())
            .hedge_swaps(Vec::new())
            .attributes(self.attributes.clone())
            .coverage_triggers(vec![CoverageTestSpec::borrowing_base(
                FACILITY_TRANCHE_ID,
                1.0,
            )])
            .coverage_rules(CoverageRules {
                borrowing_base: Some(self.borrowing_base_rules.clone()),
                ..CoverageRules::default()
            })
            .loss_allocation(LossAllocationPolicy::ParPreserving)
            .principal_covers_senior_interest(true);
        if let Some(calendar) = &self.payment_calendar_id {
            builder = builder.payment_calendar_id(calendar.clone());
        }
        if let Some(rules) = waterfall_rules {
            builder = builder.waterfall_rules(rules);
        }
        if repayment_date < self.maturity || self.term_out.is_some() {
            // The scheduled term-out end, or the same window from an
            // early-amortization event when that comes first.
            let mut call = CallAssumption::new(repayment_date, 100.0);
            if let Some(term_out) = self.term_out {
                call = call.with_after_early_amortization(term_out.months);
            }
            builder = builder.call_assumption(call);
        }
        if let Some(price) = self.liquidation_price_pct {
            builder = builder.liquidation_price_pct(price);
        }
        if let Some(fees) = &self.fees {
            builder = builder.fees(fees.clone());
        }
        if !self.draw_schedule.is_empty() {
            builder = builder.tranche_draws(
                self.draw_schedule
                    .iter()
                    .map(|draw| TrancheDraw {
                        tranche_id: FACILITY_TRANCHE_ID.to_string(),
                        date: draw.date,
                        amount: draw.amount,
                    })
                    .collect(),
            );
        }
        if self.readvance_to_borrowing_base {
            builder = builder.tranche_readvance(TrancheReadvance {
                tranche_id: FACILITY_TRANCHE_ID.to_string(),
                commitment: self.commitment,
            });
        }
        let _ = currency;
        builder.build()
    }

    /// Project the facility through the engine.
    ///
    /// # Arguments
    ///
    /// * `market` - Discount and index curves plus fixings for the note and
    ///   the collateral.
    /// * `as_of` - Valuation date the projection starts from.
    ///
    /// # Errors
    ///
    /// Returns the deal's validation or simulation errors, or
    /// `Error::Validation` when the engine does not report both synthetic
    /// classes.
    pub fn project(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<FacilityProjection> {
        let deal = self.synthesized_deal()?;
        let mut run = run_simulation_with_diagnostics(&deal, market, as_of)?;
        let facility = run.tranches.remove(FACILITY_TRANCHE_ID).ok_or_else(|| {
            finstack_quant_core::Error::Validation("engine reported no facility class".to_string())
        })?;
        let residual = run.tranches.remove(RESIDUAL_TRANCHE_ID).ok_or_else(|| {
            finstack_quant_core::Error::Validation("engine reported no residual class".to_string())
        })?;
        // The commitment ends with the revolving period: its scheduled end,
        // an earlier scheduled amortization date, or the early-amortization
        // event the engine reports.
        let commitment_end = run
            .diagnostics
            .early_amortization_date
            .map_or(self.effective_revolving_end(), |event| {
                event.min(self.effective_revolving_end())
            });
        let unused_fees = self.unused_fees(&facility, commitment_end)?;
        let draws = run
            .diagnostics
            .tranche_draws
            .iter()
            .filter(|(id, _, _)| id == FACILITY_TRANCHE_ID)
            .map(|(_, date, amount)| (*date, *amount))
            .collect();
        Ok(FacilityProjection {
            facility,
            residual,
            unused_fees,
            draws,
            diagnostics: run.diagnostics,
        })
    }

    /// Unused-commitment fee per accrual period of the note that starts
    /// before the commitment ends (`commitment_end`) with a positive
    /// opening balance: `(commitment − opening balance) × unused_fee_bp ×
    /// accrual`.
    fn unused_fees(
        &self,
        facility: &TrancheCashflows,
        commitment_end: Date,
    ) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        if self.unused_fee_bp <= 0.0 {
            return Ok(Vec::new());
        }
        let currency = self.commitment.currency();
        let mut fees = Vec::with_capacity(facility.accrual_periods.len());
        for period in &facility.accrual_periods {
            if period.start >= commitment_end || period.opening_balance.amount() <= 0.0 {
                continue;
            }
            let undrawn = (self.commitment.amount() - period.opening_balance.amount()).max(0.0);
            if undrawn <= 0.0 {
                continue;
            }
            let accrual = self.day_count.year_fraction(
                period.start,
                period.end,
                DayCountContext::default(),
            )?;
            let fee = undrawn * self.unused_fee_bp / 10_000.0 * accrual;
            if fee > 0.0 {
                fees.push((period.payment_date, Money::new(fee, currency)?));
            }
        }
        Ok(fees)
    }

    /// Internal rate of return of the lender: XIRR of `−drawn` on `as_of`
    /// against every projected interest, principal and fee receipt.
    ///
    /// # Arguments
    ///
    /// * `market` - Curves and fixings for the projection.
    /// * `as_of` - Valuation date the investment is dated on.
    ///
    /// # Errors
    ///
    /// Returns the projection errors, or `Error::Validation` when no rate
    /// solves.
    pub fn facility_irr(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let projection = self.project(market, as_of)?;
        let mut flows: Vec<(Date, f64)> = vec![(as_of, -self.drawn.amount())];
        flows.extend(
            projection
                .lender_cashflows()
                .into_iter()
                .map(|(date, amount)| (date, amount.amount())),
        );
        finstack_quant_core::cashflow::xirr(&flows, None)
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for AssetBackedFacility {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.drawn))
    }

    fn raw_cashflow_schedule(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let projection = self.project(context, as_of)?;
        let mut flows: Vec<CashFlow> = Vec::new();
        for (date, amount) in &projection.facility.interest_flows {
            if amount.amount() > 0.0 {
                flows.push(CashFlow::new(
                    *date,
                    None,
                    *amount,
                    CFKind::Fixed,
                    0.0,
                    None,
                ));
            }
        }
        for (date, amount) in &projection.facility.principal_flows {
            if amount.amount() > 0.0 {
                flows.push(CashFlow::new(
                    *date,
                    None,
                    *amount,
                    CFKind::Amortization,
                    0.0,
                    None,
                ));
            }
        }
        for (date, amount) in &projection.facility.writedown_flows {
            if amount.amount() > 0.0 {
                flows.push(CashFlow::new(
                    *date,
                    None,
                    amount.checked_neg(),
                    CFKind::DefaultedNotional,
                    0.0,
                    None,
                ));
            }
        }
        for (date, amount) in &projection.unused_fees {
            flows.push(CashFlow::new(*date, None, *amount, CFKind::Fee, 0.0, None));
        }
        Ok(schedule_from_classified_flows(
            flows,
            self.day_count,
            ScheduleBuildOpts {
                notional_hint: Some(self.drawn),
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        ))
    }
}

impl Instrument for AssetBackedFacility {
    impl_instrument_base!(crate::pricer::InstrumentType::AssetBackedFacility);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.synthesized_deal()?.validate_for_pricing()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(index_id) = &self.index_id {
            deps.add_forward_curve(index_id.clone());
            deps.add_series_id(fixing_series_id(index_id.as_str()));
        }
        let pool = self.collateral.normalized(self.closing_date)?;
        for index_id in pool
            .assets
            .iter()
            .filter_map(|asset| asset.index_id.as_deref())
        {
            deps.add_forward_curve(index_id);
            deps.add_series_id(fixing_series_id(index_id));
        }
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let flows = self.dated_cashflows(market, as_of)?;
        flows.npv(disc.as_ref(), as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        Some(self.closing_date)
    }

    crate::impl_focused_pricing_overrides!();
}
