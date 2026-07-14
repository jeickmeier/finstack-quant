//! Principal, amortization, and ad-hoc principal-event builder methods.

use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

use crate::builder::orchestrator::{CashFlowBuilder, PrincipalEvent};
use crate::builder::{AmortizationSpec, Notional};
use crate::primitives::CFKind;

impl CashFlowBuilder {
    /// Sets principal details and instrument horizon.
    ///
    /// Full-horizon coupons and amortization may be configured before or after
    /// this method. Their horizon is resolved when the schedule is built.
    /// Calling this method does not clear a previously recorded builder error.
    ///
    /// # Arguments
    ///
    /// * `initial` - Initial outstanding principal and currency.
    /// * `issue_date` - Start date for the instrument.
    /// * `maturity` - Final maturity date.
    ///
    /// # Returns
    ///
    /// Mutable builder reference for fluent chaining.
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn principal(&mut self, initial: Money, issue_date: Date, maturity: Date) -> &mut Self {
        self.notional = Some(Notional {
            initial,
            amort: self.amortization.clone().unwrap_or(AmortizationSpec::None),
        });
        self.issue = Some(issue_date);
        self.maturity = Some(maturity);
        self
    }

    /// Configures amortization for the instrument notional.
    ///
    /// This may be called before or after [`principal`](Self::principal).
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn amortization(&mut self, spec: AmortizationSpec) -> &mut Self {
        self.amortization = Some(spec.clone());
        if let Some(n) = &mut self.notional {
            n.amort = spec;
        }
        self
    }

    /// Adds a single principal event.
    ///
    /// `delta` controls the outstanding balance change. The emitted cashflow
    /// sign is derived from `kind`: `CFKind::Amortization` emits `cash` as a
    /// positive repayment, while all other kinds emit `-cash` as a borrower
    /// draw/notional cashflow.
    ///
    /// # Sign conventions
    ///
    /// * `CFKind::Amortization` (repayment): `delta` must be `<= 0` (the
    ///   outstanding balance decreases). When `cash` is `None`, it defaults to
    ///   `-delta` so the emitted flow is a positive cash repayment.
    /// * `CFKind::Notional` (draw): `delta` must be `>= 0` (the outstanding
    ///   balance increases). When `cash` is `None`, it defaults to `delta`,
    ///   emitting a negative funding outflow.
    /// * Other kinds: no sign requirement; `cash` defaults to `delta`.
    ///
    /// # Errors
    ///
    /// Records a pending error — returned by `build(...)` — when:
    /// * `cash` is provided with a different currency than `delta`, or
    /// * `delta` violates the sign convention for `kind` (including NaN
    ///   amounts, which never satisfy either sign requirement).
    #[must_use = "builder methods should be chained or terminated with .build(...)"]
    pub fn add_principal_event(
        &mut self,
        date: Date,
        delta: Money,
        cash: Option<Money>,
        kind: CFKind,
    ) -> &mut Self {
        if self.pending_error.is_some() {
            return self;
        }
        match kind {
            CFKind::Amortization => {
                if delta.amount() > 0.0 || delta.amount().is_nan() {
                    self.pending_error = Some(finstack_quant_core::Error::Validation(format!(
                        "add_principal_event: CFKind::Amortization requires delta <= 0 \
                         (repayments reduce outstanding), got {} on {date}",
                        delta.amount()
                    )));
                    return self;
                }
            }
            CFKind::Notional => {
                if delta.amount() < 0.0 || delta.amount().is_nan() {
                    self.pending_error = Some(finstack_quant_core::Error::Validation(format!(
                        "add_principal_event: CFKind::Notional requires delta >= 0 \
                         (draws increase outstanding), got {} on {date}",
                        delta.amount()
                    )));
                    return self;
                }
            }
            _ => {}
        }
        let cash_leg = cash.unwrap_or(match kind {
            CFKind::Amortization => delta * -1.0,
            _ => delta,
        });
        if cash_leg.currency() != delta.currency() {
            self.pending_error = Some(finstack_quant_core::Error::CurrencyMismatch {
                expected: delta.currency(),
                actual: cash_leg.currency(),
            });
            return self;
        }
        self.principal_events.push(PrincipalEvent {
            date,
            delta,
            cash: cash_leg,
            kind,
        });
        self
    }
}
