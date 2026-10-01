//! Cashflow-related traits and helpers.
//!
//! [`CashflowScheduleSource`] is the implementation boundary for producing a
//! raw schedule. [`CashflowProvider`] is blanket-implemented over every source
//! and owns the non-overridable public normalization and dated-flow views.
//! [`schedule_from_dated_flows`] and [`schedule_from_classified_flows`] wrap
//! ad-hoc flow lists using [`ScheduleBuildOpts`].

use crate::builder::schedule::{CashFlowMeta, CashFlowSchedule};
use crate::builder::Notional;
use crate::primitives::{is_cash_settlement_kind, CFKind, CashFlow};
pub use crate::DatedFlows;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Schedule-level inputs shared by the canonical `schedule_from_*` constructors.
#[derive(Debug, Clone, Default)]
pub struct ScheduleBuildOpts {
    /// Optional notional amount to stamp on the resulting schedule. When
    /// `None`, the constructor uses a zero notional in the currency of the
    /// first supplied flow (or USD if the list is empty).
    pub notional_hint: Option<Money>,
    /// Schedule-level metadata.
    pub meta: CashFlowMeta,
}

/// Implementation boundary for building an instrument's raw cashflow schedule.
///
/// Instruments implement this trait. Callers use [`CashflowProvider`], whose
/// blanket implementation applies the canonical public lifecycle exactly once.
pub trait CashflowScheduleSource: Send + Sync {
    /// Returns the instrument's notional amount, if applicable.
    ///
    /// Instruments with a defined notional should override this to return
    /// their principal amount. For multi-leg instruments (e.g., swaps),
    /// this typically returns the primary/receive leg notional.
    ///
    /// Default returns `None`, indicating the instrument doesn't have
    /// a simple notional concept or hasn't implemented this method.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::{Date, DayCount};
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_cashflows::builder::CashFlowSchedule;
    /// use finstack_quant_cashflows::primitives::CFKind;
    /// use finstack_quant_cashflows::{CashflowScheduleSource, schedule_from_dated_flows, ScheduleBuildOpts};
    ///
    /// struct MyInstrument {
    ///     notional: Money,
    /// }
    ///
    /// impl CashflowScheduleSource for MyInstrument {
    ///     fn raw_cashflow_schedule(
    ///         &self,
    ///         _curves: &MarketContext,
    ///         _as_of: Date,
    ///     ) -> finstack_quant_core::Result<CashFlowSchedule> {
    ///         Ok(schedule_from_dated_flows(
    ///             vec![],
    ///             CFKind::Fixed,
    ///             DayCount::Act365F,
    ///             ScheduleBuildOpts {
    ///                 notional_hint: Some(self.notional),
    ///                 ..Default::default()
    ///             },
    ///         ))
    ///     }
    ///
    ///     fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
    ///         Ok(Some(self.notional))
    ///     }
    /// }
    ///
    /// let inst = MyInstrument { notional: Money::from((1_000_000_i64, Currency::USD)) };
    /// assert_eq!(inst.notional().expect("valid notional").unwrap().currency(), Currency::USD);
    /// ```
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(None)
    }

    /// Build the complete signed schedule before public lifecycle normalization.
    ///
    /// Implementations must preserve classification and attach the correct
    /// [`crate::builder::CashflowRepresentation`] to schedule metadata. They must not perform
    /// public date filtering, PIK omission, or final sorting.
    fn raw_cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::builder::CashFlowSchedule>;
}

/// Canonical public cashflow schedule and derived dated-flow views.
///
/// This trait is blanket-implemented for every [`CashflowScheduleSource`], so
/// instruments cannot override lifecycle normalization or dated-flow meaning.
pub trait CashflowProvider: CashflowScheduleSource {
    /// Return the canonical signed cashflow schedule, future-filtered by `as_of`.
    ///
    /// The returned schedule:
    /// - Contains only flows with `date >= as_of`
    /// - Preserves fees, signed notionals, and all valid cash events
    /// - Preserves PIK classification and accrued amounts; settlement views
    ///   exclude these non-cash capitalizations
    /// - Carries the balance before `as_of` as the opening schedule notional
    ///   with no amortization recipe, because the remaining rows are resolved
    /// - Is tagged `Projected` when amounts depend on market curve projection,
    ///   `Contractual` when all future amounts are fixed by contract terms
    ///
    /// Signs represent instrument economics. Position direction determines the
    /// portfolio-level sign; there is no separate counterparty-specific schedule API.
    ///
    /// # Errors
    /// Returns an error if the schedule cannot be built due to invalid
    /// instrument parameters or missing market data.
    fn cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::builder::CashFlowSchedule> {
        self.raw_cashflow_schedule(curves, as_of)?
            .normalize_public(as_of)
    }

