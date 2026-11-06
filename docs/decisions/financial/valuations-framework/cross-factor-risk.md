# VALUATIONS-FRAMEWORK-12 — Cross-factor finite differences with ordered credit recalibration

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and controls

Cross-factor calculators combine rates, credit spreads, spot, FX and volatility using common factor bumpers. Rates and credit report per bp; spot/FX report per one percentage-point relative move; volatility reports per one volatility point. Thus spot–vol cross gamma differs from absolute-spot vanna by `S/100`. Bump settings follow the shared sensitivity configuration. Pair identifiers describe these units and support the downstream attribution mapping.

## Algorithm

1. Rates collect unique declared discount/forward curve IDs and shock them together. Credit collects every declared hazard and requires a replayable calibration recipe/provider. Spot chooses the first market scalar. FX shocks every declared FX pair. Vol chooses an active instrument quote override, otherwise present surfaces plus unitless scalar volatility dependencies.
2. Central factors use points `(−1,−1/2),(+1,+1/2)`. Volatility reduces its shock to the positive minimum active vol. Exactly zero minimum chooses the forward first-derivative stencil `(0,−3/2),(1,2),(2,−1/2)`; negative/nonfinite minimum fails. The override/scalar boundary here permits zero, unlike generic positive-vol higher Greeks.
3. At each tensor-product corner apply the noncredit factor first, then rebuild credit **from original calibration quotes against the already bumped target market**. This ordering retains rate–credit interaction regardless of the order of factories. Apply any instrument quote shocks, then reprice through selected dispatch.
4. For two central stencils return `(V++−V+−−V−++V−−)/(4hk)`. For a forward stencil use the compensated weighted sum of all corner PVs divided by `hk` in the reporting units above.

## Limitations, failures and alternatives

An absent factor factory or an applicability check returning false yields **zero** cross risk. Rate and FX applicability need only one matching curve/pair; volatility/credit use their own checks. A missing dependency can therefore be indistinguishable from no exposure until later bump/reprice errors. Declared dependency roles and ordering are material input contracts; the framework cannot infer a basket's economically correct scalar from its first entry.

Direct hazard shocks are not a fallback for credit cross gamma. The replay provider is mandatory. The generic stencil does not explicitly seed common random numbers, unlike generic FD option Greeks, so MC variance depends on the product's normal seed behavior. Analytic mixed derivatives, pathwise estimators and single global Hessians are alternatives not selected here.

## Evidence

[cross_factor.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/cross_factor.rs) and [cross_factor_pair.rs](../../../../finstack-quant/valuations/core/src/metrics/cross_factor_pair.rs) own factors, ordering and mappings; inline tests supply behavioral/formula evidence and were not run. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) supplies numerical risk context.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
