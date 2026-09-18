//! The `AssetBackedFacility` instrument: a committed warehouse / forward-flow
//! facility lending against a collateral pool under advance rates,
//! concentration limits and a borrowing-base test.

use finstack_quant_core::dates::{Date, DateExt, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use serde::{Deserialize, Serialize};

use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::fixed_income::structured_credit::{
    AssetPool, BorrowingBaseReport, BorrowingBaseRules, CreditModelConfig, DealFees,
};

/// A scheduled draw on the facility: the lender advances `amount` on the
/// first payment date at or after `date`, lifting the facility balance and
/// funding collateral purchases (or repaying, once the line has turned out).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FacilityDraw {
    /// Earliest draw date; applied on the first payment date at or after it.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Amount advanced, in the facility currency.
    pub amount: Money,
}

/// Event that ends the revolving period early and turns the facility out
/// (principal collections then repay the lender sequentially).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AmortizationEvent {
    /// A scheduled amortization date earlier than `revolving_end`.
    Date {
        /// Date revolving stops.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        date: Date,
    },
    /// Cumulative collateral losses above `max_pct` percent of the original
    /// collateral end revolving.
    CumulativeLoss {
        /// Loss threshold in percent of original collateral (`4.0` = 4%).
        max_pct: f64,
    },
    /// A three-period average excess spread below `min_3m` (annual decimal)
    /// ends revolving.
    ExcessSpread {
        /// Excess-spread floor as an annual decimal (`0.01` = 1%).
        min_3m: f64,
    },
}

/// Term-out after revolving: collateral principal repays the facility
/// sequentially for `months` after the (possibly accelerated) revolving end,
/// and whatever collateral is left is then liquidated at the facility's
/// `liquidation_price_pct` to repay the balance (the residual takes the
/// rest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TermOutSpec {
    /// Months from the revolving end to the facility's final repayment date.
    pub months: u32,
}

/// Committed asset-backed facility (warehouse line) against a collateral pool.
///
/// The facility lends `drawn` of a `commitment` against `collateral`; the
/// borrowing base is the advance-rate-weighted eligible collateral after the
/// concentration limits. Each period the structured-credit engine runs a
/// synthetic two-class deal (the facility note and the residual): a
/// borrowing-base deficiency is a mandatory repayment ahead of the residual,
/// collateral principal recycles into new collateral while revolving and
/// repays the facility sequentially afterwards, the unused commitment
/// accrues `unused_fee_bp`, and the residual keeps what is left. See
/// [`Self::synthesized_deal`] for the exact mapping.
///
/// Rates are decimals (`rate`) or basis points (`*_bp`); percentages are
/// percent values (`max_pct`).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::AssetBackedFacility;
///
/// let facility = AssetBackedFacility::example()?;
/// let report = facility.borrowing_base()?;
/// assert!(report.borrowing_base.amount() >= facility.drawn.amount());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(
    Clone, Debug, finstack_quant_valuations_macros::FinancialBuilder, Serialize, Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AssetBackedFacility {
    /// Stable facility identifier.
    pub id: InstrumentId,
    /// Collateral pool the facility lends against (asset rows, rep lines or
    /// instrument collateral).
    pub collateral: AssetPool,
    /// Advance rates, eligibility and concentration limits that define the
    /// borrowing base.
    pub borrowing_base_rules: BorrowingBaseRules,
    /// Total commitment in the collateral currency.
    pub commitment: Money,
    /// Amount drawn at closing, in the collateral currency; at most the
    /// commitment and strictly below the collateral balance.
    pub drawn: Money,
    /// Forward-curve identifier of the floating index; `None` makes
    /// `margin_bp` the all-in fixed rate.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_id: Option<CurveId>,
    /// Margin over the index (or the all-in fixed rate) in basis points.
    pub margin_bp: f64,
    /// Fee on the undrawn commitment in basis points per annum.
    #[builder(default)]
    #[serde(default)]
    pub unused_fee_bp: f64,
    /// Closing date; the first payment date is one `frequency` later.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub closing_date: Date,
    /// Scheduled end of the revolving period.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub revolving_end: Date,
    /// Legal final maturity of the facility.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Payment frequency of interest, fees and the borrowing-base test.
    pub frequency: Tenor,
    /// Accrual convention of the facility interest and unused fee.
    #[builder(default = DayCount::Act360)]
    #[serde(default = "default_day_count")]
    pub day_count: DayCount,
    /// Holiday calendar for the payment schedule (e.g. `"nyse"`); required
    /// for pricing.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_calendar_id: Option<String>,
    /// Events that end revolving early.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub amortization_events: Vec<AmortizationEvent>,
    /// Term-out window after revolving; `None` lets the collateral run to
    /// `maturity`.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term_out: Option<TermOutSpec>,
    /// Transaction fees paid through the synthetic deal's waterfall ahead of
    /// the facility's interest (trustee, servicing, ...); `None` for none.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fees: Option<DealFees>,
    /// Scheduled draws after closing, ascending by date.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub draw_schedule: Vec<FacilityDraw>,
    /// Re-advance the facility each revolving period up to the commitment
    /// and the borrowing base.
    #[builder(default)]
    #[serde(default)]
    pub readvance_to_borrowing_base: bool,
    /// Price the collateral realizes when the term-out ends and the
    /// remaining collateral is liquidated to repay the facility, as a percent
    /// of par (`None` = par).
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub liquidation_price_pct: Option<f64>,
    /// Discount curve for the facility and residual cashflows.
    pub discount_curve_id: CurveId,
    /// Collateral behavior (prepayment, default, recovery, delinquency,
    /// card) applied by the engine; flattened on the wire like the deal's.
    #[builder(default)]
    #[serde(default, flatten)]
    pub credit_model: CreditModelConfig,
    /// Free-form attributes (tags and metadata).
    #[builder(default)]
    #[serde(default)]
    pub attributes: Attributes,
    /// Instrument-level pricing overrides.
    #[builder(default)]
    #[serde(default)]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-level pricing overrides.
    #[builder(default)]
    #[serde(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-level pricing overrides.
    #[builder(default)]
    #[serde(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
}

