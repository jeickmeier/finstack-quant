# MODELS-AND-VALUATIONS-CREDIT-19: Tranche spread, quote, rate, correlation, recovery and default risk

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Tranche risk mixes analytic unit-coupon calculations, quote-space rebootstrap and direct payoff loss. The selected pricing model is not propagated uniformly to all specialized metrics.

## Scope and input domain

Live CDS tranches and CreditIndexData, with optional issuer-specific curves. Quote CS01 requires replayable hazard calibration and a provider. Recovery/rate calculations conditionally replay available calibration.

## Algorithm

1. Spread DV01 prices premium rows at one basis point. Par spread divides minus constant protection/upfront PV by premium PV per bp, clips to [0,100000] bp and requires residual below 1e−6 × notional; negligible annuity returns zero.
2. CS01 gathers distinct active issuer hazard IDs if issuer curves exist, otherwise the index hazard. It rebuilds all quote bumps together for total risk and individually for components, replacing hazard references inside CreditIndexData. Bucketed CS01 delegates quote-node series and reinserts each rebuilt index.
3. DV01 bumps discount curves and replays eligible active hazards before dispatch repricing. Correlation01 shifts all base-correlation knots ± the configured correlation bump, clips knots to [0,1], and returns the central PV difference scaled to that bump.
4. Recovery01 bumps index-level recovery within bounds, reboots only the index hazard if it has calibration metadata, otherwise freezes hazard; scales the PV span to the standard recovery bump.
5. Jump-to-default averages the marginal loss increase from one constituent default, using issuer weights/recoveries or equal names. It returns a positive loss amount without subtracting current PV or applying position sign.

## Selection and controls

Quote/rate risk uses resolved sensitivity configuration and dispatch repricing. Spread, par, EL, correlation and JTD metric calculators instantiate `CdsTranchePricer::new()` (Gaussian). Recovery01 calls instrument.value, also defaulting to its base pricer. Bucketed DV01 uses the framework’s triangular key-rate calculator.

## Conventions

CS01/DV01 are PV change per basis point; correlation risk uses an absolute correlation bump; recovery risk uses absolute recovery fraction. Spread DV01 is coupon-annuity sensitivity, distinct from quote CS01.

## Limitations, failure and fallback

Heterogeneous Recovery01 updates neither issuer-specific recovery overrides nor issuer hazards, so its economic scope differs from active-hazard CS01/DV01. Boundary-clipped correlation bumps retain the central denominator. Custom-copula valuation can therefore be paired with Gaussian specialized risk outputs.

## Alternatives and tradeoffs

Consistent selected-pricer dispatch and per-name recovery replay are possible alternatives; frozen-hazard recovery and market-quote-preserving recovery are different risk definitions and should be named deliberately.

## Evidence

Owners: [sensitivities.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/sensitivities.rs), [metric implementations](../../../../finstack-quant/valuations/credit/src/cds_tranche/metrics). [Tranche tests](../../../../finstack-quant/valuations/credit/tests/cds_tranche) locate finite-difference/repricing and sign evidence; these do not establish independent risk accuracy.

## Relationship to the accepted base-correlation boundary

[CORE-MARKET-18's complete accepted package](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks)
requires unclipped admitted source shocks and truthful actual action widths.
Correlation01 reports the PV effect per 0.01 absolute correlation, separately
from detachment/spread bp. The risk owner selects a supported central/reduced/
one-sided boundary scheme or refuses the request, without clipped nodes and a
nominal central denominator. Base/bumped valuation must use the same qualified
financial evaluator; default Gaussian metric substitution is not custom-model
risk parity. Record 18 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's complete
correlation/quote/recovery/default-risk methods or accuracy budgets.

## Relationship to the accepted credit-pool boundary

[CORE-MARKET-19's complete accepted package](../core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts)
requires actual active dependency selection, including named-loss hazards and
any consumed aggregate timing hazard. Shared CurveIds can be shocked/replayed
once while preserving separate issuer events and updating every active reference.
Zero-weight roster entries do not imply active exposure for every metric. Future
cash-recovery risk must target actual pool/member cash inputs and preserve the
separate historical-state assumption, rather than bump an unused common field
or rewrite observed history. Record 19 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this
record's JTD averaging, quote replay, recovery/rate/credit-risk methods or budgets.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