    /// Convenience: return flattened `(Date, Money)` flows derived from the canonical schedule.
    ///
    /// Extracts settlement rows from the [`CashFlowSchedule`] returned by
    /// [`CashflowProvider::cashflow_schedule`]. Non-cash PIK and default
    /// write-downs are excluded.
    /// Schedule signs represent instrument economics; position direction
    /// determines the portfolio-level sign.
    ///
    /// # Errors
    ///
    /// Forwards errors from [`CashflowProvider::cashflow_schedule`].
    fn dated_cashflows(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<DatedFlows> {
        let schedule = self.cashflow_schedule(curves, as_of)?;
        Ok(settlement_rows(&schedule))
    }
}

impl<T> CashflowProvider for T where T: CashflowScheduleSource + ?Sized {}

/// Extract currency-tagged settlement cash from a validated schedule.
///
/// Preserves schedule order and payment dates. PIK capitalizations and
/// default write-downs are non-cash state changes and are omitted.
///
/// # Arguments
///
/// * `schedule` - Canonical classified schedule. Amounts, accrual metadata,
///   row ordering, and issue-date consistency are validated. Single-currency
///   principal paths also reconcile against the opening notional. Composite
///   principal paths in multiple currencies cannot be reconciled against one
///   representative notional; their native cash remains currency tagged.
///
/// # Errors
///
/// Returns the schedule validation error for invalid amounts, accrual
/// metadata, currencies within a row, dates, or single-currency principal movements.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::{dated_flows, schedule_from_dated_flows, ScheduleBuildOpts};
/// use finstack_quant_cashflows::primitives::CFKind;
/// use finstack_quant_core::{currency::Currency, dates::{Date, DayCount}, money::Money};
/// use time::Month;
///
/// let date = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
/// let schedule = schedule_from_dated_flows(
///     vec![(date, Money::from((100_i64, Currency::USD)))],
///     CFKind::Fixed,
///     DayCount::Act360,
///     ScheduleBuildOpts::default(),
/// );
/// assert_eq!(dated_flows(&schedule)?[0].1.currency(), Currency::USD);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn dated_flows(schedule: &CashFlowSchedule) -> finstack_quant_core::Result<DatedFlows> {
    schedule.validate_native_cash()?;
    Ok(settlement_rows(schedule))
}

fn settlement_rows(schedule: &CashFlowSchedule) -> DatedFlows {
    schedule
        .get_flows()
        .iter()
        .filter(|flow| is_cash_settlement_kind(flow.kind))
        .filter(|flow| flow.amount.amount() != 0.0 || flow.principal_delta.is_none())
        .map(|flow| (flow.date, flow.amount))
        .collect()
}

/// Resolve the schedule-level notional from an optional `Money` hint and a
/// fallback currency inferred from the flow list.
fn resolve_notional(hint: Option<Money>, fallback_currency: Currency) -> Notional {
    Notional {
        initial: hint.unwrap_or_else(|| Money::from((0_i64, fallback_currency))),
        amort: crate::builder::AmortizationSpec::None,
    }
}

