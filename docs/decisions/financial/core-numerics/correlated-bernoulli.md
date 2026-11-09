# CORE-NUMERICS-16: Feasible correlated Bernoulli joint laws

- Decision status: **Accepted — complete correlated-Bernoulli package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent exact-rational examples and read-only mathematics/host/caller reviews were completed on2026-11-08; these do not certify production arithmetic, sampling or a financial coupling.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/probability.rs`: two Bernoulli events with decimal marginal probabilities and their joint law, Pearson feasibility, conditional queries and scalar-uniform sampling. This is a two-event construction, not a general high-dimensional copula. The complete accepted domain and undefined-correlation rules are below; current source behavior is retained separately.

## Accepted decision: strict correlated Bernoulli laws

Accepted on **2026-11-08** after whole-record review. Runtime,
host/caller reconciliation and independent numerical/statistical/financial
qualification remain pending; source-observed behavior is retained below.

1. **Retain one immutable validated two-event law.** Keep CorrelatedBernoulli
   with private canonical inputs and checked cached/factored joint information,
   Clone/Debug and atomic construction. No Default, public mutable fields,
   setters, unchecked constructor, new serde/restore path, coupling registry or
   general multivariate copula. The same mathematical law must govern reported
   cells, conditional queries and sampling, subject to disclosed binary64 views.

2. **Make required correlation explicit and undefined cases honest.** Select
   new(p1,p2,correlation:Option<f64>)->Result<Self>. Both marginals must be finite
   in[0,1]. Require Some finite rho in[-1,1] iff both marginals are strictly
   interior; require None iff either marginal is exactly0/1. The third argument
   remains required, with no omitted independent/zero default. A deterministic
   marginal uniquely fixes the joint law but makes Pearson correlation undefined.
   Remove the1e-10 near-degenerate threshold: tiny positive variance is not zero.
   Signed zeros may be canonicalized without an epsilon endpoint policy.

3. **Select strict Fréchet feasibility with no projection.** For interior inputs,
   let s=sqrt(p1*(1-p1))*sqrt(p2*(1-p2)), t=p1*p2+rho*s,
   L=max(0,p1+p2-1), U=min(p1,p2). Require L<=t<=U for the actual represented
   inputs. Reject genuinely infeasible requested rho as an input error; do not
   project, clamp a small negative cell, relax a tolerance or replace dependence
   by independence. If numerical uncertainty prevents resolving required
   feasibility, return computation failure rather than falsely accepting it.
   Preserve exact rho0 independence, rho1 only for equal interior marginals, and
   rho-1 only for exactly complementary interior marginals, without rounded-sum
   or epsilon tests.

4. **Keep bounds as checked numerical views, not an admission oracle.** Retain
   correlation_bounds(p1,p2)->Result<Option<(f64,f64)>>: None only for a genuinely
   deterministic marginal; otherwise a finite ordered pair in[-1,1]. Use stable
   covariance bounds cmin=-min(p1*p2,(1-p1)*(1-p2)) and
   cmax=min(p1*(1-p2),(1-p1)*p2), or equivalent bounded ratios. With m=min(p1,p2),
   M=max(p1,p2), rhoMax=sqrt(m/M)*sqrt((1-M)/(1-m)). For the exact sum<=1 branch,
   rhoMin=-sqrt(p1/(1-p2))*sqrt(p2/(1-p1)); above1 use
   -sqrt((1-p1)/p2)*sqrt((1-p2)/p1). Preserve complementary/affine information
   in branch selection. These rounded interval endpoints need not themselves
   be exactly feasible inputs: construction must independently verify geometry.
   No exact endpoint roundtrip or global correct-rounding claim is selected.

5. **Construct cells without losing consequential products or boundaries.**
   The cells in order(11,10,01,00) are
   (p1*p2+c,p1*(1-p2)-c,(1-p1)*p2-c,(1-p1)*(1-p2)+c), c=rho*s.
   Use stable scaled/fused products, compensated differences and retained local/
   exponent information rather than sqrt(var1*var2), raw tiny products or a
   rounded p1+p2-1. Combine terms before final rounding when a half-subnormal
   tie or cancellation could change a required result. Exact deterministic and
   perfect-dependence identities keep structural zero cells exact. No generic
   arbitrary-precision framework, cell floor, renormalization or residual dump.
   Required unresolved arithmetic errors; not every finite feasible input is
   guaranteed to succeed.

6. **Require complete checked output with qualified reconstruction claims.**
   Report four finite cells in[0,1]. Checked compensated total mass must be
   within8*EPSILON of1, and the two reconstructed marginal sums within8*EPSILON
   of their supplied probabilities. These fixed numerical admission gates do
   not prove exact marginals, dependence or individual-tail accuracy. Positive
   mathematical cells may round0, and independently rounded cells cannot always
   encode the exact input law. Preserve underlying nonzero/zero and factored
   information for later queries rather than infer impossibility from a rounded
   cell. Failed gates or negative/nonfinite output errors, with no tolerance-based
   cleanup or successful partial state.

7. **Remove requested/effective duplication and use canonical short getters.**
   Remove requested_correlation and the separate effective-correlation state.
   get_correlation()->Option<f64> reports the admitted correlation parameter,
   Some only for interior marginals, not a measured Pearson statistic of the
   rounded cell tuple. Use get_p1/get_p2, get_p11/get_p10/get_p01/get_p00 and
   get_joint for the four-cell tuple. Reconcile these names across all three
   languages, removing old property/method aliases rather than preserving them.

8. **Use genuine conditional availability and stable requested ratios.**
   Select get_p2_given_x1/get_p1_given_x2 ->Result<Option<f64>>. None means the
   conditioning marginal is exactly0, not an arithmetic failure or a rounded-zero
   joint numerator. For a positive conditioning probability, compute Bayes from
   preserved law information before requested division/restoration; require a
   finite result in[0,1] or return computation error. Independence retains the
   opposite marginal even for subnormal events. Some0/Some1 are valid results;
   remove the impossible-event opposite-marginal fallback and any clamp.

9. **Use a strict half-open scalar-uniform sampling domain.** Rename the sole
   canonical method to sample(u)->Result<(u8,u8)>, finite u in[0,1). Reject1,
   NaN/infinity/outside values, without nudge, floor or extra draw. This method
   transforms a supplied variate, not an RNG/seed; successive independent draws
   require the caller's independent uniforms. Preserve outcome order11,10,01,00
   and half-open comparisons; boundary equality goes to the next nonempty cell.

10. **Sample from preserved cumulative geometry.** The mathematical cuts are
    t=P11, p1 and1-P00, ending at exact1. Preserve monotone cuts and exact
    coincident boundaries for structurally zero cells, using scaled/affine or
    complementary comparisons where rounding would erase a required decision.
    Never select a structurally impossible outcome or create its interval by
    summing rounded cells and sending leftovers to an unconditional final else.
    u0 belongs to the first mathematically positive interval even if its reported
    cell rounds0; rounded table values alone do not reproduce every sampling
    comparison. If a required boundary decision is unresolved, return computation
    error. A finite uniform grid cannot guarantee reachability or exact mass for
    every tiny event; no bitwise distribution/reconstruction promise is selected.

11. **Preserve explicit failures and a simple state boundary.** Invalid marginal,
    rho-presence/range, proven infeasibility and invalid u are input/domain errors.
    Unresolved bounds/feasibility, cell/conditional/cumulative arithmetic and
    numerical postcondition failures are computation errors with stage context.
    Validate complete requests before identities. Construction failure publishes
    no object; queries/sampling do not mutate it. None is reserved for the
    stated mathematical undefined cases. No successful NaN, undeclared infinity,
    hidden repair, retry, alternate dependence model or new diagnostics/state
    framework. O(1) work/storage does not establish all-range numerical accuracy.

12. **Reconcile the existing hosts and parity in one slice.** Preserve the
    current models.correlation Python/WASM placement documented in parity,
    rather than adding parallel core hosts. Keep the frozen/private wrappers,
    four-cell tuple/typed-array and sample-pair shapes. Map required nullable
    correlation and optional query/bounds results explicitly as Python None and
    WASM null; no omitted argument/default for constructor correlation. Convert
    the canonical short get_* and sample names together, remove requested/effective
    aliases and update stubs/runtime docs/declarations/examples/ledger. Use
    centralized ValueError/domain versus RuntimeError/computation mapping and
    corresponding WASM categories; no host-only financial logic.

13. **Keep financial coupling and higher-dimensional compatibility separate.**
    No current Rust pricing/model caller delegates to this core type; actual
    consumers are direct host users. The rates-credit tree has its own transition
    coupling and feasibility choices. Do not implicitly adopt its clipping,
    marginal/default-probability, calibration or state/recombination policy, or
    claim this core change fixes its prices. Two-event feasibility/PSD cannot
    certify a multivariate Bernoulli correlation matrix or financial dependence
    model. Financial owners retain chosen dependence, evidence and risk acceptance.

14. **Qualify boundaries and requested outputs independently.** Cover exact
    rational laws, rho endpoints/near-feasible violations, actual binary64 affine
    complements, tiny/subnormal products and combined ties, deterministic/rare
    conditionals, rounded-table versus latent-law behavior, every sample boundary/
    zero-cell/u rejection, host null/name/error parity and actual owner boundaries.
    Separate algebraic/mass gates, independent numerical accuracy, finite-grid
    sampling and statistical/financial evidence. This review changes no runtime,
    test fixture, generator or host API; production reconciliation remains pending.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Interior marginals with None, or deterministic marginal with Some rho | Input error; no inferred independence or rho0 sentinel. |
| p1=0,p2=3/4,correlationNone | Unique cells(0,0,3/4,1/4), correlation/boundsNone, forward conditionalNone and reverseSome0. |
| p1=1/4,p2=3/4,rho0 | Reference cells(3,1,9,3)/16; feasible interval[-1,1/3]. |
| Same marginals,rho1/2 | Input error, not projection to1/3 or cleanup of negative P10. |
| Same marginals,rho=nextUp(float64(1/3)) | Exact P10=-2^-57: genuinely infeasible despite its tiny magnitude. |
| p1=p2=2^-40,rho1 | Cells(p,0,0,1-p), no near-degenerate independence branch. |
| rho1 with unequal interior marginals /rho-1 with noncomplementary marginals | Input error even if reported rounded bounds contain+/-1. |
| p1=.5,p2=.5+2^-53 | Exact lower joint bound2^-53, not0 from rounded addition. |
| p1=p2=2^-600 | Preserve representable standard-deviation products despite var1*var2 underflow. |
| p1=eta,p2=.5,rho0 | Rounded P11 may0, but conditionalSome.5 and latent sampling information remain available. |
| p1=p2=eta,rho.5 | True P11=eta/2+eta²/2; retain combined information or error, not separately rounded product/covariance zero. |
| p1=p2=.5,rho-1 | Cells(0,.5,.5,0); u0 samples10,u.5 samples01,u1 errors. |
| p1=.5,p2=2eta,rho0,u=.5 | Sample01 from the positive P01=eta interval, retaining u-p1<P01 even though p1+P01 rounds to p1. |
| Unresolved numerical feasibility/cell/query/cut | Computation error, no epsilon projection, fabricated conditional or impossible outcome. |

## Observed choice and rationale

The implementation projects requested correlation onto the Fréchet-feasible interval while retaining both requested and effective correlation. This keeps joint probabilities compatible with marginals. Whether projection is permitted in a financial model is not implied by the constructor succeeding.

## Observed algorithm

1. Compute v_i=p_i(1−p_i). If either variance is below 1e-10, assign effective rho=0 and independent joint masses, while preserving requested rho.
2. Otherwise compute rho bounds from `(max(p1+p2−1,0)−p1 p2)/sqrt(v1 v2)` and `(min(p1,p2)−p1 p2)/sqrt(v1 v2)`, each clamped into [-1,1]. Clamp requested rho to those bounds.
3. Let c=rho_eff sqrt(v1 v2). Set P11=p1p2+c, P10=p1(1−p2)−c, P01=(1−p1)p2−c and P00=(1−p1)(1−p2)+c.
4. A supplied u∈[0,1] selects outcomes in order (1,1), (1,0), (0,1), (0,0), using strict comparisons with cumulative mass. Thus u=1 always selects the final outcome even if its mass is zero.
5. Conditional `P(X2=1|X1=1)=P11/p1` and its symmetric counterpart use the other marginal when the conditioning event has zero probability.

## Observed selection, controls and conventions

Projection and near-degeneracy threshold are fixed; no strict-rejection option exists. Randomness is supplied as a uniform scalar; the type itself has no seed or RNG. `correlation_bounds` returns (0,0) for the near-degenerate branch.

## Observed limitations, failure and fallback

Invalid input ranges error, but infeasible correlations within [-1,1] are silently changed. Tiny nonzero marginal variances below threshold lose requested dependence. Conditional values on impossible events are defined by a fallback convention, not Bayes' rule. Boundary roundoff in joint masses is not separately repaired or reported.

## Observed alternatives and tradeoffs

Strict feasibility rejection, a warning-bearing constructor, or conditional-probability parameterization would expose different contracts; none is available. Explicit effective-correlation access lets consumers record the projection, provided they choose to inspect it.

## Evidence

- **Observed source and located tests.** [Implementation and inline tests](../../../../finstack-quant/core/src/math/probability.rs) contain the1e-10 variance threshold, requested/effective duplicate state, automatic correlation clamps, direct variance-product square root, raw conditional fallback and closed-u1 final bucket. `test_joint_probabilities_marginals`, `test_correlation_bounds` and `test_correlated_bernoulli_reports_effective_correlation` check algebraic/projection properties of that source; uniform-partition tests locate fixed boundary behavior. These tests were inspected/located, not executed, and do not qualify the accepted strict/optional/numerical/host contract.
- **Direct primary provenance.** [Erdely2017 Example3.1](../../../REFERENCES.md#erdely-2017-bernoulli-subcopula), printed p238, supplies the four Bernoulli cells, Fréchet interval, independence condition and Pearson expression. [Huber–Marić2019](../../../REFERENCES.md#huber-maric-2019-bernoulli-correlations) distinguishes the completely characterized two-variable case from higher-dimensional admissibility, including a PSD3x3 counterexample. These support the mathematical law, not a projection, degeneracy threshold or numerical cleanup policy. Existing source bivariate-normal/t-copula links and [general probability context](../../../REFERENCES.md#glasserman-2004-monte-carlo) are not direct adoption evidence for those observed policies.
- **Independent exact-rational laws.** On2026-11-08, standard-library Fraction calculations independently checked p1=1/4,p2=3/4, s=3/16, bounds[-1,1/3], and cells at rho-1:(0,1/4,3/4,0), rho0:(3,1,9,3)/16, rho1/3:(1/4,0,1/2,1/4). Rho1/2 instead gives(9,-1,15,9)/32 and is infeasible. The actual binary64 rho immediately above float64(1/3) yields exact P10=-2^-57, showing why an epsilon-small negative is not automatically rounding noise. Exact deterministic/conditional and uniform-partition examples were also checked; no production kernel ran.
- **Affine/product/conditional targets.** Exact represented p1=.5,p2=.5+2^-53 have lower joint bound2^-53 even though their floating sum rounds to1. At p1=p2=2^-600 the variance product underflows while the standard-deviation product remains representable. At p1=p2=eta,rho.5, P11=eta/2+eta²/2 rounds to eta while separately rounded product/covariance terms erase it; rounded P11/p1 would then give1 rather than the underlying conditional near.5. At p1=eta,p2=.5,rho0, rounded P11=0 still has conditional.5. These source-derived references require retained requested-output information or truthful failure, not an arbitrary-precision framework or all-range success claim.
- **Sampling geometry targets.** At p1=p2=.5,rho-1, u1 currently reaches impossible00. At p1=.5,p2=2eta,rho0,u.5, the correct01 interval is lost if its cumulative cut is rounded back to.5; a preserved comparison u-p1<P01 resolves it. A structural zero and a positive interval erased in a rounded view are different. The accepted latent-law comparisons, [0,1) domain and numerical failure rule do not certify that a finite random grid reaches every positive cell or that the displayed rounded table exactly reconstructs the sampler.
- **Actual state/host/consumer boundary.** Rust has private immutable fields, Clone/Debug and no serde/default/restore path. [Python](../../../../finstack-quant-py/src/bindings/models/correlation/mod.rs) is frozen; [WASM](../../../../finstack-quant-wasm/src/api/models/correlation.rs) holds the canonical inner type. The [parity ledger](../../../../finstack-quant-py/parity_contract.toml) intentionally locates these core-owned operations in models.correlation. Required nullable correlation, optional scalar/bounds results, canonical get_*/sample names, docs/examples/declarations and centralized error mapping need coordinated reconciliation, preserving existing data shapes. No Rust pricing/model consumer delegates to this type; the [rates-credit tree coupling](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/sampling.rs) has separate gates/transitions and is not adopted or fixed by this core package.
- **Qualification limit.** Read-only independent mathematical/API/host reviews assessed the complete package. No production tests, binding build, generator, random experiment or financial model benchmark ran. Mass/marginal gates, arithmetic examples, finite-grid sampling and financial dependence acceptance remain separate evidence roles.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
