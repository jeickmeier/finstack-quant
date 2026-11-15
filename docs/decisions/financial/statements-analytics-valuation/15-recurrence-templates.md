# STATEMENTS-ANALYTICS-VALUATION-15: Roll-forward formulas and vintage convolution templates

- **Decision status:** Accepted for the full roll-forward and vintage template contract (2026-11-15).
- **Implementation status:** Reconciliation pending.
- **Validation status:** Existing evidence mapped. Independent qualification pending.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Accepted decision for roll-forward and vintage templates

1. Use Statements templates and the standard Statements evaluator. Validate identifiers, references, units and numeric inputs before inserting any node. Reject output collisions and duplicate or overlapping movements. Apply the same validation in Rust, Python and WASM.
2. Require an explicit typed opening for the first covered period, including an explicit zero when applicable. Use the immediately preceding close thereafter. Apply signed movements as entered. For magnitude buckets, add nonnegative increases and subtract nonnegative decreases. A missing or masked prior close or required movement makes the affected result unavailable.
3. For vintage calculations, sum contributions from new volumes by model-period age with a nonempty curve of finite, potentially signed coefficients. Retain every nonzero coefficient at the supplied precision. Skip exact-zero coefficients and treat ages outside the curve as zero. An all-zero curve yields a typed zero in the source unit. Use checked arithmetic under the accepted monetary rules. [Formula units and currency](../statements-contracts/05-currency-dimensions.md#accepted-decision-for-formula-units-and-currency).
4. Treat pre-inception volumes as zero only with an explicit declaration that no earlier cohorts exist. Otherwise, require earlier observations on the same model calendar for nonzero coefficients. A missing or masked required observation makes the result unavailable and identifies the affected source period.

For example, one trillion units of Q1 new volume multiplied by an age-one coefficient of `5e-11` contributes 50 units to Q2. If the required Q1 observation is missing, the Q2 result is unavailable and identifies Q1.

## Audited choice, domain and selection

[Roll-forward](../../../../finstack-quant/statements-analytics/src/templates/roll_forward/mod.rs) and [vintage](../../../../finstack-quant/statements-analytics/src/templates/vintage/mod.rs) helpers generate statement DSL formulas. They do not execute a separate numerical engine. Formula parsing, missing-data semantics and typed-unit inference occur later in the statement builder/evaluator. The helpers do not validate input names, collisions or all numeric inputs before embedding them.

## Audited algorithms and controls

For a named roll-forward, create `<name>_beg = coalesce(lag(<name>_end,1),opening)` and `<name>_end = <name>_beg + sum(increases) − sum(decreases)`. Opening defaults to zero or is explicitly supplied. Missing first-period history therefore starts from opening; later missing history also follows the DSL's coalesce semantics. Changes are summed in supplied order, without sign conversion or balance-specific restrictions.

For a vintage buildup, coefficient `d[k]` contributes `new_volume × d[0]` for age zero and `coalesce(lag(new_volume,k),0) × d[k]` for older ages. Sum all terms; an empty/all-skipped curve becomes literal zero. Coefficients with `abs(d[k])<1e−10` are omitted. There is no probability normalization, monotonicity, nonnegativity or sum-to-one requirement, so the same template supports a general finite impulse-response convolution rather than only survival/decay curves.

The common [float formatter](../../../../finstack-quant/statements-analytics/src/templates/mod.rs) uses shortest-roundtrip formatting and adds `.0` to integer-like literals, preserving small finite coefficients rather than fixed-decimal truncation. Nonfinite opening/coefficients are not prevalidated here and can create invalid or unsupported DSL literals for later parsing. All generated names are inserted into the builder; no template-specific collision guard protects existing nodes.

## Audited limits, alternatives and evidence

The absolute coefficient cutoff can discard meaningful tails when units/volumes are large. Fixed lag steps follow model periods, not elapsed calendar days or age-specific irregular intervals. Missing history is zero rather than estimated backfill. These choices are explicit projection assumptions; they are not conservation or accounting proofs.

Source formulas and the statement evaluator provide the owning implementation. No dedicated independent template numerical benchmark was located in this scope; shared integration tests exercise generated models but cannot certify every coefficient/unit domain. Explicit typed templates, finite-input validation, no-cutoff convolution, calendar-aged cohorts and validated survival curves are alternatives requiring a separate choice.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
