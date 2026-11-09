# CORE-NUMERICS-13: Full binomial probability mass

- Decision status: **Accepted — complete full-binomial-mass package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent exact-rational reference cases and read-only formula/host/caller reviews were completed on 2026-11-08; these do not certify production numerical range or a credit model.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/distributions.rs::binomial_pmf_all[_into]`: all count probabilities k=0..n for homogeneous independent Bernoulli trials. This function does not supply heterogeneous default probabilities or dependence.

## Accepted decision: checked full binomial probability mass

Accepted on **2026-11-08** after whole-record review. Runtime,
caller reconciliation and independent numerical/statistical/financial qualification
remain pending; source-observed behavior is retained below.

1. **Keep one full homogeneous binomial law and result shape.** The contract is
   P(K=k)=choose(n,k)*p^k*(1-p)^(n-k), k=0..n, for n common-probability independent
   Bernoulli trials. The supplied p may itself be conditional on an owner's
   factor state; core does not assume the resulting unconditional credit pool is
   independent. Retain `binomial_pmf_all(n,p)->Result<Vec<f64>>` and
   `binomial_pmf_all_into(out,n,p)->Result<()>`, one canonical kernel and ascending
   count order, always exactly n+1 entries. No Option, result/report class,
   individual-cell/log/sparse API, alternate-method selector or distribution state.

2. **Require a strict probability domain before every identity.** Require finite
   p in[0,1], including validation when n0. Reject NaN, infinities and outside
   probabilities rather than saturating them. For admitted p, n0 returns[1]; p0
   places all mass at0 and p1 atn. Negative zero can be treated as0, with zero
   output cells canonical+0. Exact endpoints are model identities, not epsilon
   snaps. All identities remain subject to the count/resource admission below.

3. **Select an explicit dense resource domain.** Require integer
   0<=n<=1,000,000, inclusive, a fixed project resource policy rather than a
   statistical cutoff or portfolio-size recommendation. This bounds the output
   at1,000,001 binary64 cells (8,000,008 raw bytes) and the serial O(n) work.
   Check n+1, vector length/bytes/capacity and fallible reservation before writing,
   including degenerate requests; no huge endpoint bypass or configurable cap.
   Counts used as floating operands are exact under this limit. No recovery from
   every global allocator failure is promised.

4. **Choose the upper mathematical mode for the actual supplied bits.** For
   0<p<1 set m=floor((n+1)*p) using the exact real value represented by binary64
   p. When the product is an integer, m-1 and m tie; choose m deterministically.
   Do not floor a rounded floating product or epsilon-snap a near tie. A private
   significand/exponent decomposition with a checked u128 product and guarded
   shift suffices: under the cap the integer product needs fewer than74 bits.
   This is exact mode selection, not arbitrary-precision probability arithmetic
   or a public numeric framework.

5. **Replace the forward log anchor with mode-centered relative weights.**
   Set w[m]=1. Descend from k=m to1 using
   w[k-1]=w[k]*k*(1-p)/((n-k+1)*p); ascend from k=m to n-1 using
   w[k+1]=w[k]*(n-k)*p/((k+1)*(1-p)). These are the same binomial adjacent ratios;
   the exact-mode choice makes every outward ratio<=1 in real arithmetic, so
   weights stay in[0,1] and their sum Z is in[1,n+1]. Write every entry, including
   zero tails, with fixed finite loops. No factorial/log-Gamma calls, accumulated
   log-P0 drift, Poisson/normal/LHP approximation or alternative runtime path.

6. **Use single-rounded products for safe outward coefficients.** At an active
   right step form numerator RN((n-k)*p) and denominator
   FMA(-p,k+1,k+1), then divide. At an active left step form numerator
   FMA(-p,k,k) and denominator RN((n-k+1)*p), then divide. Here RN is binary64
   multiplication, and FMA has one final rounding of the exact product-plus-sum;
   counts are exactly represented. The products round exact nonnegative ordered
   numerator/denominator quantities once, preserving their <= relation and exact
   ratio1 on genuine ties without clipping. Check positive denominator, finite
   ratio in[0,1] and finite nonnegative updated weight. Never precompute unusable
   odds on an empty side, round 1-p separately before these complement products,
   or repair a ratio greater than1 through a tolerance/clamp.

7. **Normalize once as construction, with explicit postconditions.** Obtain Z
   with accepted09 checked Neumaier summation in ascending count order, require
   finite Z in[1,n+1], and divide all weights by Z exactly once. This removes the
   unknown relative anchor, not a conditional repair of an already calibrated
   PMF. Return finite cells in[0,1], positive modal mass and nonincreasing entries
   outward from the selected mode. Require a checked compensated final mass M
   with abs(M-1)<=8*f64::EPSILON. This fixed numerical admission gate is not a
   per-cell/tail accuracy bound or exact stored-mass certificate. On failure,
   error without renormalizing again, dumping residual mass into a cell, flooring
   probabilities, clamping or trying another estimator. No normalization report
   is needed for this canonical vector construction.

8. **Make representational tail limits explicit.** Interior-law cells are
   mathematically positive, but final binary64 values may be0. Permit arithmetic
   underflow without an arbitrary tiny-probability cutoff or heuristic support
   truncation. Do not promise every representable true tail survives rounded
   recurrence/normalization or every component is correctly rounded. An exported
   zero is not proof a count is impossible; normalized mass and monotonicity do
   not establish relative-tail accuracy or negligible payoff-weighted loss.
   Do not force exact real/bitwise sum1 or mirror a separately rounded complement
   distribution to claim universal symmetry. No new log-PMF API is selected.

9. **Preserve useful scratch semantics without transactional copying.** Validate
   probability, count and storage arithmetic, then reserve as needed before
   clearing/resizing out. Input/admission or reservation failure leaves existing
   values and length unchanged; capacity is not a financial state guarantee.
   Once calculation starts, a later computation error may leave scratch contents
   unspecified and callers must discard them. No extra full-buffer copy/rollback
   is required. The allocating wrapper delegates to the same kernel and exposes
   identical success/failure behavior; no successful partial or stale array.

10. **Keep truthful bounded work and failure categories.** Invalid probability,
    unsupported n and impossible size are input/domain failures. Failed allocation,
    mode arithmetic, coefficient/weight/sum/normalization and output admission
    failures are computation/resource errors with stage and available k context.
    Every loop is bounded by the admitted support; inputs are immutable apart
    from the declared scratch writes. No panic, successful NaN/infinity, silent
    saturation, hidden retry, correction threshold or fallback distribution.
    Fixed order/FMA/compensation does not imply portable bit identity or a global
    numerical error certificate.

11. **Preserve the existing Rust-only boundary.** These two functions have no
    current Python/WASM exposure; the parity contract intentionally treats the
    distributions module as Rust-only. Do not add host bindings, schemas, classes
    or generated artifacts merely for this decision. Reconcile canonical Rust
    docs/examples/tests and actual caller errors with the selected strict domain,
    fixed limit, algorithm, scratch and accuracy rules in the same runtime slice.

12. **Leave credit assumptions, weighting and approximations with owners.**
    The actual CDS-tranche caller uses a reusable PMF inside conditional expected
    loss and copula integration, reads it only after success and already propagates
    its errors. Preserve that path, including first-error integration behavior;
    do not label temporary callback zeros as successful error suppression. Credit
    pool n>0/u16 constraints, homogeneity tolerances/routing, recoveries/exposures,
    factor models and large-pool/heterogeneous approximations are separately owned.
    A full binary64 PMF cannot support every tail-weighted payoff: if an erased
    cell times a large payoff could matter, the owner must select/qualify a
    suitable same-model evaluation rather than treat the zero as exact. This
    package approves neither unconditional independence nor production credit risk.

13. **Require independent formula and numerical qualification.** Cover exact
    rational small PMFs and ties, represented-probability near-integer modes,
    extreme/subnormal probabilities, wide n and the inclusive cap, unit mass,
    moments/outward shape and actual cell/tail errors against independent
    high-precision references. Include invalid p even at n0/endpoints, resource/
    scratch failure boundaries and allocating/into equivalence. Dependency
    comparison, normalized moments and symmetry are distinct from independent
    accuracy and financial validation. This review changes no runtime algorithm,
    tests, fixtures, dependencies or host API.

## Boundary examples for the accepted package

Numerical PMFs below are mathematical reference targets, not additional
correct-rounding guarantees beyond the selected exact identities.

| Input / situation | Required accepted outcome |
| --- | --- |
| Invalid p, including n0, or n>1,000,000 | Input/domain error before scratch values/length change; no saturation or identity bypass. |
| Admitted n0 / p0 / p1 | [1] / full vector with only P0=1 / full vector with only Pn=1. |
| n4,p1/4 | [81,108,54,12,1]/256. |
| n5,p.5 | [1,5,10,10,5,1]/32. |
| n3,p.5 | Modes1,2 tie; choose upper seed2, yielding[1,3,3,1]/8. |
| n2,p=float64(1/3) | Exact3p=1-2^-54, so mode0; rounded3*p=1 must not select mode1. |
| n9,p=float64(.3) | Exact10p<3, so mode2 even though floating multiplication rounds to3. |
| n1,p=eta=2^-1074 | Preserve the representational[1,eta] behavior; do not erase the tail to force an exact real sum1. |
| n2,p=eta | Reference rounded cells[1,2eta,0]; no promise of universally correctly rounded tails. |
| n1000,p=2^-276 | Reference P4 rounds to39eta; forming w3*p before the full outward coefficient must not prematurely erase the contribution. |
| n2,p=2^-600, payoff2^1000 only at K2 | Exported P2 underflows0, but exact weighted expectation2^-200 is representable: owner-tail limitation. |
| Failed final8*EPS mass gate, invalid ratio/output or unresolved arithmetic | Computation error without repeat normalization, clipping or residual-cell repair. |
| Input/resource error before writing / computation error after writing | Existing scratch values/length preserved / scratch contents unspecified and discarded. |
| n=1,000,000 / n=1,000,001 | Inside the resource domain subject to allocation/computation / input error, even for p0/p1. |

## Observed choice and rationale

A logarithmic recurrence avoids direct binomial coefficients and factorial overflow. All probabilities are materialized to support portfolio count distributions. Independence and common p are caller assumptions.

## Observed algorithm

1. Reject NaN p before clearing the caller's scratch vector. Resize to n+1 zeros.
2. For p≤0 set P(0)=1; for p≥1 set P(n)=1. This includes infinities and finite out-of-range p.
3. Otherwise start `log P(0)=n log(1−p)` and `log_ratio=log(p)−log(1−p)`. Advance `log P(k+1)=log P(k)+log(n−k)−log(k+1)+log_ratio`, exponentiating each entry.
4. Sum mass with ordinary f64 addition. If mass>0 and |mass−1|>1e-10, divide every entry by mass. Return the vector without a separate mass-error result.

## Observed selection, controls and conventions

The allocating wrapper and scratch-buffer variant share exactly the same kernel. n is an integer trial count; p is a decimal probability. The normalization trigger is fixed, not caller-configurable.

## Observed limitations, failure and fallback

Out-of-range p is silently saturated to a degenerate distribution. Memory is O(n); n+1 allocation has no public resource cap. Underflow can erase tiny probabilities; normalization does not recover their relative values. No approximation such as Poisson or normal is selected for large n. The returned result is not a correlated credit portfolio distribution.

## Observed alternatives and tradeoffs

Direct probability recurrences or a mode-centered recurrence are possible alternative implementations; they are not selected here. Poisson-binomial convolution and copula integration answer different assumptions and belong to their product records.

## Evidence

- **Observed source and located tests.** [Implementation and inline tests](../../../../finstack-quant/core/src/math/distributions.rs) reject only NaN before scratch writes, saturate other invalid probabilities, form unchecked n+1 and accumulate a forward logarithmic recurrence. An ordinary mass sum triggers normalization only when abs(mass-1)>1e-10. `test_binomial_pmf_all` checks small hand values; `binomial_pmf_all_matches_statrs` and extreme-probability cases use another computational implementation. `binomial_pmf_all_large_n_keeps_probability_mass` checks total mass and positive modal mass for n2000; the separate n100 credit-portfolio test checks mean/variance. Tests were inspected/located, not executed, and do not qualify the accepted replacement or homogeneous-independence assumptions.
- **Primary law and derived array profile.** The [binomial reference](../../../REFERENCES.md#nist-binomial) identifies the PMF and mode inequality. Dividing adjacent exact PMFs derives the two outward ratios; an upper exact mode implies weights<=1 and positive Z>=1. The relative-weight normalization is the selected derived project algorithm, not a claim that NIST/Boost supplies or certifies this array implementation. The fixed count cap and8*EPS mass gate are project policies, not source-prescribed statistical accuracy limits. statrs0.18 supplies scalar Binomial pmf/ln_pmf but no full-array counterpart; per-cell dependency comparison remains a distinct evidence role.
- **Fused-product reasoning.** [Rust f64::mul_add](https://doc.rust-lang.org/std/primitive.f64.html#method.mul_add) specifies a single final rounding of the product-plus-sum. With exact represented p and exact bounded counts, each selected numerator/denominator rounds its exact nonnegative product once. Ordered rounding preserves the exact-mode outward ordering before division; forming separate rounded complements or partial products does not have that same derivation. This supports the selected arithmetic structure without proving per-cell/normalization accuracy or cross-target bit identity.
- **Independent exact references.** On2026-11-08, standard-library Fraction/Decimal calculations independently checked small rational laws: n4,p1/4 gives[81,108,54,12,1]/256; n5,p.5 gives[1,5,10,10,5,1]/32; n3,p.5 gives[1,3,3,1]/8 with two modes. The first case has relative weights[3/4,1,1/2,1/9,1/108] and Z=64/27. Represented-p calculations give3*float64(1/3)=1-2^-54 exactly, unlike the rounded floating product1; likewise10*float64(.3)<3 exactly while its floating product is3. These verify the exact-mode distinction independently of the production routine.
- **Subnormal/composition references.** With eta=2^-1074, exact rational n1/n2 laws round to[1,eta] and[1,2eta,0]; their exact-real stored sums exceed1 although ordinary binary64 sums round to1. For n1000,p=2^-276, the exact P4 is about38.57270325534eta and rounds to39eta, while prematurely forming w3*p yields about.154755eta and loses the cell before a later factor could rescue it. For n2,p=2^-600, P2=2^-1200 underflows but a K2 payoff2^1000 has representable expectation2^-200. These motivate safe coefficient ordering and explicit financial tail limits; they do not qualify every recurrence tail or numerical payoff. Exact symmetry requires exact complementary inputs: represented .1 and its rounded 1-p do not sum to1 in exact real arithmetic. No bitwise symmetry or correct-rounding certificate is inferred.
- **Actual Rust-only consumer boundary.** The [parity contract](../../../../finstack-quant-py/parity_contract.toml) leaves distributions Rust-only. [CDS-tranche scratch evaluation](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/sensitivities.rs) calls the core into function, propagates failure and reads scratch only after success; it does not require rollback copying. [Conditional expectations](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/expected_loss.rs) and [factor integration](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/integration.rs) retain the first callback error, so temporary callback zero is not a successful output fallback. [Homogeneity routing](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/heterogeneous.rs) and [credit pool count](../../../../finstack-quant/core/src/market_data/term_structures/credit_index.rs) own their tolerances/positive u16 count restrictions. They do not change generic n0, prove unconditional independence, or adopt a heterogeneous/large-pool approximation in this record.
- **Qualification limit.** Read-only independent mathematical/host/caller reviews assessed the complete package. No production Rust suite, benchmark, fixture generator, binding or statistical/financial experiment ran. [Monte Carlo/probability context](../../../REFERENCES.md#glasserman-2004-monte-carlo) remains general context; actual price/risk/tail-weighted acceptance stays with the caller.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
