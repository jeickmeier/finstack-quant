# MARGIN-COLLATERAL-IM-013 — Schedule IM net-to-gross reduction

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: calculate each trade's schedule IM before applying one net-to-gross adjustment across the supplied set. Proposed rationale: preserve heterogenous asset-class/maturity rates while recognizing directional MTM offset.

## Input domain and conventions

Rows are `(signed_mtm, notional, class, maturity_years)` in one currency. Amounts must be finite and maturity nonnegative. The caller must supply an economically/legal valid netting set; the calculator does not inspect agreements or counterparty IDs.

## Ordered algorithm

1. Empty input returns None. Use the first MTM currency; reject any row whose MTM or notional differs.
2. Sum `abs(notional)*trade_rate` by class and gross absolute notional across rows. If gross notional equals zero, return None even if MTM is nonzero.
3. Use Neumaier sums for total signed MTM V and positive MTM P. If P=0, NGR=0; otherwise `NGR=clamp(max(V,0)/P,0,1)`.
4. Multiply every class gross subtotal by `0.4+0.6*NGR`. Fully directional positive MTM gives100% gross IM; fully offset or all-negative MTM gives40%.
5. Store `<class>_ngr` components and Neumaier-sum their monetary amounts for total. Gross-by-class and notional accumulation themselves use ordinary sums. MPOR is not rescaled here.

## Selection, controls and limitations

The same reduction is applied across all included classes. Notional signs do not offset gross IM; MTM signs determine NGR. There is no instrument-product or regulatory offset partition beyond the rows the caller provides. Missing rate cells use the single-trade schedule fallback.

## Alternatives and tradeoffs

Separate netting partitions may be required by a chosen contractual/regulatory interpretation; an unnetted schedule intentionally keeps gross IM. Sensitivity netting is a separate methodology and must not be mixed into this formula as a convenience.

## Source and validation evidence

Inspected unit tests cover full/partial/no offset, all-negative MTM, empty/mixed currency, and matching constructors. Independent hand regression uses 1m IR at 1% and 1m equity at 15%, offsetting±100 MTM, for `(10k+150k)*0.4` total.

Implementation and data: [src/calculators/im/schedule.rs](../../../../finstack-quant/margin/src/calculators/im/schedule.rs), [data/margin/schedule_im.v1.json](../../../../finstack-quant/margin/data/margin/schedule_im.v1.json).

Existing evidence: [src/calculators/im/schedule.rs](../../../../finstack-quant/margin/src/calculators/im/schedule.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
