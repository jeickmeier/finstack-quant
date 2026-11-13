# MODELS-NUMERICAL-009: Bernoulli correlation coupling and reproducible lattice paths

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Source and callers inspected. Independent rational checks covered strict coupling and finite-grid sampling boundaries. A standalone Philox calculation matched published vectors and checked segmented and antithetic replay. Production pricing, risk and external joint qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked rates-credit coupling and path sampling

1. **Couple the admitted binary innovations.** Use the checked
   marginals and actual axis widths from record 008. Correlation
   means the conditional Pearson correlation of the two raw
   up/down innovations at one departing joint node. It has the
   same requested value at every admitted stochastic pair.
   The effective hazard floor can compress intensity dispersion
   without removing the latent hazard innovation. This parameter
   does not identify correlation of rates or intensities across
   time, terminal levels, realized defaults or market spreads.
   [Accepted marginal law](rates-credit-marginal-calibration.md#accepted-decision-for-checked-rates-credit-marginal-calibration)

2. **Require a defined correlation only for two stochastic axes.**
   The prepared request supplies a required nullable correlation.
   Two stochastic axes require an explicit finite value in
   [−1,1]. Exact zero selects conditional independence. If
   either axis has one deterministic state, require absence
   of a numeric correlation. An axis with one child has no
   Bernoulli innovation. A single stochastic axis uses its
   admitted marginal, and two deterministic axes have one
   joint child with unit weight.
   [Accepted correlated Bernoulli law](../core-numerics/correlated-bernoulli.md#accepted-decision-strict-correlated-bernoulli-laws)

3. **Use one canonical checked bivariate Bernoulli law.** At
   each stochastic pair take the represented up probabilities
   p and q from the prepared axes and their mathematical
   complements. Use the same pair convention in marginal
   replay and joint valuation. With covariance increment
   `c=rho*sqrt(p*(1-p))*sqrt(q*(1-q))`, the cells in order
   (up/up, up/down, down/up, down/down) are
   `(p*q+c, p*(1-q)-c, (1-p)*q-c, (1-p)*(1-q)+c)`.
   Reuse the accepted checked core construction and its
   preserved probability information. Remove probability
   projection, epsilon-expanded feasibility and separate
   requested versus effective rho.
   [Coupling and sampling references](../../../REFERENCES.md#checked-rates-credit-coupling-and-path-sampling)

4. **Check whole-lattice feasibility from row extrema.** For
   every departing row with two stochastic axes, test the four
   combinations of actual minimum and maximum represented
   up probabilities using the strict constructor. For interior
   probabilities and log odds l(p) and l(q), the real upper
   bound is `exp(-abs(l(p)-l(q))/2)` and the lower bound is
   `-exp(-abs(l(p)+l(q))/2)`. The largest absolute sum and
   difference over the row rectangle occur at corners.
   This proves the intersection for all stored pairs,
   including pairs that have no path mass for a particular
   endpoint coupling. A genuinely infeasible rho errors.

5. **Report the full signed feasible range as a diagnostic.**
   Intersect the row and corner bounds and report both
   endpoints when correlation is defined. A one-state axis
   makes this range unavailable. Use checked bounds from
   the actual represented marginals. Exact symmetric laws
   can have a symmetric interval, while rounded inputs
   require the actual bounds. A displayed rounded endpoint
   still needs strict construction. Retire the symmetric
   magnitude as the admission interface.

6. **Validate coupling before publishing the joint candidate.**
   Complete the marginal gates and whole-lattice feasibility
   before replacing a prepared model. Keep checked local
   transition construction on demand. The corner proof
   establishes mathematical feasibility for all pairs.
   Required cell or cutoff arithmetic at use can still
   fail and propagates with node context. Sampled paths need
   no dense Cartesian probability table or exhaustive
   joint scan. Check actual marginal and requested joint
   work under the resource contract.

7. **Share the same children and probabilities across uses.**
   Joint rollback, joint coupon-folded claims and sampling
   consume the same prepared law. Preserve stable contributions
   from the checked cell information through the signed
   weighted sum. A rounded cell view alone can lose a
   consequential product. Exact structural zeros have no
   contribution. Use checked widths and indices, including
   the sole child on a deterministic axis. A rate-marginal
   conditional unit claim uses its checked rate branch law.
   Record 010 owns its use in cashflow valuation.
   [Checked rollback](tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback),
   [joint coupon folding](rates-credit-coupon-folding.md)

8. **Pin the Philox draw coordinates.** Use the shared
   Philox4x32-10 profile with the supplied u64 effective seed
   as key and the u64 base path ID as stream ID. Departing
   step k uses block `floor(k/2)` and word pair (0,1) at
   even k or (2,3) at odd k. The counter layout puts the
   block in its low 64 bits and stream in its high 64 bits.
   Form a little-word-first u64 from the pair. Reserve
   one draw address for every departing step. A deterministic
   move can skip evaluating its reserved slot. There is
   no terminal draw.
   [Philox references](../../../REFERENCES.md#checked-rates-credit-coupling-and-path-sampling)

9. **Use exact open midpoint uniforms and reflection.** Take
   the top 52 bits b of the paired word and map to
   `u=(b+1/2)*2^-52`. The antithetic leg uses the same
   key, base ID and step, with index `(2^52-1)-b`.
   This is the exact represented reflection 1−u. Both
   endpoints are strictly inside (0,1). A base ID denotes
   one random coordinate stream even when used by both
   legs. The product owns effective-seed derivation,
   training/pricing separation and pairing in its estimator.

10. **Sample the preserved half-open cumulative geometry.**
    Use the accepted core sampler in the stated outcome
    order. Its mathematical cuts are P11, p and 1−P00
    before the endpoint one. Exact equality advances to
    the next nonempty interval. A single active axis uses
    its up-first split, and a deterministic move has no
    random outcome choice. Use preserved cutoff arithmetic
    where rounded sums would lose a boundary decision.
    Reject unresolved decisions. Remove rounded-cell
    accumulation and unconditional residual assignment.

11. **State the resolution of the sampled law.** With exact
    cutoff comparisons the midpoint grid differs from the
    ideal CDF by at most 2^−53 at a cut, and from an ideal
    cell probability by at most 2^−52. Some positive
    cells receive no midpoint. Structural zero cells are
    impossible. The prescribed finite random grid does
    not promise exact sampled marginals, exact empirical
    correlation or antithetic variance improvement.

12. **Return checked factor states and departing interval terms.**
    A sampled path includes the prepared time and node identities.
    At departing nodes expose the signed short rate, effective
    hazard and checked base discount, local survival and default
    terms. Use the same prepared width and integrated hazard
    law as the fit, preserving the local default complement
    with expm1. These are conditional survival weights along
    a factor path. Default time, recovery and OAS cash valuation
    stay with their owners. The terminal node exposes genuine
    factor state and has no outgoing rate, hazard, factor or
    spread. Retire the terminal (1,1,0) interval substitute.

13. **Give a segment exact inclusive state boundaries.** A
    segment from a valid checkpoint at s through e returns
    states s..=e and uses only transition addresses s..e−1.
    Require 0≤s≤e≤N and validate both indices against their
    actual marginal widths. A length-one request draws
    nothing. An interior segment endpoint still has its
    actual outgoing interval terms. The root and a complete
    path use the same prepared law and coordinates.

14. **Bind resumable checkpoints to the originating path.**
    Canonical root creation binds the prepared model,
    effective seed, base ID and antithetic leg. A non-root
    checkpoint comes from a checked state of that path and
    preserves its step and two node indices. Validate the
    prepared model and path identity on resume without
    replaying the prefix. In-range node indices alone
    cannot establish same-path provenance. A resumed
    segment from a matching checkpoint reproduces the
    corresponding uninterrupted states on the same target.
    Keep product accumulators and exercise state in
    their existing owner.

15. **Check sampling work and propagate failures.** Validate
    the prepared model, logical IDs, range, indices and
    draw-address conversions. Check inclusive output
    length and allocation before clearing a reusable buffer.
    Admission failure leaves the previous buffer available.
    A later computation failure returns no partial path.
    Use actual node counts for one-state axes and preserve
    segment order. Recalibration invalidates prior prepared
    checkpoints for the replaced model.

16. **Keep the band cache observationally equivalent.** A
    cached entry and a checked direct computation use the
    same immutable prepared node terms and error contract.
    An admitted out-of-band node follows direct evaluation.
    Invalid nodes fail before cache use. The cache changes
    work only. It neither truncates tails nor changes the
    probabilities, intervals or path coordinates.

17. **Reconcile joint products and existing host inputs.**
    The callable-bond and sampled bullet paths use the
    same indexed factor sampler. Bermudan-style blocked
    training replays from bound checkpoints. Handle
    terminal cash and exercise without synthetic regression
    features. The joint term-loan path shares the checked
    coupling in main and coupon rollback. Carry nullable
    correlation semantics and errors through the existing
    Rust, Python and WASM product surfaces. Prepared
    lattice, range diagnostics and checkpoints stay Rust-only.
    [Bond PV dispatch](../valuations-bonds/03-pv-models.md),
    [term-loan pricing](../valuations-other-fixed-income/08-term-loan-pricing.md)

18. **Qualify reproducibility, dependence and risk separately.**
    Verify the strict corner scan against exhaustive small
    grids, zero and one-state cases, infeasible boundary
    inputs, exact sampling cuts, known-answer RNG vectors,
    resumed segments and antithetic legs. Scope draw and
    path repeatability to the declared profile and
    prepared model, with target-specific baselines under
    the reproducibility ADRs. Price aggregation and pair
    statistics remain with the estimator. A risk scenario
    can reuse random addresses after preparing its own
    model and checkpoints. A bump that makes correlation
    undefined requires explicit owner treatment. Qualify
    actual option, coupon and credit-risk prices independently.
    [Reproducibility](../../standards/0021-reproducibility-guarantees.md),
    [deterministic execution](../../standards/0043-deterministic-ordering-and-monte-carlo-execution.md)

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Up probabilities {.25,.75} on both axes | The four-corner and full scans give [−1/3,1/3]. At p=.25, q=.75 the binary64 value immediately above rounded 1/3 gives exact P10=−2^−57. |
| p=.5, q=.25+5×2^−54, rho=0, midpoint u=.625+2^−53 | The true third cut exceeds u by 2^−55, so the outcome is down/up. The observed rounded three-cell cumulative cut equals u and chooses down/down. |
| p=q=.5, rho=−1, u=.5 | The zero up/up and down/down cells are impossible. Half-open comparison chooses down/up at the midpoint boundary. |
| p=2^−54, q=.5, rho=0 | The positive up-rate region lies below the first open Philox midpoint. This region has no hit on the selected uniform grid. |
| p=q=nextDown(1), rho=0 | The down/down cell is exactly 2^−106. The observed subtractive expression for that cell gives zero. |
| Philox key and stream zero, first four step addresses | The 52-bit indices are `e169c58d6627e`, `9b00dbd8bc57a`, `5cb200dbf8e4c`, and `97eff67b1a57` in hexadecimal. Independent block computation agrees with the Random123 vector and odd/even resumed draws. |

## Observed implementation and evidence

### Choice, scope and algorithm

[sampling.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/sampling.rs) preserves each fitted marginal while coupling one-step up/down indicators. Set a=p_r p_h+ρ√(p_r(1−p_r)p_h(1−p_h)). Joint probabilities are (a,p_r−a,p_h−a,1−p_r−p_h+a). Clamp a to the Frechet interval [max(p_r+p_h−1,0),min(p_r,p_h)] after a debug assertion; calibration is expected to have rejected economically infeasible ρ.

[calibration.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/calibration.rs) scans each row's minimum/maximum nondegenerate marginal probabilities, tests their four combinations and intersects attainable correlation intervals. Degenerate p=0/1 nodes are skipped. No scan is needed for ρ=0 or two zero mean-reversion factors (p=1/2). `max_feasible_correlation` returns the symmetric magnitude min(|lower|,|upper|), not the complete asymmetric interval.

### Sampling, controls and conventions

Use `PhiloxRng::with_stream_u01_offset(seed,path_index,checkpoint.step)` and consume one uniform per transition. Cumulative intervals select up/up, up/down, down/up, down/down; antithetic sampling replaces U by 1−U. Segment resumption preserves random-coordinate alignment. Each returned node includes raw short rate, floored hazard, exp(−r dt), exp(−h dt), and stable default probability −expm1(−h dt); terminal values use (DF,survival,default)=(1,1,0). These are weighted survival states: no default time is drawn by this method.

`BandTable` stores cached terms around step/2 with half-width 2 ceil(√step)+2. Zero-volatility rows cache one identical term. Out-of-band access evaluates the identical formula; this is an acceleration cache, not lattice truncation or discarded probability mass. Invalid checkpoint/segment geometry fails.

### Alternatives and evidence

Correlation here is Bernoulli innovation correlation, not terminal levels or realized default correlation. Degenerate marginal tails cannot express arbitrary innovation correlation. Gaussian/copula or exact continuous factor simulation would be different methods. [tests.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/tests.rs) contains `extrema_correlation_scan_matches_exhaustive_node_pairs`, `sampled_joint_moves_preserve_skewed_correlated_marginals`, `sampled_paths_are_seeded_weighted_and_antithetic`, and `sampled_states_match_direct_node_terms_bit_for_bit`. These are implementation/invariant evidence; external market calibration is absent. [Philox reference](../../../REFERENCES.md#salmon-2011-philox) supports the RNG family, not the coupling model. [band_table.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/band_table.rs) is storage plumbing and adds no probability approximation.

The product resolver currently defaults omitted correlation to
zero. Current checkpoint validation checks the step and two
indices without binding the prepared model or the random
coordinates. Callable-bond LSMC constructs and consumes these
checkpoints in blocked exercise replay and can read terminal
rate and hazard as regression features. Joint term-loan rollback
uses the coupled law in both ordinary continuation and its
coupon-folded claims.
[Product correlation resolver](../../../../finstack-quant/valuations/core/src/instruments/pricing_overrides.rs),
[bond checkpoint construction](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/mod.rs),
[bond replay and terminal snapshots](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/replay.rs),
[joint rollback and coupon claims](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/pricing.rs)

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
