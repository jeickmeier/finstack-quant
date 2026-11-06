# VALUATIONS-FX-AND-EXOTICS-NDF-FIXING-SETTLEMENT: NDF quote direction, fixing transition and settlement value

- Record: `VALUATIONS-FX-AND-EXOTICS-NDF-FIXING-SETTLEMENT`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

A long-base NDF is represented by its base-currency notional and settled in the convertible currency. Both base-per-settlement and settlement-per-base quote conventions map to the same economic exposure.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Convert rates to settlement-per-base: `g(x)=1/x` for base-per-settlement quotes and `g(x)=x` otherwise. Settlement cash is `N[g(fixing)-g(contract)]`, discounted to maturity on the settlement curve. Missing contract rate gives zero at-market settlement after the fixing/projection requirements have been resolved.

Before fixing, use the selected spot quote direction and `F=S D_settlement/D_base` for base-per-settlement, reciprocal DF ratio for settlement-per-base. The implementation projects to **settlement maturity**, not the earlier fixing date. If a direct matrix query fails, the reverse quote is queried and inverted. After fixing date, missing observed fixing errors; on fixing date projection is still allowed. The settlement remains live on maturity and becomes zero afterward.

Trade construction rolls spot by the pair-specific FX calendar convention, adds tenor, then steps fixing backward over joint business days and applies Preceding. Quote FX01 bumps economic settlement-per-base exposure, so an inverse quoted rate is divided by `1+b` instead of multiplied; observed fixing is not bumped. Parallel/key-rate DV01 delegates to the shared registry.

## Selection and controls

The default discounting implementation prioritizes observed fixing, then (only before or on fixing date) a projected rate. Projection prioritizes quoted forward, then quoted spot/FX matrix plus both discount curves. Missing foreign curve is rejected unless quoted forward is present.

Notional must be positive and in base currency; currencies differ; fixing cannot follow maturity. Supplied rates must be finite and greater than the local minimum positive rate. The optional fixing source is checked against its typical currency. Settlement schedule carries one projected net cashflow and no residual after maturity.

## Limitations, failure and fallback

Pre-fixing projection uses settlement rather than fixing maturity. Inverse quotation makes substituting the reciprocal of a projected rate a different operation from modeling a stochastic reciprocal fixing; no explicit convexity adjustment is implemented. Observed fixing input may determine settlement even before its scheduled date.

## Alternatives and tradeoffs

Possible alternatives are fixing-date projection followed by settlement discounting and an explicit settlement-measure payoff model. Neither is a silent replacement authorized by this audit. Mandatory trade terms would avoid the missing-strike template convention.

## Evidence

- [finstack-quant/valuations/fx/src/ndf/metrics/mod.rs](../../../../finstack-quant/valuations/fx/src/ndf/metrics/mod.rs)
- [finstack-quant/valuations/fx/src/ndf/metrics/tests.rs](../../../../finstack-quant/valuations/fx/src/ndf/metrics/tests.rs)
- [finstack-quant/valuations/fx/src/ndf/types.rs](../../../../finstack-quant/valuations/fx/src/ndf/types.rs)

- [finstack-quant/valuations/fx/src/ndf/metrics/tests.rs](../../../../finstack-quant/valuations/fx/src/ndf/metrics/tests.rs): FX01 quotation-direction and fixing-state regression assertions.
- [finstack-quant/valuations/fx/tests/ndf/test_ndf_types.rs](../../../../finstack-quant/valuations/fx/tests/ndf/test_ndf_types.rs): Fixing-source, date-construction and type/dependency assertions; not an independent fixing-measure benchmark.

Canonical reference index: [Clark FX Options](../../../REFERENCES.md#clark-fx-options); [Wystup FX Options](../../../REFERENCES.md#wystup-fx-options). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
