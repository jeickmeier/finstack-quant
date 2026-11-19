# MARGIN-REGULATORY-XVA-001: FRTB sensitivity units, factor identity, wire reconstruction and provenance

- Decision status: **Accepted 2026-11-17 for checked FRTB sensitivity inputs, factor identity and provenance.**
- Implementation status: Existing behavior is described below. Input basis, factor admission, checked restoration and caller reconciliation remain pending.
- Validation status: Source and primary references inspected. Independent examples checked delta units, equity repo weights, reporting-currency legs and snapshot admission. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked FRTB inputs and provenance

1. **Bind the inputs to one explicit calculation basis.** Elect a supported
   rulebook edition, jurisdictional profile and immutable parameter set.
   Require the valuation date, information cutoff, reporting currency,
   position scope and component coverage. Valuation and classification owners
   supply qualified reprices, buckets and dated FX evidence. The receiving
   engine admits those inputs on their declared basis.
2. **Keep delta amounts and weights on matching scales.** For GIRR, credit
   spread and equity repo, multiply the signed P&L from the prescribed upward
   1 bp reprice by 100 to express it per percentage point. Equity spot,
   commodity and FX use signed P&L from an upward 1% relative move. Pair these
   amounts with percentage-point weight numbers. A signed 120 per bp gives
   12,000 on this basis. A 1.7% weight then gives 20,400 of weighted sensitivity.
   Equity repo uses its own smaller weight.
   [Basel MAR21.19–24 and Table 10](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Require prepared vega and curvature on their stated bases.** Vega is signed
   `σ × ∂PV/∂σ` before the decimal vega weight, with matching volatility and
   derivative conventions. The Basel reference permits normal or lognormal
   inputs for GIRR and credit spread and uses lognormal for equity, commodity
   and FX. Curvature carries both signed loss-positive P&L amounts after
   delta removal under the prescribed full shocks. Those amounts already
   reflect the shock size.
   [Basel MAR21.5 and MAR21.25–28](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
4. **Preserve full factor identity and risk-specific tenor grids.** Require
   applicable curve, currency, tenor, issuer or tranche, credit basis,
   commodity location, equity spot or repo identity, and option maturity.
   Reject conflicting classifications for the same factor and require an
   explicit residual assignment where applicable. Keep the ten ordinary GIRR
   delta vertices `0.25Y`, `0.5Y`, `1Y`, `2Y`, `3Y`, `5Y`, `10Y`, `15Y`, `20Y`
   and `30Y`. Ordinary GIRR vega uses `0.5Y`, `1Y`, `3Y`, `5Y` and `10Y` on
   each applicable maturity axis. Missing required coordinates make the
   affected result unavailable.
   [Basel MAR21.8–14](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
5. **Use the direct reporting-currency basis for FX.** Define an upward FX
   move as more reporting currency per unit of risk currency. Require both
   reporting-currency legs for cross-FX exposures where applicable, including
   translation risk, and reject self-pairs. Retain the qualified option pair,
   quote convention and expiry for vega. A reciprocal quote or alternative
   base-currency method requires an explicitly qualified transformation or
   reprice.
   [Basel MAR21.14 and MAR21.24](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
6. **Check coverage before reporting zero or complete capital.** Keep DRC and
   residual-risk inputs as separate positions with explicit amount roles and
   classifications. Required securitisation DRC is unavailable until a
   qualified route exists. Distinguish evaluated zero, qualified inapplicability
   and missing risk. Incomplete scope can support explicitly labelled
   diagnostics. [Record 002](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation) owns
   aggregation and diagnostic assembly.
7. **Use the accepted wire rules at every entry point.** Sum qualified
   contributions by the full matching key with checked accumulation in the
   declared order. Reject duplicate snapshot keys. Validate complete changes
   before committing them through Rust, JSON, tables, Python or WASM. Stamp
   results with the actual profile, parameter identity, currency, cut,
   coverage and source history.
   [Margin wire contract](../margin-collateral-im/22-wire-tables-and-provenance.md#accepted-decision-for-checked-margin-tables-wire-state-and-provenance).

## Choice and domain

The observations below describe the audited design. The accepted contract binds the reporting currency and sensitivity scale to the qualified factor and position scope.

The FRTB engine consumes already-classified, already-converted sensitivities in one reporting currency. It does not extract instrument risk, choose regulatory buckets, compute curvature shocks, perform FX conversion, or establish portfolio completeness. `base_currency` labels the input container; the charge arithmetic does not use it to convert values. The recorded D457/MAR21 provenance is source metadata, not a new verification of a rulebook or jurisdiction.

## Algorithm and conventions

1. GIRR and CSR delta inputs are monetary P&L per one percentage-point rate/spread shift (100 times per-bp DV01/CS01); equity/commodity/FX deltas are P&L per one percent price move. Published-number delta weights multiply these directly. Equity repo uses P&L per percentage-point repo-rate shift. Vega inputs are already `sigma × dV/dsigma`, with decimal vega weights. Curvature is the signed loss-positive, delta-hedged `CVR_up/CVR_down` under the prescribed full shock; no risk weight is subsequently applied to CVR.
2. Full factor keys net matching exposures: GIRR currency/tenor; GIRR vega currency/expiry/underlying-tenor; CSR delta name/bucket/tenor/basis; commodity delta name/bucket/tenor/location; equity separate spot/repo maps; FX ordered currency pairs, with expiry for vega. `add_*` methods sum scalars and each curvature side; DRC/RRAO append positions.
3. Engine validation requires finite numeric inputs, recognized tenor strings, allowed bucket numbers, nonblank name/basis/location/position identifiers, valid DRC positions, and finite RRAO notionals. Allowed tenor strings are exactly `0.25Y, 0.5Y, 1Y, 2Y, 3Y, 5Y, 10Y, 15Y, 20Y, 30Y`; the same lookup validates all delta and vega tenors.
4. JSON uses arrays of factor tuples, sorts map entries lexicographically when serializing, and rejects unknown top-level fields. Reconstruction uses `insert`/`extend`: **the last duplicate tuple key wins**, unlike `add_*` accumulation. DRC/RRAO vector order and repeated identifiers are retained. Deserialization itself does not call financial validation; `calculate` does.
5. Risk weights/correlations come from fixed Rust tables/functions; they are not the margin registry JSON and have no runtime version selector. Source comments record D457/MAR21 and a 2026-08-20 review; equity/commodity comments cite a 2024 publication path. The audit preserves those provenance assertions as assertions. The output stamps default `ResultsMeta`, not the effective regulatory version, jurisdiction, valuation date or sensitivity-source history.

## Selection and controls

`FrtbSensitivities::new` starts all maps/vectors empty. Missing factors therefore contribute zero. A sensitivity container is mutable; mutators defer validation to the engine. Unknown public weight lookups return NaN, while engine validation is intended to reject unsupported buckets before private charge stages. A valid JSON shape or schema match is not financial validity.

## Limitations and failure behavior

Ordered FX pairs are not inverted/canonicalized, same-currency FX pairs are not rejected, and one issuer/name can be assigned multiple risk buckets without a global consistency check. The ten-tenor GIRR lookup is also the accepted vega grid; no remapping to another regulatory grid occurs. No currency/scale completeness check can infer whether a caller supplied raw vega or DV01 incorrectly. The deterministic JSON test proves serialization order, not floating-point charge invariance under arbitrary insertion order.

## Alternatives and tradeoffs

The accepted contract separates additive contributions from unique-key snapshots. Typed coordinates keep distinct curves and qualified classifications visible before netting. Valuation and classification owners prepare the input risk.

## Evidence and role

Runtime and domain contracts: [src/regulatory/frtb/types.rs](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs), [src/regulatory/frtb/wire.rs](../../../../finstack-quant/margin/src/regulatory/frtb/wire.rs), [src/regulatory/frtb/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/mod.rs). Fixed parameters: [src/regulatory/frtb/params/mod.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs); historical provenance narrative: [data/margin/README.md](../../../../finstack-quant/margin/data/margin/README.md).

[tests/regulatory_determinism.rs](../../../../finstack-quant/margin/tests/regulatory_determinism.rs) contains inspected sorted-JSON/roundtrip and wrong-shape assertions; these are transport evidence, not independent capital calculations. [src/regulatory/frtb/engine.rs](../../../../finstack-quant/margin/src/regulatory/frtb/engine.rs) locates unknown-label/bucket/nonfinite rejection tests. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements) supplies the existing canonical pointer; the external standard was not re-certified in this audit.

Implementation reconciliation and independent qualification are tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
