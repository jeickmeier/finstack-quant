# FEATURES-022: Inclusive trailing median and MAD Hampel replacement

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

`HampelFilter` uses positive window/min_periods (defaults 1/window) and finite threshold>=0 (default 3). It is a causal inclusive trailing-row filter: the current observation participates in the median and MAD. A finite current value and at least min_periods finite window values are required. It does not use future rows, a centered calendar window or a separate training sample.

## Ordered computation and boundaries

1. Sort finite window values and compute the Type-7 median c. Find scale a=max(abs(sample)); if a<=0 return the current value directly.
2. Form normalized absolute deviations `abs(x/a-c/a)`, sort them and calculate their Type-7 median MAD. Multiply by `1.482602218505602`, the reciprocal of the shared normal third-quartile constant.
3. Compare current normalized deviation with `threshold*scaled_MAD`. Replace current with raw median only when deviation is strictly greater; equality preserves current. Otherwise return current unchanged.

A zero MAD makes every nonzero current deviation an outlier, so a spike against a flat majority is replaced. This differs from robust z-score's all-zero output when MAD=0. Threshold=0 replaces any nonzero deviation when the computed scale is meaningful. A very large finite threshold can make the comparison bound infinite; the implementation does not check that internal bound and then no finite deviation exceeds it. Output remains in input units and is checked for finiteness by the caller.

Missing current rows stay None; the method never imputes them. Repeated application recomputes the sample median/MAD and can change other replacements, so no idempotence or preservation of economic events is promised. Normal-consistency scaling is a convention, not an assertion of Gaussian observed data.

## Rationale, alternatives and evidence

The source names a Hampel filter and supplies normal-consistency constants; it does not record independent provenance for the selected window direction, threshold or zero-MAD choice. Proposed rationale is limiting isolated signal spikes with robust local estimates. Prior-only or symmetric windows, a zero-MAD floor and explicit no-filter decisions on flat windows would differ; none is active.

- Owning [advanced helper](../../../../finstack-quant/features/src/timeseries/advanced.rs) `hampel_value`; [parse/count controls](../../../../finstack-quant/features/src/timeseries.rs); [normal constants](../../../../finstack-quant/features/src/types.rs).
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs) replace the spike in [1,2,3,100,4] using three-row windows; [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs), `hampel_replaces_a_spike_in_a_flat_history_at_any_scale`, checks [a,a,100a] for a=1e-150,1,1e150. Eager negative-threshold rejection is also inspected.

No filter experiment was run.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
