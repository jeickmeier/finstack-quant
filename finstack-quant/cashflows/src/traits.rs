//! Cashflow-related traits and helpers.
//!
//! [`CashflowProvider`] is the instrument boundary for emitting a
//! [`crate::builder::CashFlowSchedule`]. [`schedule_from_dated_flows`] and
//! [`schedule_from_classified_flows`] wrap ad-hoc flow lists using
//! [`ScheduleBuildOpts`].

use crate::builder::schedule::{CashFlowMeta, CashFlowSchedule, CashflowRepresentation};
use crate::builder::Notional;
use crate::primitives::{is_cash_settlement_kind, CFKind, CashFlow};
pub use crate::DatedFlows;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Options bag shared by the canonical public `schedule_from_*` constructors.
///
/// Gathers the orthogonal knobs (notional hint, default kind, representation,
/// schedule-level metadata) that previously required a fan-out of
/// `_with_representation`, `_with_kind`, and `_with_meta` variants. Construct
/// with struct-update syntax and pass [`Default::default()`] when you only
/// need the base behavior.
#[derive(Debug, Clone, Default)]
pub struct ScheduleBuildOpts {
    /// Optional notional amount to stamp on the resulting schedule. When
    /// `None`, the constructor uses a zero notional in the currency of the
    /// first supplied flow (or USD if the list is empty).
    pub notional_hint: Option<Money>,
    /// Override the default [`CFKind`] applied to dated flows that have no
    /// intrinsic classification. `None` (the default) means [`CFKind::Fixed`].
    /// Ignored by constructors that already receive pre-classified
    /// [`CashFlow`] values.
    pub kind: Option<CFKind>,
    /// Representation tag (`Contractual` vs `Projected` vs `Placeholder`).
    pub representation: CashflowRepresentation,
    /// Schedule-level metadata (calendar IDs, facility limit, issue date).
    /// When supplied, takes precedence over `representation`.
    pub meta: Option<CashFlowMeta>,
}

impl ScheduleBuildOpts {
    /// Apply the resolved options to build the final `CashFlowMeta`.
    fn resolved_meta(&self) -> CashFlowMeta {
        self.meta.clone().unwrap_or_else(|| CashFlowMeta {
            representation: self.representation,
            ..Default::default()
        })
    }
}

/// Build cashflow schedules and provide currency-safe aggregation hooks.
///
/// Instruments implement this to generate their canonical signed cashflow schedule
/// given market curves and valuation date. The returned schedule is future-filtered
/// (`date >= as_of`), preserves fees and signed notionals, omits pure PIK accretion,
/// and tags curve-dependent amounts as `Projected`.
pub trait CashflowProvider: Send + Sync {
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
    /// use finstack_quant_cashflows::{CashflowProvider, schedule_from_dated_flows, ScheduleBuildOpts};
    ///
    /// struct MyInstrument {
    ///     notional: Money,
    /// }
    ///
    /// impl CashflowProvider for MyInstrument {
    ///     fn cashflow_schedule(
    ///         &self,
    ///         _curves: &MarketContext,
    ///         _as_of: Date,
    ///     ) -> finstack_quant_core::Result<CashFlowSchedule> {
    ///         Ok(schedule_from_dated_flows(
    ///             vec![],
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
    ) -> finstack_quant_core::Result<crate::builder::CashFlowSchedule>;

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
    /// Override this method only if you want to bypass schedule construction
    /// and produce dated flows by some other means.
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
/// All orthogonal knobs (notional hint, default [`CFKind`], representation,
/// schedule metadata) are configured through [`ScheduleBuildOpts`], which
/// implements [`Default`] for the common "contractual, fixed, no hint" case.
///
/// # Arguments
///
/// * `flows` - List of dated cashflows as `(Date, Money)` pairs.
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
/// let schedule = schedule_from_dated_flows(flows, DayCount::Thirty360, ScheduleBuildOpts::default());
/// assert_eq!(schedule.day_count, DayCount::Thirty360);
/// ```
pub fn schedule_from_dated_flows(
    flows: DatedFlows,
    day_count: DayCount,
    opts: ScheduleBuildOpts,
) -> CashFlowSchedule {
    let kind = opts.kind.unwrap_or(CFKind::Fixed);
    let classified = flows
        .into_iter()
        .map(|(date, amount)| CashFlow::new(date, None, amount, kind, 0.0, None))
        .collect();
    schedule_from_classified_flows(classified, day_count, opts)
}

/// Build a [`CashFlowSchedule`] from pre-classified [`CashFlow`] values.
///
/// Preserves the supplied [`CFKind`] on each flow; `opts.kind` is ignored.
/// Use this constructor when callers already carry classified flows (PIK,
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
/// * `opts` - See [`ScheduleBuildOpts`]. The `kind` field is ignored because
///   the supplied `CashFlow`s already carry classifications.
///
/// # Returns
///
/// A [`CashFlowSchedule`] whose flows are deterministically sorted by the
/// canonical schedule ordering and whose metadata reflects `opts.meta` (or
/// `opts.representation` if `meta` is absent).
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
/// assert_eq!(schedule.flows.len(), 2);
/// // Original CFKind values are preserved; opts.kind is ignored on this constructor.
/// assert_eq!(schedule.flows[0].kind, CFKind::Fixed);
/// assert_eq!(schedule.flows[1].kind, CFKind::PIK);
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
    CashFlowSchedule::from_parts(flows, notional, day_count, opts.resolved_meta())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use time::Month;

    struct DummyInstrument;

    impl CashflowProvider for DummyInstrument {
        fn notional(&self) -> Option<Money> {
            Some(Money::new(1_000_000.0, Currency::USD))
        }

        fn cashflow_schedule(
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

    #[test]
    fn empty_classified_schedule_preserves_non_default_representation() {
        let schedule = schedule_from_classified_flows(
            Vec::new(),
            DayCount::Act365F,
            ScheduleBuildOpts {
                notional_hint: Some(Money::new(1_000_000.0, Currency::USD)),
                representation: CashflowRepresentation::Placeholder,
                ..Default::default()
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
        let schedule =
            schedule_from_dated_flows(flows, DayCount::Thirty360, ScheduleBuildOpts::default());
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
            DayCount::Act365F,
            ScheduleBuildOpts {
                notional_hint: Some(Money::new(100.0, Currency::USD)),
                kind: Some(CFKind::Notional),
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
                meta: Some(meta.clone()),
                ..Default::default()
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
