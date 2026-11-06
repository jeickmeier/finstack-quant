# PORTFOLIO-EXECUTION-016 — Margin membership, sensitivity netting and initial-margin dispatch

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and scope

The portfolio margin adapter groups eligible instruments into netting sets and delegates SIMM, schedule or clearing-house calculations to the margin crate. It applies signed holdings to sensitivities and absolute holdings to standalone collateral requirements. It is not a regulatory certification: product proxies and incomplete extraction remain explicitly relevant.

## Membership and sensitivity stages

Only instruments exposing `as_marginable()` and a netting-set ID are members. Validate each supplied OTC margin specification. Within a set, supplied CSA and clearing-status terms must agree; a set may inherit its first supplied spec. Across sets, the same CSA ID must retain the same counterparty and terms. Instruments without membership remain in `positions_without_margin`.

A default-version SIMM calculator is constructed before method dispatch. Native builds calculate member sensitivities in indexed parallel order; WASM is serial. When an instrument has a spec whose chosen methodology is not SIMM, its contribution is an empty sensitivity set. Otherwise extract product SIMM sensitivities, multiply by signed position scale and convert the calculation currency to portfolio base. Cross-currency rebasing with any nonempty FX-delta map is rejected because scaling alone does not define the required FX calculation-currency remap. Other sensitivities can be uniformly FX-scaled. Successful sets merge in member order; extraction/conversion failures become degraded positions and are omitted from sensitivity aggregation.

## Initial-margin selection and formulas

A supplied spec with no IM parameters produces zero IM. Otherwise use its selected methodology, falling back to ClearingHouse for cleared sets and SIMM for bilateral sets without a method. Unsupported methodologies error.

- **SIMM:** calculate from aggregated sensitivities using the CSA MPOR if present, otherwise the calculator default. Missing aggregate sensitivities produce zero IM. The margin crate owns bucket/correlation/concentration formulas; the portfolio does not reconstruct them.
- **Schedule:** require regulatory exposure notional and contractual expiry. Expired positions are skipped. Remaining years are calendar days/365. Supported classification is IRS and fixed-income-index TRS→interest rates; CDS/CDS index→credit; equity TRS→equity. Other products error. Use absolute position scale on regulatory notional, signed scaled MTM, base-currency conversion, and the margin crate's net-to-gross-ratio schedule calculation. Rescale its result by `sqrt(target MPOR/source MPOR)`; zero source MPOR errors.
- **Clearing house:** choose the CCP proxy by netting-set counterparty. Calculate each standalone requirement, multiply by absolute scale and `sqrt(target/source MPOR)`, spot-convert and add. No portfolio offsets are granted. Calculation/conversion failures are degraded and omitted; it is not the same failure policy as schedule IM.

## Failure, approximation and aggregation limits

SIMM extraction failures can leave a partially populated or zero IM while the call succeeds with degradation metadata. Schedule input/calculator errors abort; CCP per-position failures can yield a partial total. VM counts positions through its separate MTM pass, so `position_count` is not necessarily the count of complete IM contributions. `positions_without_margin` counts untracked or degraded positions, even if a partial number exists. The first degradation message per position is retained, potentially hiding additional failure reasons.

`is_approximate` is set for cleared sets or when a SIMM breakdown is present. A false flag on schedule/zero-IM output is not evidence of exact regulatory compliance. SIMM model/version, schedule classification and CCP-proxy adoption require the [margin methodology scope](../margin-collateral-im/README.md); current external rule applicability was not researched here.

Netting-set traversal and several monetary accumulations use hash-map iteration and plain f64 addition. Those mixed-sign/dynamic-range and deterministic-order choices differ from [existing invariants](../../../../INVARIANTS.md#12-arithmetic-and-aggregation); the observation is not an approved exception. No numerical impact was reproduced. A strict all-or-error margin policy and richer per-position completeness report are possible alternatives requiring explicit behavior design.

## Evidence

[aggregator.rs](../../../../finstack-quant/portfolio/src/margin/aggregator.rs), [netting_set.rs](../../../../finstack-quant/portfolio/src/margin/netting_set.rs) and [results.rs](../../../../finstack-quant/portfolio/src/margin/results.rs). [Margin aggregation tests](../../../../finstack-quant/portfolio/tests/margin_aggregation.rs) use instrument probes for quantity, sign, FX and total reconciliation; [bilateral IRS test](../../../../finstack-quant/portfolio/tests/margin_bilateral_irs.rs) uses a product path. These are bridge/invariant evidence. [ISDA SIMM](../../../REFERENCES.md#isda-simm) and [BCBS/IOSCO](../../../REFERENCES.md#bcbs-iosco-uncleared-margin) are unversioned conceptual references here, not proof of current compliance.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