/// Build a [`CashFlowSchedule`] from instrument-signed `(Date, Money)` flows.
///
/// Schedule metadata and an optional notional hint are configured through
/// [`ScheduleBuildOpts`]. The flow kind is explicit because only this
/// constructor classifies untyped dated amounts.
///
/// # Arguments
///
/// * `flows` - List of dated cashflows as `(Date, Money)` pairs.
/// * `kind` - Classification applied to every supplied amount.
/// * `day_count` - Day count convention. **Must be explicitly specified**
///   to avoid incorrect yield/accrual calculations.
/// * `opts` - See [`ScheduleBuildOpts`]. Pass [`Default::default()`] for
///   the standard contractual schedule.
///
/// # Example
///
/// ```rust
/// use finstack_quant_cashflows::{schedule_from_dated_flows, ScheduleBuildOpts};
/// use finstack_quant_core::dates::{Date, DayCount};
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
/// use time::Month;
///
/// let flows = vec![
///     (Date::from_calendar_date(2025, Month::June, 15).unwrap(), Money::from((50_000_i64, Currency::USD))),
///     (Date::from_calendar_date(2025, Month::December, 15).unwrap(), Money::from((1_050_000_i64, Currency::USD))),
/// ];
/// let schedule = schedule_from_dated_flows(
///     flows,
///     finstack_quant_cashflows::primitives::CFKind::Fixed,
///     DayCount::Thirty360,
///     ScheduleBuildOpts::default(),
/// );
/// assert_eq!(schedule.get_day_count(), DayCount::Thirty360);
/// ```
pub fn schedule_from_dated_flows(
    flows: DatedFlows,
    kind: CFKind,
    day_count: DayCount,
    opts: ScheduleBuildOpts,
) -> CashFlowSchedule {
    let classified = flows
        .into_iter()
        .map(|(date, amount)| CashFlow::new(date, None, amount, kind, 0.0, None))
        .collect();
    schedule_from_classified_flows(classified, day_count, opts)
}

