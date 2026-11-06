# CASHFLOWS-015 — Survival-adjusted reporting PV and funded-principal recovery

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped to cashflow PV aggregation, not a complete instrument credit pricer. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice and domain

The reporting kernel discounts future settlement cash, conditions survival on the valuation date and optionally estimates recovery on funded principal at each repayment's cash payment date. This is a discrete payment-date recovery approximation. It does not integrate recovery at default time and does not calibrate hazard or recovery. Source comments motivate excluding noncash default-state/PIK rows and preventing recovery on principal before funding.

## Settlement formula and curve conventions

For future cash at `T`, relative discount is `D(as_of,T)` from `Discounting::df_between_dates`; conditional survival is `Q(T)=SP(T)/SP(as_of)`. Require finite positive base survival and finite future survival in `[0,1]`. At as_of use survival one. No hazard means survival one.

- Noncash kinds, and all dates `<=as_of`, receive zero PV.
- Recovery and AccruedOnDefault receive `cash×D`, without another survival factor.
- Other settlement cash, including negative future funding commitments, receives `cash×D×Q(T)`.

The row API requires one supplied discount factor per flow; every factor must be finite nonnegative (zero is permitted). Historical rows still have discount inputs validated. The period wrapper computes relative discounts for future dates and one for past/on-date rows, then buckets the row PVs using [CASHFLOWS-014](14-aggregation-and-life.md). Schedule convenience lookup takes a discount curve ID and optional hazard ID and automatically takes recovery rate from the hazard curve.

## Recovery exposure and allocation algorithm

Recovery input must be finite in `[0,1]` and requires a hazard curve even when rate is zero. Positive estimated recovery is rejected if **any** DefaultedNotional row is present, to avoid mixing explicit default events with inferred recovery. Explicit Recovery rows alone do not trigger this guard.

For positive recovery, process each native currency separately:

1. Identify positive future repayment claims by principal-repayment kind and positive cash. For economic dates on/after valuation, a negative explicit principal delta supplies par amount, positive delta suppresses repayment, and zero/absent delta uses cash. For economic dates before valuation, still-pending repayment uses positive cash amount.
2. Collect future economic principal increases. Explicit positive delta overrides inference; otherwise PIK uses positive amount and Notional/RevolvingDraw uses negative cash magnitude. Separate PIK or zero-cash capitalizations from ordinary advances.
3. Pair an early-paid or zero-cash repayment with same-economic-date capitalization first. The already-paid portion creates no new recoverable claim. A still-future early cash payment receives a lot beginning on its cash date, preventing recovery before capitalization for that paired claim.
4. Infer opening funded principal as remaining positive repayment claims minus future advances/capitalizations. Reject materially negative opening amount using tolerance `max(largest repayment,1)×1e-12`; a complete future principal stream is required.
5. Put positive inferred opening principal in a lot at as_of; append future funding lots in economic-date order. Sort repayments by cash date and allocate each **FIFO** across funding lots. Reject repayments before the required lot's funding date or repayments exceeding remaining lots beyond tolerance.
6. For a repayment at `T` with lots `(tj,Nj)`, add `recovery × D(as_of,T) × Σ Nj[Q(tj)−Q(T)]`. Reject survival increases above `1e-12` over a funded interval; clamp tiny negative default mass to zero. Allocate recovery PV to that repayment's row and reporting period.

Recovery exposure is therefore tied to actual funded intervals and par deltas where available, while the payment-date discount approximation defers every default recovery in a lot to its scheduled principal payment date.

## Alternatives and limitations

Supported alternatives are no hazard, survival-only with no/zero recovery, explicit post-default flows, or this funded-principal approximation. Immediate recovery-at-default integration, recovery of market value/treasury value, stochastic recovery and counterparty-specific treatment of unfunded commitments are not implemented here.

The kernel infers opening recoverable principal from remaining cashflow structure rather than accepting schedule notional. Incomplete principal streams fail or imply different opening exposure. Recovery/protection rows without DefaultedNotional can coexist with estimated recovery; users must ensure they are not overlapping claims. The row-level function does not itself full-validate an enclosing schedule or guarantee currency/curve economic matching.

## Evidence

- [aggregation.rs](../../../../finstack-quant/cashflows/src/aggregation.rs): `credit_adjusted_cashflow_pvs`, `funded_principal`, `default_mass`, `survival_on_date`, `pv_by_period`; [schedule curve dispatch](../../../../finstack-quant/cashflows/src/builder/schedule.rs).
- Inspected `funded_recovery_payment_discount_matches_independent_interval_formula`: explicit exponential relative discount and survival powers establish the payment-date formula independently of the production allocation kernel. Located tests cover forward funding, FIFO amortization/draw/PIK, explicit delta versus cash, early PIK repayment, pending settlements, currency separation and DefaultedNotional guard. [credit_models.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/credit_models.rs) locates explicit default/recovery PV behavior.
- [Hull–White CDS](../../../REFERENCES.md#hull-white-2000-cds) is a credit-pricing reference pointer, not evidence that this reporting approximation is equivalent to a CDS/default-time integral.
- Tests were not executed.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
