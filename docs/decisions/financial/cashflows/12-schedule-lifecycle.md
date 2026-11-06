# CASHFLOWS-012 — Canonical schedules, balance replay and residual normalization

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and domain

Schedules preserve classified native-currency rows, explicit economic principal deltas and flow-owned accrual metadata. Public provider methods normalize the raw schedule to a valuation date and expose settlement cash without paying PIK or default-state movements as cash. There is no FX conversion in these operations. Source comments motivate keeping economic balance dates distinct from cash settlement dates.

## Canonicalization and validation

Rows sort by payment date, then economic kind rank: interest; fees; amortization; prepayment; defaulted principal; recovery/default accrual; PIK; notionals/revolving movements; margin/collateral; other. Ties use currency, exact Decimal amount, reset/economic dates, accrual factor, rate, detailed accrual metadata, principal delta, kind discriminant and Decimal representation. This guarantees deterministic serialization beyond economic equality.

`from_flows` validates each row then sorts; it does **not** perform full notional/principal validation. Serde sorts but is structural; `validate` separately checks notional, rows, sorted dates, principal balances and interest payments before issue. Flows over 100 years after issue warn rather than fail. Principal negative-balance rejection tolerance is `max(abs(initial)×1e-8,1e-6)`; replay itself warns below `-1e-9`. Mutators and scale/merge operations can require a later explicit validation. `try_update_flows` can leave earlier rows modified on callback failure and does not reach its final sort in that case.

## Balance reconstruction

1. Start with the schedule's initial notional. Order rows by `get_balance_date()` then canonical order. Explicit `principal_delta` overrides inferred movement.
2. Skip exactly one inferred initial funding row: negative Notional, no explicit delta, cash date equal to funding anchor, and magnitude equal to initial principal within relative `1e-12` with scale floor one.
3. Otherwise infer `+amount` for PIK; `−amount` for amortization, prepayment, defaulted principal, Notional, revolving draw or repayment; other kinds leave balance unchanged. Use checked Money arithmetic and emit closing balance per economic date.
4. `outstanding_by_date` and `balances_by_flow` require explicit issue metadata. `outstanding_at_dates` requires sorted queries and returns the closing balance including movements on the query date. `outstanding_on` separately floors its lookup result at zero and assumes the supplied path is sorted.
5. `balances_by_flow` records before/after principal; for nonmoving accrual rows it attaches balance held strictly before accrual end. It rounds displayed residues below half a currency minor unit to zero. That display cleanup does not redefine the underlying replay/validation tolerance.

## Public residual algorithm

`CashflowProvider` is blanket-implemented and always calls `raw_cashflow_schedule(...).normalize_public(as_of)`:

- Rebase opening principal using economic movements strictly **before** `as_of` in the initial-notional currency. A movement exactly on `as_of` remains in the residual stream.
- If that past movement's cash is still pending (`cash_date>=as_of`), preserve cash but set its explicit principal delta to zero, preventing double balance movement.
- If cash settled before `as_of` but its economic date is still future/on-date, create a zero-cash Notional row retaining the future principal delta and economic date.
- Clear the inception amortization recipe to None; retained rows already resolve remaining movements. Remove cash rows dated strictly before `as_of`, retain on-date rows and resort. Original issue/maturity metadata is retained.
- Settlement view includes only `is_cash_settlement` kinds and excludes zero-cash rows carrying explicit deltas. PIK and DefaultedNotional are state, not cash. Provider's dated view directly uses normalized settlement rows; schedule's public `dated_flows` additionally calls native-cash validation.

Full scalar-principal validation is unsuitable for multiple principal currencies. `dated_flows` validates structure/dates but skips single-balance validation when more than one nonzero principal currency is present. Raw adapters without a notional hint use zero in the first flow's currency, or USD when empty; they do not infer face principal from cash.

## Merge and scale choices

Merge concatenates/sorts rows, unions calendars, normalizes projected observations and keeps commitment/issue/maturity only when every input agrees. Representation precedence is Projected, Placeholder, Contractual, then NoResidual. Caller supplies combined notional and representative day count; merge does not prove economic consistency. `scale_amounts` scales cash and explicit principal deltas, not opening notional or amortization recipe, and resorts after sign changes. Caller must set compatible notional before asking for balance-based analytics.

## Alternatives and evidence

Cash-date-only filtering would lose pending economic movements; scalar validation across currencies would invent FX economics. Both differ from implementation.

- [schedule.rs](../../../../finstack-quant/cashflows/src/builder/schedule.rs), [traits.rs](../../../../finstack-quant/cashflows/src/traits.rs), [JSON validate/dated adapters](../../../../finstack-quant/cashflows/src/json.rs).
- Located invariant/regression tests in schedule source for exact canonical ordering, structural serde, mixed-currency merge, normalization before/after economic/cash dates, and same-day opening balance. Traits tests locate `public_provider_owns_the_complete_cashflow_lifecycle` and `native_settlements_support_multiple_principal_currencies`.
- No external standard dictates this internal representation. Tests were not executed.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
