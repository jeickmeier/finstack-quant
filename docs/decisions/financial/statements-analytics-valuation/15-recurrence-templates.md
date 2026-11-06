# STATEMENTS-ANALYTICS-VALUATION-15: Roll-forward formulas and vintage convolution templates

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed in the defined source scope linked below; discrepancies and limits are explicit.
- **Validation status:** Evidence inspected or located, not executed. No numerical qualification or external reconciliation is inferred.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Choice, domain and selection

[Roll-forward](../../../../finstack-quant/statements-analytics/src/templates/roll_forward/mod.rs) and [vintage](../../../../finstack-quant/statements-analytics/src/templates/vintage/mod.rs) helpers generate statement DSL formulas. They do not execute a separate numerical engine. Formula parsing, missing-data semantics and typed-unit inference occur later in the statement builder/evaluator. The helpers do not validate input names, collisions or all numeric inputs before embedding them.

## Algorithms and controls

For a named roll-forward, create `<name>_beg = coalesce(lag(<name>_end,1),opening)` and `<name>_end = <name>_beg + sum(increases) − sum(decreases)`. Opening defaults to zero or is explicitly supplied. Missing first-period history therefore starts from opening; later missing history also follows the DSL's coalesce semantics. Changes are summed in supplied order, without sign conversion or balance-specific restrictions.

For a vintage buildup, coefficient `d[k]` contributes `new_volume × d[0]` for age zero and `coalesce(lag(new_volume,k),0) × d[k]` for older ages. Sum all terms; an empty/all-skipped curve becomes literal zero. Coefficients with `abs(d[k])<1e−10` are omitted. There is no probability normalization, monotonicity, nonnegativity or sum-to-one requirement, so the same template supports a general finite impulse-response convolution rather than only survival/decay curves.

The common [float formatter](../../../../finstack-quant/statements-analytics/src/templates/mod.rs) uses shortest-roundtrip formatting and adds `.0` to integer-like literals, preserving small finite coefficients rather than fixed-decimal truncation. Nonfinite opening/coefficients are not prevalidated here and can create invalid or unsupported DSL literals for later parsing. All generated names are inserted into the builder; no template-specific collision guard protects existing nodes.

## Limits, alternatives and evidence

The absolute coefficient cutoff can discard meaningful tails when units/volumes are large. Fixed lag steps follow model periods, not elapsed calendar days or age-specific irregular intervals. Missing history is zero rather than estimated backfill. These choices are explicit projection assumptions; they are not conservation or accounting proofs.

Source formulas and the statement evaluator provide the owning implementation. No dedicated independent template numerical benchmark was located in this scope; shared integration tests exercise generated models but cannot certify every coefficient/unit domain. Explicit typed templates, finite-input validation, no-cutoff convolution, calendar-aged cohorts and validated survival curves are alternatives requiring a separate choice.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
