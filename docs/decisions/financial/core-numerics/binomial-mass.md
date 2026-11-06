# CORE-NUMERICS-13: Full binomial probability mass by log recurrence

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/distributions.rs::binomial_pmf_all[_into]`: all count probabilities k=0..n for homogeneous independent Bernoulli trials. This function does not supply heterogeneous default probabilities or dependence.

## Choice and rationale

A logarithmic recurrence avoids direct binomial coefficients and factorial overflow. All probabilities are materialized to support portfolio count distributions. Independence and common p are caller assumptions.

## Algorithm

1. Reject NaN p before clearing the caller's scratch vector. Resize to n+1 zeros.
2. For p≤0 set P(0)=1; for p≥1 set P(n)=1. This includes infinities and finite out-of-range p.
3. Otherwise start `log P(0)=n log(1−p)` and `log_ratio=log(p)−log(1−p)`. Advance `log P(k+1)=log P(k)+log(n−k)−log(k+1)+log_ratio`, exponentiating each entry.
4. Sum mass with ordinary f64 addition. If mass>0 and |mass−1|>1e-10, divide every entry by mass. Return the vector without a separate mass-error result.

## Selection, controls and conventions

The allocating wrapper and scratch-buffer variant share exactly the same kernel. n is an integer trial count; p is a decimal probability. The normalization trigger is fixed, not caller-configurable.

## Limitations, failure and fallback

Out-of-range p is silently saturated to a degenerate distribution. Memory is O(n); n+1 allocation has no public resource cap. Underflow can erase tiny probabilities; normalization does not recover their relative values. No approximation such as Poisson or normal is selected for large n. The returned result is not a correlated credit portfolio distribution.

## Alternatives and tradeoffs

Direct probability recurrences or a mode-centered recurrence are possible alternative implementations; they are not selected here. Poisson-binomial convolution and copula integration answer different assumptions and belong to their product records.

## Evidence

- [Implementation and inline tests](../../../../finstack-quant/core/src/math/distributions.rs).
- `test_binomial_pmf_all` checks small hand values. `binomial_pmf_all_matches_statrs` and extreme-probability cases compare a separate dependency implementation; `binomial_pmf_all_large_n_keeps_probability_mass` checks normalization and mean/variance. These are numerical/invariant checks, not empirical validation of homogeneous independence.
- [Monte Carlo/probability context](../../../REFERENCES.md#glasserman-2004-monte-carlo); exact log-recurrence adoption provenance is not separately recorded.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
