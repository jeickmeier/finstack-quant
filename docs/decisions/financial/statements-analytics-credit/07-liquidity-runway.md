# STATEMENTS-ANALYTICS-CREDIT-07: Cash-burn runway and period-to-month conversion

- Decision status: **Accepted for the full liquidity runway contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for liquidity runway in reporting months

1. Use verified period-end spendable cash and a named signed net cash-use series in the same selected reporting period, scope and currency. Disclose the included flows. Positive net cash use is burn, zero is flat cash use, and negative net cash use is cash generation.
2. Validate the reporting calendar before reading observations. Reject daily and weekly calendars. With positive burn, divide spendable cash by burn per period and multiply by 1, 3, 6 or 12 months for monthly, quarterly, semiannual or annual periods, respectively. Zero cash with positive burn gives zero months. With nonnegative cash, zero burn and negative burn make the ratio inapplicable for distinct reasons. If explicitly signed available cash is negative, report an immediate shortfall, its magnitude and zero remaining runway, and fail the check. [Reporting periods and calendar slots](../core-conventions/014-fiscal-periods-ltm.md#accepted-decision-reporting-periods-fiscal-context-and-ltm-windows).
3. Use sourced, finite thresholds in months with `0 ≤ error floor ≤ warning floor`. Evaluate the error floor first. Equality meets the corresponding floor.
4. Report the inputs, cadence, runway or reason it is unavailable, and threshold outcome. Missing required inputs make the check incomplete. Invalid inputs or arithmetic produce errors. [Numerical comparisons and check suites](../statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).

For example, USD 200 of period-end spendable cash and USD 100 of quarterly burn give six months of runway. This meets a six-month error floor and breaches a nine-month warning floor.

## Audited choice and rationale

Runway is current cash divided by positive burn per model period, converted to months using the period kind. This is a constant-burn proxy, not a cash-flow forecast or facility-liquidity simulation.

## Audited scope and input domain

Cash and burn nodes, model periods and warning/error thresholds in months. The mapping's prose calls the burn monthly, but the actual calculation treats it as burn for the current period kind.

## Audited algorithm

For each period, require finite cash and burn through the shared scalar helper. Skip burn <= 0. Compute `months=(cash/burn)*12/period.kind.periods_per_year()`. Compare strictly below the Error threshold first, then Warning. A warning remains a passed check; a negative cash balance with positive burn yields negative runway and can trigger Error.

## Audited selection and controls

The credit suite enables the check only with both mappings and sets 12-month Warning and 6-month Error thresholds. Its non-finite sweep does not include these optional cash/burn nodes.

## Audited conventions

Burn must be a positive outflow magnitude per period; monthly burn can be used unchanged only with monthly period identifiers. Conversion uses nominal period frequency, not actual elapsed days. Cash/burn yields periods before conversion.

## Audited limitations, failure and fallback

The check does not validate currency/units, threshold ordering or finite arithmetic output. Missing or non-finite inputs skip silently in this check. It ignores committed liquidity, minimum-cash covenants, seasonal burn, refinancing and within-period timing.

## Audited alternatives and tradeoffs

A dated cash-flow forecast can model variable burn and financing but requires more assumptions. A monthly-only interface would be simpler but narrower; explicit period burn is flexible if documented consistently. A zero/negative-burn status could expose excluded coverage.

## Evidence

[liquidity.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/credit/liquidity.rs), [mappings.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/mappings.rs) and [suites.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/suites.rs) own calculation/selection. [credit_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/credit_tests.rs) locates `liquidity_runway_converts_periods_to_months`, threshold and no-burn cases. This is conversion evidence, not liquidity-stress validation.

The live conversion uses Core's `PeriodKind::slots_per_year`. It rejects daily and weekly periods when a finite pair with positive burn reaches conversion. Calendar admission before evaluating observations remains part of the accepted reconciliation. The mapping documentation still labels the input monthly burn. [Live runway check](../../../../finstack-quant/statements-analytics/src/analysis/checks/credit/liquidity.rs) · [Live credit mapping](../../../../finstack-quant/statements-analytics/src/analysis/checks/mappings.rs).

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
