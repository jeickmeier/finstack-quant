//! Identified gross cash dividends per entitled share, with distinct entitlement
//! and payment dates. Continuous yields are model inputs held separately.

use crate::currency::Currency;
use crate::dates::Date;
use crate::money::Money;
use crate::types::{CurveId, UnderlyingId};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct EventWire {
    event_id: String,
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    ex_date: Date,
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pay_date: Date,
    amount: Money,
}

/// One gross cash distribution per entitled share on its schedule's share basis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EventWire")]
#[cfg_attr(
    feature = "json-schema",
    derive(schemars::JsonSchema),
    schemars(with = "EventWire")
)]
pub struct DividendEvent(EventWire);

impl TryFrom<EventWire> for DividendEvent {
    type Error = Error;

    fn try_from(wire: EventWire) -> Result<Self> {
        Self::new(wire.event_id, wire.ex_date, wire.pay_date, wire.amount)
    }
}

impl DividendEvent {
    /// Admit a distribution with its supplied ex-date and payment date.
    ///
    /// # Arguments
    ///
    /// * `event_id` - Stable, nonempty issuer or data-source event identifier.
    /// * `ex_date` - Supplied entitlement-transfer date.
    /// * `pay_date` - Supplied cash payment date, including payments before the ex-date.
    /// * `amount` - Nonnegative gross cash per one entitled share in the distribution currency.
    pub fn new(
        event_id: impl Into<String>,
        ex_date: Date,
        pay_date: Date,
        amount: Money,
    ) -> Result<Self> {
        let event_id = event_id.into();
        if event_id.trim().is_empty() || amount.amount_decimal() < rust_decimal::Decimal::ZERO {
            return Err(Error::Validation(
                "dividend requires a nonempty event ID and nonnegative cash per share".into(),
            ));
        }
        Ok(Self(EventWire {
            event_id,
            ex_date,
            pay_date,
            amount,
        }))
    }

    /// Source identity used to distinguish distributions on the same ex-date.
    pub fn get_event_id(&self) -> &str {
        &self.0.event_id
    }

    /// Supplied date of the entitlement transfer.
    pub fn get_ex_date(&self) -> Date {
        self.0.ex_date
    }

    /// Supplied date of cash payment.
    pub fn get_pay_date(&self) -> Date {
        self.0.pay_date
    }

    /// Gross cash per unit of the schedule's declared share basis.
    pub fn get_amount(&self) -> Money {
        self.0.amount
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct ScheduleWire {
    id: CurveId,
    underlying: UnderlyingId,
    share_basis: String,
    currency: Currency,
    events: Vec<DividendEvent>,
}

/// Validated distributions for one security, share basis and currency.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "ScheduleWire")]
#[cfg_attr(
    feature = "json-schema",
    derive(schemars::JsonSchema),
    schemars(with = "ScheduleWire")
)]
pub struct DividendSchedule(ScheduleWire);

impl TryFrom<ScheduleWire> for DividendSchedule {
    type Error = Error;

    fn try_from(wire: ScheduleWire) -> Result<Self> {
        Self::new(
            wire.id,
            wire.underlying,
            wire.share_basis,
            wire.currency,
            wire.events,
        )
    }
}

impl DividendSchedule {
    /// Construct a schedule from identified cash distributions in source order.
    ///
    /// # Arguments
    ///
    /// * `id` - Nonempty market lookup key for this schedule.
    /// * `underlying` - Exact nonempty identifier of the security paying the distribution.
    /// * `share_basis` - Exact nonempty identifier of the quoted share or unit entitled to one payment.
    /// * `currency` - Distribution currency required on every event.
    /// * `events` - Gross cash distributions in ascending (ex-date, event ID) order, with unique IDs.
    pub fn new(
        id: impl Into<CurveId>,
        underlying: impl Into<UnderlyingId>,
        share_basis: impl Into<String>,
        currency: Currency,
        events: Vec<DividendEvent>,
    ) -> Result<Self> {
        let id = id.into();
        let underlying = underlying.into();
        let share_basis = share_basis.into();
        if [id.as_str(), underlying.as_str(), &share_basis]
            .iter()
            .any(|v| v.trim().is_empty())
        {
            return Err(Error::Validation(
                "dividend schedule requires an ID, underlying and share basis".into(),
            ));
        }
        let mut ids = HashSet::new();
        let mut previous = None;
        for event in &events {
            let key = (event.get_ex_date(), event.get_event_id());
            if !ids.insert(event.get_event_id()) || previous.is_some_and(|prior| prior >= key) {
                return Err(Error::Validation(
                    "dividend events require unique IDs and ascending (ex_date, event_id) order"
                        .into(),
                ));
            }
            if event.get_amount().currency() != currency {
                return Err(Error::Validation(format!(
                    "dividend {} currency differs from schedule currency {currency}",
                    event.get_event_id()
                )));
            }
            previous = Some(key);
        }
        Ok(Self(ScheduleWire {
            id,
            underlying,
            share_basis,
            currency,
            events,
        }))
    }

    /// Key used to retrieve the schedule.
    pub fn get_id(&self) -> &CurveId {
        &self.0.id
    }

    /// Exact identity of the underlying security.
    pub fn get_underlying(&self) -> &UnderlyingId {
        &self.0.underlying
    }

    /// Exact share or unit basis of each per-share amount.
    pub fn get_share_basis(&self) -> &str {
        &self.0.share_basis
    }

    /// Distribution currency shared by every event.
    pub fn get_currency(&self) -> Currency {
        self.0.currency
    }

    /// Distributions in canonical entitlement-date and event-ID order.
    pub fn get_events(&self) -> &[DividendEvent] {
        &self.0.events
    }
}