fn default_day_count() -> DayCount {
    DayCount::Act360
}

impl AssetBackedFacility {
    /// Canonical example: the USD 100M example CLO pool financed by a
    /// USD 80M commitment drawn USD 70M at a fixed 6% (600 bp),
    /// 80% advance rate on first-lien loans, 20% obligor limit, two-year
    /// revolving period and a 24-month term-out.
    ///
    /// # Errors
    ///
    /// Returns the builder's validation error; the example terms are fixed,
    /// so this does not occur in practice.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use crate::instruments::fixed_income::structured_credit::{
            AdvanceRate, ConcentrationLimit, ConcentrationScope, EligibilityRule, StructuredCredit,
        };
        use finstack_quant_core::currency::Currency;
        use time::macros::date;

        let deal = StructuredCredit::example();
        let closing = date!(2024 - 01 - 15);
        let mut facility = Self::builder()
            .id(InstrumentId::new("ABF-EXAMPLE".to_string()))
            .collateral(deal.pool)
            .borrowing_base_rules(BorrowingBaseRules {
                advance_rates: vec![AdvanceRate {
                    asset_class: "*".to_string(),
                    rate: 0.8,
                    eligibility: EligibilityRule::default(),
                }],
                concentration_limits: vec![ConcentrationLimit {
                    scope: ConcentrationScope::Obligor,
                    max_pct: 20.0,
                }],
            })
            .commitment(Money::from((80_000_000_i64, Currency::USD)))
            .drawn(Money::from((70_000_000_i64, Currency::USD)))
            .margin_bp(600.0)
            .unused_fee_bp(50.0)
            .closing_date(closing)
            .revolving_end(closing.add_months(24))
            .maturity(closing.add_months(72))
            .frequency(Tenor::quarterly())
            .payment_calendar_id("nyse".to_string())
            .term_out(TermOutSpec { months: 24 })
            .discount_curve_id(CurveId::new("USD-OIS".to_string()))
            .credit_model(deal.credit_model)
            .build()?;
        facility.collateral.reinvestment_period = None;
        Ok(facility)
    }

    /// Borrowing base on the closing collateral balances.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the rules are malformed.
    pub fn borrowing_base(&self) -> finstack_quant_core::Result<BorrowingBaseReport> {
        self.borrowing_base_rules.validate()?;
        self.borrowing_base_rules.evaluate(&self.collateral, None)
    }

    /// Undrawn commitment at closing.
    pub fn undrawn(&self) -> finstack_quant_core::Result<Money> {
        self.commitment.checked_sub(self.drawn)
    }

    /// Effective end of revolving: the scheduled `revolving_end` or the
    /// earliest scheduled [`AmortizationEvent::Date`], whichever is first.
    pub fn effective_revolving_end(&self) -> Date {
        self.amortization_events
            .iter()
            .filter_map(|event| match event {
                AmortizationEvent::Date { date } => Some(*date),
                _ => None,
            })
            .fold(self.revolving_end, Date::min)
    }

    /// Final repayment date: `maturity`, or the effective revolving end plus
    /// the term-out window when that is earlier (the collateral is then
    /// liquidated on the first payment date at or after it).
    pub fn repayment_date(&self) -> Date {
        match self.term_out {
            Some(term_out) => self
                .effective_revolving_end()
                .add_months(i32::try_from(term_out.months).unwrap_or(i32::MAX))
                .min(self.maturity),
            None => self.maturity,
        }
    }

    /// Validate amounts, rates, dates and the borrowing-base rules.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` describing the first violated invariant:
    /// commitment/drawn ordering and currency, drawn not below the collateral
    /// balance, non-finite or negative rates, unordered dates, a zero
    /// frequency or term-out, or malformed rules.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        let currency = self.collateral.get_base_currency();
        if self.commitment.currency() != currency || self.drawn.currency() != currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: currency,
                actual: if self.commitment.currency() == currency {
                    self.drawn.currency()
                } else {
                    self.commitment.currency()
                },
            });
        }
        if self.drawn.amount() <= 0.0 || self.drawn.amount() > self.commitment.amount() {
            return Err(invalid(format!(
                "drawn ({}) must be positive and at most the commitment ({})",
                self.drawn.amount(),
                self.commitment.amount()
            )));
        }
        let collateral = self.collateral.total_balance()?.amount();
        if self.drawn.amount() >= collateral {
            return Err(invalid(format!(
                "drawn ({}) must be below the collateral balance ({}) so a residual class exists",
                self.drawn.amount(),
                collateral
            )));
        }
        for (label, value) in [
            ("margin_bp", self.margin_bp),
            ("unused_fee_bp", self.unused_fee_bp),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(invalid(format!(
                    "{label} ({value}) must be finite and non-negative"
                )));
            }
        }
        if self.closing_date >= self.revolving_end || self.revolving_end > self.maturity {
            return Err(invalid(format!(
                "dates must satisfy closing {} < revolving_end {} <= maturity {}",
                self.closing_date, self.revolving_end, self.maturity
            )));
        }
        if self.frequency.months().unwrap_or(0) == 0 {
            return Err(invalid(
                "frequency must be a whole number of months".to_string(),
            ));
        }
        if self.term_out.is_some_and(|t| t.months == 0) {
            return Err(invalid("term_out.months must be positive".to_string()));
        }
        if let Some(price) = self.liquidation_price_pct {
            if !price.is_finite() || price <= 0.0 {
                return Err(invalid(format!(
                    "liquidation_price_pct ({price}) must be a finite positive percent of par"
                )));
            }
        }
        let mut previous_draw: Option<Date> = None;
        for draw in &self.draw_schedule {
            if draw.amount.currency() != currency || draw.amount.amount() <= 0.0 {
                return Err(invalid(format!(
                    "draw_schedule amounts must be positive {currency} amounts, got {}",
                    draw.amount
                )));
            }
            if draw.date <= self.closing_date || draw.date > self.maturity {
                return Err(invalid(format!(
                    "draw_schedule date {} must lie inside (closing, maturity]",
                    draw.date
                )));
            }
            if previous_draw.is_some_and(|prev| draw.date < prev) {
                return Err(invalid(
                    "draw_schedule must be ascending by date".to_string(),
                ));
            }
            previous_draw = Some(draw.date);
        }
        for event in &self.amortization_events {
            match event {
                AmortizationEvent::Date { date } => {
                    if *date <= self.closing_date || *date > self.maturity {
                        return Err(invalid(format!(
                            "amortization event date {date} must lie inside (closing, maturity]"
                        )));
                    }
                }
                AmortizationEvent::CumulativeLoss { max_pct } => {
                    if !max_pct.is_finite() || *max_pct <= 0.0 || *max_pct > 100.0 {
                        return Err(invalid(format!(
                            "cumulative-loss event max_pct ({max_pct}) must be a percent in (0, 100]"
                        )));
                    }
                }
                AmortizationEvent::ExcessSpread { min_3m } => {
                    if !min_3m.is_finite() {
                        return Err(invalid(
                            "excess-spread event min_3m must be finite".to_string(),
                        ));
                    }
                }
            }
        }
        self.borrowing_base_rules.validate()?;
        Ok(())
    }
}
