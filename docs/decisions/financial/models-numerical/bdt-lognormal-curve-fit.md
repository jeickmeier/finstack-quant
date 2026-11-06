# MODELS-NUMERICAL-005: BDT lognormal lattice with clamped node rates

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and domain

[bdt.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/bdt.rs) represents r[n,j]=α_n exp((n−2j)σ√dt) with equal binary transition probabilities. Volatility is relative lognormal volatility, unlike Ho–Lee's absolute rate volatility. N>0, T>0, finite nonnegative σ and zero mean reversion are required by [tree.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/tree.rs). Negative-rate targets are not represented by this strictly positive construction.

## Algorithm and controls

Initialize a root guess from −ln(P(dt))/dt. For each row solve ΣQ_j df(clamp(α u^(n−2j),1e−8,5),dt)=P(t_next) using default core Brent; initial guess is the root estimate or geometric mean of the already populated row. Clamp the solved α and each resulting rate to [1e−8,5]. Propagate discounted state prices with equal probabilities and populate the next row, including a synthetic terminal row using the last α. Calibration replaces the existing tree only on success.

Track maximum relative DF error ×10000 and the first rate clamp changing a node by more than 1e−6 relative. Fail on solver error or maximum fit error above configured tolerance (default 0.1 relative-DF bp). Material clamp engagement alone does not fail: an accurate curve fit can retain clamps in low-weight tails. A warning threshold of 1 bp is separate from the hard gate.

## Selection and alternatives

The [bond dispatcher](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs) selects BDT explicitly, threads compounding and invokes an extra quality check. BK is a separate supported model for lognormal mean reversion; Ho–Lee/HW allow negative rates. Exact ZCB repricing cannot establish tail-option fidelity when rates are clamped. `error_bp > max_error_bp` is used without a per-row nonfinite check; a NaN row error would not update the finite maximum. This is a static guard limitation, not a reproduced pricing failure.

## Evidence

[short-rate tests](../../../../finstack-quant/models/src/trees/short_rate_tree/tests.rs) include `test_bdt_stored_lattice_prices_zero_coupon_to_calibration_curve`, `bdt_calibration_fails_when_node_rate_clamp_engages_materially`, near-zero/high-rate cases, and `bdt_solver_failure_is_immediate_and_preserves_previous_calibration`. These test roundtrip fit and guard behavior. [Brent](../../../REFERENCES.md#brent-1973) covers the solver family; [Hull](../../../REFERENCES.md#hull-options-futures) is general background, not evidence approving [1e−8,5].

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
