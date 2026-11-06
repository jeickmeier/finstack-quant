# STATEMENTS-ANALYTICS-CREDIT-07: Cash-burn runway and period-to-month conversion

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Runway is current cash divided by positive burn per model period, converted to months using the period kind. This is a constant-burn proxy, not a cash-flow forecast or facility-liquidity simulation.

## Scope and input domain

Cash and burn nodes, model periods and warning/error thresholds in months. The mapping's prose calls the burn monthly, but the actual calculation treats it as burn for the current period kind.

## Algorithm

For each period, require finite cash and burn through the shared scalar helper. Skip burn <= 0. Compute `months=(cash/burn)*12/period.kind.periods_per_year()`. Compare strictly below the Error threshold first, then Warning. A warning remains a passed check; a negative cash balance with positive burn yields negative runway and can trigger Error.

## Selection and controls

The credit suite enables the check only with both mappings and sets 12-month Warning and 6-month Error thresholds. Its non-finite sweep does not include these optional cash/burn nodes.

## Conventions

Burn must be a positive outflow magnitude per period; monthly burn can be used unchanged only with monthly period identifiers. Conversion uses nominal period frequency, not actual elapsed days. Cash/burn yields periods before conversion.

## Limitations, failure and fallback

The check does not validate currency/units, threshold ordering or finite arithmetic output. Missing or non-finite inputs skip silently in this check. It ignores committed liquidity, minimum-cash covenants, seasonal burn, refinancing and within-period timing.

## Alternatives and tradeoffs

A dated cash-flow forecast can model variable burn and financing but requires more assumptions. A monthly-only interface would be simpler but narrower; explicit period burn is flexible if documented consistently. A zero/negative-burn status could expose excluded coverage.

## Evidence

[liquidity.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/credit/liquidity.rs), [mappings.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/mappings.rs) and [suites.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/suites.rs) own calculation/selection. [credit_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/credit_tests.rs) locates `liquidity_runway_converts_periods_to_months`, threshold and no-burn cases. This is conversion evidence, not liquidity-stress validation.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
