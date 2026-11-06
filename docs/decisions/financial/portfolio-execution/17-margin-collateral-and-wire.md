# PORTFOLIO-EXECUTION-017 — VM perspective, CSA collateral terms and margin result validation

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and scope

The adapter reports initial margin and signed variation-margin calls in portfolio base currency, with a collateral-outflow total computed separately from net signed VM. It applies CSA IM account terms once to grouped gross IM, while VM is calculated per netting set. The portfolio API accepts current IM collateral by CSA but no current VM collateral input.

## VM and collateral algorithm

Obtain each member instrument's VM MTM, multiply by signed position scale, and spot-convert into base currency. A `Notional(Some(currency))` mismatch is a position error here, unlike the warning-only ordinary scale path. MTM/FX failures are logged as degraded and excluded from the netting-set sum; successful MTM entries determine `position_count`.

For a netting set with a CSA, convert gross signed MTM to CSA base currency and invoke `VmCalculator` with **zero current VM collateral**, then convert its signed net margin back to portfolio currency. The margin crate owns threshold, minimum-transfer and perspective logic. Without a spec, return negative gross MTM; bilateral positions are explicitly degraded because no threshold/MTA/haircut terms were applied. Repo instruments can reach this path: their repo haircut specification is not consumed by this OTC adapter. Cleared no-spec paths do not receive that bilateral degradation marker.

For IM collateral, collect netting-set IM by CSA only where IM parameters exist. Convert each gross amount to CSA currency and sum, then reject any supplied current-collateral CSA absent from that collection. Missing current IM collateral defaults to zero in CSA currency. Apply the CSA's own collateral terms to gross/current balances. Convert required collateral and transfer back to portfolio currency; separately add required balances whose account is segregated. These account fields are distinct from gross model IM.

Per-set `total_margin = IM + max(VM,0)`. Portfolio total is the sum of those per-set totals, while `total_variation_margin` separately sums signed VM. It is therefore **not** `total_IM + max(sum(VM),0)`: a receipt at one counterparty does not cancel another set's positive outflow in that field. Result construction checks IM/VM currency equality; it does not require nonnegative IM at the standalone constructor boundary.

## Wire validation and ordering

Serialization sorts netting sets by display ID and emits CSA/breakdown maps as ordered maps. Deserialization checks matching set currencies and the above per-set total identity using absolute tolerance `1e-9`; portfolio Money currencies must equal declared base. It rejects duplicate netting IDs, checks set sums against all three portfolio margin totals and verifies summed position count.

Each IM account must reference at least one result set with that CSA. Gross/required/current balances must be finite, nonnegative and in one currency. Transfer must be finite in that currency and either zero or equal required minus current within tolerance; required cannot exceed gross. Portfolio collateral totals are reconciled to accounts **only when all account currencies equal the portfolio base**. Cross-currency accounts lack an FX proof in this payload, so no corresponding sum check is made.

The wire adapters do not check all nested consistency: set dates against portfolio date, breakdown currency/sum, sensitivity calculation currency, every degradation ID/count relationship or model/version provenance. Their helper wire structs do not deny unknown fields. Public typed results can also be mutated and serialized without re-running deserialize validation. Thus accepted JSON means the implemented subset of checks passed, not that a margin run was replayed or certified.

## Alternatives and limitations

Actual current VM collateral is needed for a live incremental call; the present zero-balance calculation is an observed scope limit. A strict completeness policy, repo-specific collateral bridge and validated result constructors are possible changes, not existing capabilities. Plain f64/hash-map reductions differ from the [required accumulation policy](../../../../INVARIANTS.md#12-arithmetic-and-aggregation). The `1e-9` wire tolerance is absolute, and canonical wire order differs from runtime map traversal. No bad collateral transfer or round-trip failure was numerically reproduced.

## Evidence

[aggregator.rs](../../../../finstack-quant/portfolio/src/margin/aggregator.rs), [results.rs](../../../../finstack-quant/portfolio/src/margin/results.rs) and [wire.rs](../../../../finstack-quant/portfolio/src/margin/wire.rs). Inspected [margin aggregation tests](../../../../finstack-quant/portfolio/tests/margin_aggregation.rs) distinguish net signed VM from positive outflow totals. [Serialization tests](../../../../finstack-quant/portfolio/tests/margin_serialization.rs) check mixed currencies, inconsistent totals, duplicate IDs and sensitivity bucket round trips. These are algebraic/contract evidence. Underlying VM/IM formulas and reference-version qualification belong to the [margin scope](../margin-collateral-im/README.md); [BCBS/IOSCO](../../../REFERENCES.md#bcbs-iosco-uncleared-margin) is conceptual context only.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
