# MARGIN-REGULATORY-XVA-007: FX delta equicorrelation norm

- Decision status: **Accepted 2026-11-17 for checked FX delta.**
- Implementation status: Existing behavior is described below. Canonical reporting-currency factor identity, elective pair relief and checked aggregation and result paths remain pending.
- Validation status: Primary references and published pair list checked. Independent examples covered the reciprocal shock, pair and first-order cross eligibility, weight scales and all three scenario norms. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked FX delta

1. **Use one canonical bucket per risk currency.** Fix the reporting
   currency for the calculation and identify each bucket by its distinct
   risk currency. Reject self-pairs. Map qualified onshore, offshore,
   deliverable and non-deliverable variants to the same regulatory currency
   and retain their source evidence. Net only contributions to the same
   canonical bucket.
   [Basel MAR21.14 and MAR21.86](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
2. **Require signed P&L for the exact `1%` relative shock.** Define the
   quote as reporting-currency units per unit of risk currency. Reprice
   at `1.01 × quote`, holding the other factors fixed. A reciprocal
   source quote reproduces the prescribed shock by dividing its quote by
   `1.01`. Require every applicable reporting-currency leg. For example,
   a USD/JPY forward in a CAD report requires USD/CAD and JPY/CAD
   sensitivities.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance) ·
   [Basel MAR21.14–16 and MAR21.24](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Support the direct reporting-currency method.** A calculation
   against a separate base currency is unavailable until a qualified route
   supports one supervisory-approved base currency, explicit translation
   risk and dated spot conversion of the resulting capital charge.
   [Basel MAR21.14(1)](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
4. **Make pair relief an explicit profile election.** Use `15` as the
   standard multiplier on prepared P&L. Apply `15/√2` to eligible buckets
   under a profile that elects and permits relief. Eligible pairs are the
   published pairs and their first-order crosses formed from two listed
   pairs sharing one currency. Evaluate eligibility from the two
   currencies, retain the prescribed reporting quote orientation and
   apply the election consistently to all eligible buckets. EUR/AUD
   qualifies through USD/EUR and USD/AUD under the published list.
   [Basel MAR21.87–88 and footnotes 22–23](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
5. **Keep the checked linear-time aggregation.** Let `w_b` be the net
   weighted sensitivity of bucket `b`. Report `K_b = |w_b|` and `S_b = w_b`.
   Transform the `0.60` cross-bucket correlation to `0.45`, `0.60` and `0.75`
   for Low, Medium and High. Calculate
   `√((1 − r) Σ_b w_b² + r (Σ_b w_b)²)` with checked, scaled arithmetic
   in canonical order. Two weighted buckets of `+150` and `−150` give
   approximately `134.1641` in Medium.
   [FRTB aggregation](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation) ·
   [Basel MAR21.4, MAR21.6 and MAR21.89](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
6. **Preserve coverage and the applied weight.** Carry the currency, net
   amount, relief evidence, weight and scenario results through Rust,
   wire, Python and WASM. Report evaluated zero only when coverage
   establishes it. Pass the elected delta weight to the curvature owner
   for the applicable shock size.
   [Curvature record](11-frtb-curvature.md) ·
   [Basel MAR21.98](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

FX delta treats each ordered currency-pair key as a risk factor and uses one uniform pair correlation. Inputs are P&L per 1% FX-rate move already in reporting currency.

## Algorithm and conventions

1. For each pair form `w=15×delta` using the fixed unrelieved FX risk weight.
2. Transform base `rho=0.60` under the chosen low/medium/high scenario.
3. Calculate `K=sqrt(max((1−rho)Σw² + rho(Σw)²,0))`. This O(n) equicorrelation identity replaces explicit pairwise summation and is the entire FX delta charge.
4. Repeated identical ordered pairs net through map accumulation; reversing a pair creates another factor rather than negating or transforming the original.

## Selection and controls

The FX delta branch returns zero for an empty map. Engine validation checks finite amounts and admits arbitrary ordered currency pairs, including self-pairs and pairs unrelated to `base_currency`. The runtime applies the fixed standard weight.

## Limitations and failure behavior

The current norm consumes pair keys without validating the normalization. Equal economic exposures can occupy different factors and create false diversification. The current result omits the valuation date, currency-conversion provenance and the pair-relief election. The acceptance review verified the published Basel eligibility list. Jurisdictional applicability requires qualification under the elected profile.

## Alternatives and tradeoffs

Canonical reporting-currency mapping makes the offset rule explicit and requires qualified sensitivities on the declared basis. The linear-time norm evaluates the prescribed uniform-correlation quadratic after applying each bucket's weight.

## Evidence and role

The inspected source covers the runtime norm, ordered-pair inputs and fixed parameters. Existing examples cover a single FX factor and scenario differences. Independent arithmetic checked the published pair list and first-order crosses, the inverse shock, both weight scales and mixed-sign charges under all three scenarios. Runtime verification remains pending.
[Runtime norm](../../../../finstack-quant/margin/src/regulatory/frtb/delta.rs) ·
[Input validation](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs) ·
[FX parameters](../../../../finstack-quant/margin/src/regulatory/frtb/params/fx.rs) ·
[Engine examples](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) ·
[FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

Reconciliation and qualification work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