/// Build a [`CashFlowSchedule`] from pre-classified [`CashFlow`] values.
///
/// Preserves the supplied [`CFKind`] on each flow. Use this constructor when
/// callers already carry classified flows (PIK,
/// Recovery, DefaultedNotional, etc.) and want them surfaced verbatim in the
/// resulting schedule. For raw `(Date, Money)` pairs use
/// [`schedule_from_dated_flows`] instead.
///
/// # Arguments
///
/// * `flows` - Pre-classified [`CashFlow`] values; each flow's [`CFKind`] is
///   preserved as-is.
/// * `day_count` - Day count convention attached to the schedule. **Must be
///   explicitly specified** to avoid incorrect downstream yield/accrual
///   calculations.
/// * `opts` - See [`ScheduleBuildOpts`].
///
/// # Returns
///
/// A [`CashFlowSchedule`] whose flows are deterministically sorted by the
/// canonical schedule ordering and whose metadata reflects `opts.meta`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::{schedule_from_classified_flows, ScheduleBuildOpts};
/// use finstack_quant_cashflows::primitives::{CashFlow, CFKind};
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::{Date, DayCount};
/// use finstack_quant_core::money::Money;
/// use time::Month;
///
/// let date = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
/// let flows = vec![
///     CashFlow::new(date, None, Money::from((50_000_i64, Currency::USD)), CFKind::Fixed, 0.5, Some(0.05)),
///     CashFlow::new(date, None, Money::from((10_000_i64, Currency::USD)), CFKind::Pik, 0.5, Some(0.01)),
/// ];
/// let schedule = schedule_from_classified_flows(
///     flows,
///     DayCount::Act365F,
///     ScheduleBuildOpts {
///         notional_hint: Some(Money::from((1_000_000_i64, Currency::USD))),
///         ..Default::default()
///     },
/// );
/// assert_eq!(schedule.get_flows().len(), 2);
/// // Original CFKind values are preserved.
/// assert_eq!(schedule.get_flows()[0].kind, CFKind::Fixed);
/// assert_eq!(schedule.get_flows()[1].kind, CFKind::Pik);
/// ```
pub fn schedule_from_classified_flows(
    flows: Vec<CashFlow>,
    day_count: DayCount,
    opts: ScheduleBuildOpts,
) -> CashFlowSchedule {
    let inferred_currency = flows
        .first()
        .map(|cf| cf.amount.currency())
        .unwrap_or(Currency::USD);
    let notional = resolve_notional(opts.notional_hint, inferred_currency);
    CashFlowSchedule::from_parts(flows, notional, day_count, opts.meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::CashflowRepresentation;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use time::Month;

    #[test]
    fn dated_flows_rejects_invalid_principal_state() {
        let date = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
        let mut schedule = schedule_from_dated_flows(
            vec![(date, Money::from((150_i64, Currency::USD)))],
            CFKind::Amortization,
            DayCount::Act360,
            ScheduleBuildOpts {
                notional_hint: Some(Money::from((100_i64, Currency::USD))),
                ..Default::default()
            },
        );

        let error = dated_flows(&schedule).expect_err("over-amortization must fail");
        assert!(error.to_string().contains("repayments exceed outstanding"));
        let wire = serde_json::to_string(&schedule).expect("serialize schedule");
        assert!(crate::dated_flows_json(&wire).is_err());

        // Foreign fees and zero principal placeholders must not disable
        // reconciliation of the actual USD principal path.
        schedule.push_flow(CashFlow::new(
            date,
            None,
            Money::from((10_i64, Currency::EUR)),
            CFKind::Fee,
            0.0,
            None,
        ));
        schedule.push_flow(
            CashFlow::new(
                date,
                None,
                Money::from((0_i64, Currency::EUR)),
                CFKind::Notional,
                0.0,
                None,
            )
            .with_principal_delta(Money::from((0_i64, Currency::EUR))),
        );
        let error = dated_flows(&schedule)
            .expect_err("foreign non-principal rows cannot bypass balance checks");
        assert!(error.to_string().contains("repayments exceed outstanding"));
    }

    #[test]
    fn native_settlements_support_multiple_principal_currencies() {
        let date = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
        let schedule = schedule_from_dated_flows(
            vec![
                (date, Money::from((100_i64, Currency::USD))),
                (date, Money::from((90_i64, Currency::EUR))),
            ],
            CFKind::Notional,
            DayCount::Act360,
            ScheduleBuildOpts {
                notional_hint: Some(Money::from((100_i64, Currency::USD))),
                ..Default::default()
            },
        );
        assert!(matches!(
            schedule.validate(),
            Err(finstack_quant_core::Error::CurrencyMismatch { .. })
        ));
        let expected = vec![
            (date, Money::from((100_i64, Currency::USD))),
            (date, Money::from((90_i64, Currency::EUR))),
        ];
        assert_eq!(
            dated_flows(&schedule).expect("native settlements"),
            expected
        );
        let wire = serde_json::to_string(&schedule).expect("serialize schedule");
        let json = crate::dated_flows_json(&wire).expect("native JSON settlements");
        let decoded: Vec<crate::DatedFlowJson> = serde_json::from_str(&json).expect("dated rows");
        assert_eq!(
            decoded
                .into_iter()
                .map(|flow| (flow.date, flow.amount))
                .collect::<DatedFlows>(),
            expected
        );
    }

    #[test]
    fn dated_flows_omits_non_cash_state_and_matches_json() {
        let date = Date::from_calendar_date(2025, Month::June, 15).expect("valid date");
        let rows = [
            (CFKind::Fixed, 10_i64),
            (CFKind::Pik, 5),
            (CFKind::DefaultedNotional, 20),
        ]
        .into_iter()
        .map(|(kind, amount)| {
            CashFlow::new(
                date,
                None,
                Money::from((amount, Currency::USD)),
                kind,
                0.0,
                None,
            )
        })
        .collect();
        let schedule = schedule_from_classified_flows(
            rows,
            DayCount::Act360,
            ScheduleBuildOpts {
                notional_hint: Some(Money::from((100_i64, Currency::USD))),
                ..Default::default()
            },
        );

        let typed = dated_flows(&schedule).expect("valid schedule");
        assert_eq!(typed, vec![(date, Money::from((10_i64, Currency::USD)))]);
        let wire = serde_json::to_string(&schedule).expect("serialize schedule");
        let json = crate::dated_flows_json(&wire).expect("valid JSON extraction");
        let decoded: Vec<crate::DatedFlowJson> = serde_json::from_str(&json).expect("dated rows");
        assert_eq!(
            decoded
                .into_iter()
                .map(|flow| (flow.date, flow.amount))
                .collect::<DatedFlows>(),
            typed
        );
    }

    struct DummyInstrument;

    impl CashflowScheduleSource for DummyInstrument {
        fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
            Ok(Some(Money::from((1_000_000_i64, Currency::USD))))
        }

        fn raw_cashflow_schedule(
            &self,
            _curves: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            let d1 = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
            let d2 = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
            let flows = vec![
                (d1, Money::from((100_i64, Currency::USD))),
                (d2, Money::from((250_i64, Currency::USD))),
            ];
            Ok(schedule_from_dated_flows(
                flows,
                CFKind::Fixed,
                DayCount::Act365F,
                ScheduleBuildOpts {
                    notional_hint: self.notional().expect("valid notional"),
                    ..Default::default()
                },
            ))
        }
    }

    #[test]
    fn dated_cashflows_matches_schedule_contents() {
        let curves = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let dummy = DummyInstrument;
        let dated_flows = dummy
            .dated_cashflows(&curves, as_of)
            .expect("should build flows");
        assert_eq!(dated_flows.len(), 2);
        assert_eq!(dated_flows[0].1.amount(), 100.0);
        assert_eq!(dated_flows[1].1.amount(), 250.0);
    }

    struct LifecycleInstrument;

    impl CashflowScheduleSource for LifecycleInstrument {
        fn raw_cashflow_schedule(
            &self,
            _curves: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            let past = Date::from_calendar_date(2024, Month::December, 31).expect("valid date");
            let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
            let future = Date::from_calendar_date(2025, Month::February, 1).expect("valid date");
            let flows = vec![
                CashFlow::new(
                    future,
                    None,
                    Money::from((30_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    future,
                    None,
                    Money::from((40_i64, Currency::USD)),
                    CFKind::Pik,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    past,
                    None,
                    Money::from((10_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    as_of,
                    None,
                    Money::from((20_i64, Currency::USD)),
                    CFKind::Fixed,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    future,
                    None,
                    Money::from((50_i64, Currency::USD)),
                    CFKind::DefaultedNotional,
                    0.0,
                    None,
                ),
            ];
            Ok(schedule_from_classified_flows(
                flows,
                DayCount::Act365F,
                ScheduleBuildOpts {
                    notional_hint: Some(Money::from((100_i64, Currency::USD))),
                    meta: CashFlowMeta {
                        representation: CashflowRepresentation::Projected,
                        ..Default::default()
                    },
                },
            ))
        }
    }

    #[test]
    fn public_provider_owns_the_complete_cashflow_lifecycle() {
        let curves = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let instrument = LifecycleInstrument;

        let schedule = instrument
            .cashflow_schedule(&curves, as_of)
            .expect("public schedule");
        assert_eq!(
            schedule.meta.representation,
            CashflowRepresentation::Projected
        );
        assert_eq!(schedule.flows.len(), 4);
        assert_eq!(schedule.flows[0].date, as_of);
        assert_eq!(schedule.flows[1].date, as_of + time::Duration::days(31));
        assert_eq!(schedule.flows[2].date, as_of + time::Duration::days(31));
        let capitalization = schedule
            .flows
            .iter()
            .find(|f| f.kind == CFKind::Pik)
            .expect("PIK principal movement");
        assert_eq!(capitalization.amount.amount(), 40.0);

        let dated = instrument
            .dated_cashflows(&curves, as_of)
            .expect("dated cash settlements");
        assert_eq!(dated.len(), 2);
        assert_eq!(dated[0].1.amount(), 20.0);
        assert_eq!(dated[1].1.amount(), 30.0);
    }

    #[test]
    fn empty_classified_schedule_preserves_non_default_representation() {
        let schedule = schedule_from_classified_flows(
            Vec::new(),
            DayCount::Act365F,
            ScheduleBuildOpts {
                notional_hint: Some(Money::from((1_000_000_i64, Currency::USD))),
                meta: CashFlowMeta {
                    representation: CashflowRepresentation::Placeholder,
                    ..Default::default()
                },
            },
        );
        assert!(schedule.flows.is_empty());
        assert_eq!(
            schedule.meta.representation,
            CashflowRepresentation::Placeholder
        );
    }

    #[test]
    fn schedule_from_dated_flows_uses_notional_hint() {
        let flows = vec![(
            Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
            Money::from((100_i64, Currency::USD)),
        )];
        let notional = Money::from((5_000_000_i64, Currency::USD));
        let schedule = schedule_from_dated_flows(
            flows,
            CFKind::Fixed,
            DayCount::Act365F,
            ScheduleBuildOpts {
                notional_hint: Some(notional),
                ..Default::default()
            },
        );
        assert_eq!(schedule.notional.initial.amount(), 5_000_000.0);
        assert_eq!(schedule.notional.initial.currency(), Currency::USD);
    }

    #[test]
    fn schedule_from_dated_flows_defaults_currency() {
        let flows = vec![(
            Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
            Money::from((100_i64, Currency::EUR)),
        )];
        let schedule = schedule_from_dated_flows(
            flows,
            CFKind::Fixed,
            DayCount::Thirty360,
            ScheduleBuildOpts::default(),
        );
        assert_eq!(schedule.notional.initial.amount(), 0.0);
        assert_eq!(schedule.notional.initial.currency(), Currency::EUR);
        assert_eq!(schedule.day_count, DayCount::Thirty360);
    }

    #[test]
    fn schedule_from_classified_flows_preserves_kinds() {
        let date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let flows = vec![
            CashFlow::new(
                date,
                None,
                Money::from((20_i64, Currency::USD)),
                CFKind::PrePayment,
                0.0,
                None,
            ),
            CashFlow::new(
                date,
                None,
                Money::from((-5_i64, Currency::USD)),
                CFKind::DefaultedNotional,
                0.0,
                None,
            ),
        ];

        let schedule = schedule_from_classified_flows(
            flows,
            DayCount::Act365F,
            ScheduleBuildOpts {
                notional_hint: Some(Money::from((100_i64, Currency::USD))),
                ..Default::default()
            },
        );

        assert_eq!(schedule.flows.len(), 2);
        assert_eq!(schedule.flows[0].kind, CFKind::PrePayment);
        assert_eq!(schedule.flows[1].kind, CFKind::DefaultedNotional);
    }

    #[test]
    fn schedule_from_dated_flows_with_kind_applies_requested_kind() {
        let flows = vec![(
            Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
            Money::from((100_i64, Currency::USD)),
        )];

        let schedule = schedule_from_dated_flows(
            flows,
            CFKind::Notional,
            DayCount::Act365F,
            ScheduleBuildOpts {
                notional_hint: Some(Money::from((100_i64, Currency::USD))),
                ..Default::default()
            },
        );

        assert_eq!(schedule.flows.len(), 1);
        assert_eq!(schedule.flows[0].kind, CFKind::Notional);
    }

    #[test]
    fn schedule_from_classified_flows_with_meta_preserves_notional_and_meta() {
        let date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let flows = vec![CashFlow::new(
            date,
            None,
            Money::from((25_i64, Currency::USD)),
            CFKind::Fee,
            0.0,
            None,
        )];
        let notional = Notional::par(250.0, Currency::USD).expect("valid notional fixture");
        let meta = CashFlowMeta {
            projected_fixings: Vec::new(),
            representation: CashflowRepresentation::Contractual,
            calendar_ids: vec!["weekends_only".to_string()],
            commitment: Some(Money::from((500_i64, Currency::USD))),
            issue_date: Some(date),
            maturity: None,
        };

        let schedule = schedule_from_classified_flows(
            flows,
            DayCount::Act365F,
            ScheduleBuildOpts {
                notional_hint: Some(notional.initial),
                meta: meta.clone(),
            },
        );

        assert_eq!(
            schedule.notional.initial.amount(),
            notional.initial.amount()
        );
        assert_eq!(schedule.meta.issue_date, meta.issue_date);
        assert_eq!(schedule.meta.commitment, meta.commitment);
        assert_eq!(schedule.meta.calendar_ids, meta.calendar_ids);
    }
}
