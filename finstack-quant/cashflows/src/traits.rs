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
    ///     fn notional(&self) -> Option<Money> {
    ///         Some(self.notional)
    ///     }
    /// }
    ///
    /// let inst = MyInstrument { notional: Money::new(1_000_000.0, Currency::USD) };
    /// assert_eq!(inst.notional().unwrap().currency(), Currency::USD);
    /// ```
    fn notional(&self) -> Option<Money> {
        None
    }

    /// Build the complete signed schedule before public lifecycle normalization.
    ///
    /// Implementations must preserve classification and attach the correct
    /// [`CashflowRepresentation`] to schedule metadata. They must not perform
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
    /// - Omits pure PIK accretion (notional capitalisation without cash movement)
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
        Ok(self
            .raw_cashflow_schedule(curves, as_of)?
            .normalize_public(as_of))
    }

    /// Convenience: return flattened `(Date, Money)` flows derived from the canonical schedule.
    ///
    /// Simply converts the [`CashFlowSchedule`] returned by
    /// [`CashflowProvider::cashflow_schedule`] into a `Vec<(Date, Money)>`.
    /// Schedule signs represent instrument economics; position direction
    /// determines the portfolio-level sign.
    ///
    /// # Errors
    ///
    /// Forwards any error returned by [`CashflowProvider::cashflow_schedule`].
    fn dated_cashflows(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<DatedFlows> {
        let schedule = self.cashflow_schedule(curves, as_of)?;
        Ok(schedule
            .flows
            .iter()
            .filter(|cf| is_cash_settlement_kind(cf.kind))
            .map(|cf| (cf.date, cf.amount))
            .collect())
    }
}

impl<T> CashflowProvider for T where T: CashflowScheduleSource + ?Sized {}

/// Resolve the schedule-level notional from an optional `Money` hint and a
/// fallback currency inferred from the flow list.
fn resolve_notional(hint: Option<Money>, fallback_currency: Currency) -> Notional {
    match hint {
        Some(money) => Notional::par(money.amount(), money.currency()),
        None => Notional::par(0.0, fallback_currency),
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
///     (Date::from_calendar_date(2025, Month::June, 15).unwrap(), Money::new(50_000.0, Currency::USD)),
///     (Date::from_calendar_date(2025, Month::December, 15).unwrap(), Money::new(1_050_000.0, Currency::USD)),
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
///     CashFlow::new(date, None, Money::new(50_000.0, Currency::USD), CFKind::Fixed, 0.5, Some(0.05)),
///     CashFlow::new(date, None, Money::new(10_000.0, Currency::USD), CFKind::PIK, 0.5, Some(0.01)),
/// ];
/// let schedule = schedule_from_classified_flows(
///     flows,
///     DayCount::Act365F,
///     ScheduleBuildOpts {
///         notional_hint: Some(Money::new(1_000_000.0, Currency::USD)),
///         ..Default::default()
///     },
/// );
/// assert_eq!(schedule.get_flows().len(), 2);
/// // Original CFKind values are preserved.
/// assert_eq!(schedule.get_flows()[0].kind, CFKind::Fixed);
/// assert_eq!(schedule.get_flows()[1].kind, CFKind::PIK);
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

    struct DummyInstrument;

    impl CashflowScheduleSource for DummyInstrument {
        fn notional(&self) -> Option<Money> {
            Some(Money::new(1_000_000.0, Currency::USD))
        }

        fn raw_cashflow_schedule(
            &self,
            _curves: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            let d1 = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
            let d2 = Date::from_calendar_date(2025, Month::July, 1).expect("valid date");
            let flows = vec![
                (d1, Money::new(100.0, Currency::USD)),
                (d2, Money::new(250.0, Currency::USD)),
            ];
            Ok(schedule_from_dated_flows(
                flows,
                CFKind::Fixed,
                DayCount::Act365F,
                ScheduleBuildOpts {
                    notional_hint: self.notional(),
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
                    Money::new(30.0, Currency::USD),
                    CFKind::Fixed,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    future,
                    None,
                    Money::new(40.0, Currency::USD),
                    CFKind::PIK,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    past,
                    None,
                    Money::new(10.0, Currency::USD),
                    CFKind::Fixed,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    as_of,
                    None,
                    Money::new(20.0, Currency::USD),
                    CFKind::Fixed,
                    0.0,
                    None,
                ),
                CashFlow::new(
                    future,
                    None,
                    Money::new(50.0, Currency::USD),
                    CFKind::DefaultedNotional,
                    0.0,
                    None,
                ),
            ];
            Ok(schedule_from_classified_flows(
                flows,
                DayCount::Act365F,
                ScheduleBuildOpts {
                    meta: CashFlowMeta {
                        representation: CashflowRepresentation::Projected,
                        ..Default::default()
                    },
                    ..Default::default()
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
        assert_eq!(schedule.flows.len(), 3);
        assert_eq!(schedule.flows[0].date, as_of);
        assert_eq!(schedule.flows[1].date, as_of + time::Duration::days(31));
        assert_eq!(schedule.flows[2].date, as_of + time::Duration::days(31));
        assert!(schedule.flows.iter().all(|flow| flow.kind != CFKind::PIK));

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
                notional_hint: Some(Money::new(1_000_000.0, Currency::USD)),
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
            Money::new(100.0, Currency::USD),
        )];
        let notional = Money::new(5_000_000.0, Currency::USD);
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
            Money::new(100.0, Currency::EUR),
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
                Money::new(20.0, Currency::USD),
                CFKind::PrePayment,
                0.0,
                None,
            ),
            CashFlow::new(
                date,
                None,
                Money::new(-5.0, Currency::USD),
                CFKind::DefaultedNotional,
                0.0,
                None,
            ),
        ];

        let schedule = schedule_from_classified_flows(
            flows,
            DayCount::Act365F,
            ScheduleBuildOpts {
                notional_hint: Some(Money::new(100.0, Currency::USD)),
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
            Money::new(100.0, Currency::USD),
        )];

        let schedule = schedule_from_dated_flows(
            flows,
            CFKind::Notional,
            DayCount::Act365F,
            ScheduleBuildOpts {
                notional_hint: Some(Money::new(100.0, Currency::USD)),
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
            Money::new(25.0, Currency::USD),
            CFKind::Fee,
            0.0,
            None,
        )];
        let notional = Notional::par(250.0, Currency::USD);
        let meta = CashFlowMeta {
            representation: CashflowRepresentation::Contractual,
            calendar_ids: vec!["weekends_only".to_string()],
            facility_limit: Some(Money::new(500.0, Currency::USD)),
            issue_date: Some(date),
            maturity_date: None,
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
        assert_eq!(schedule.meta.facility_limit, meta.facility_limit);
        assert_eq!(schedule.meta.calendar_ids, meta.calendar_ids);
    }
}
