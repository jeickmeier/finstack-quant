# VALUATIONS-BONDS-07: Joint bond continuation regression, make-whole projection and sampling

**Decision status:** Accepted — complete checked joint bond exercise policy and make-whole package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, economic fit, pricing, risk and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked joint bond exercise policy and make-whole

1. **Select only the stages required by the admitted joint claim.**
   Consume record 06's exact decision dates, qualified alive state,
   event phases, contractual cash and separate recoverable par.
   An eligible immediate payoff known from current state is
   evaluated directly. Identify that case by its payoff and
   remaining claims, including legitimate zero or signed cash.
   A delayed ordinary terminal payment or retained contingent
   claim remains in continuation. Two deterministic axes use
   one-state rollback. A sampled bullet needs only pricing
   groups. A stochastic claim uses make-whole or continuation
   training only where the respective value needs estimation.
   After a frozen valuation-origin decision, bypass pricing
   paths only when every remaining payable value is resolved.
   Preserve the selected model and the same contractual
   entitlements in each structural reduction.
   [Accepted joint payoff replay](06-joint-coupon-replay.md#accepted-decision-for-checked-joint-bond-payoff-and-recovery-replay).

2. **Elect a qualified make-whole benchmark per right.** Use
   the deterministic-reference law of record 04 for a complete
   deterministic strip where its contract admits that law.
   Alternatively select a one-factor stochastic reference
   discount with deterministic basis on the fitted rate
   marginal. Require the exact reference source, matching
   currency, native dated clock, signed annual continuous
   benchmark spread, actual exercise and index/pay dates as
   applicable, and the contractually authorized promised cash
   strip and terminal horizon for the redeemed claim. Preserve
   its coupon, principal and entitlement identities. The strip
   is the continuation of the promised benchmark cash under
   its terms, independent of holder ex-coupon filtering or
   future stopping decisions. Exclude common same-day cash
   and separately retained claims under this profile.
   Require exact floor and accrued rules. A Treasury
   yield convention, notice-dated benchmark or separate basis
   risk requires its own qualified law. If a curve serves two
   authorized roles, a shock to its identity moves both.
   [Accepted deterministic make-whole and exercise](04-rates-tree-exercise.md#accepted-decision-for-checked-bond-rates-trees-and-exercise)
   and [filed Treasury benchmark example](https://www.sec.gov/Archives/edgar/data/72971/000119312522055987/d273152d424b2.htm).

3. **Condition the raw stochastic reference PV before applying its floor.**
   At exercise `E`, for promised cash `K_i` on actual date `T_i`,
   value the conditional expectation of the sum of
   `K_i*D_rate_path(E,T_i)*D_ref(E,T_i)/(B(0,T_i)/B(0,E))*exp(−s_ref*Δc_ref)`.
   Here `D_rate_path` is the unspread model interval discount
   product, `B` is the achieved rate-only unit bond, and
   `Δc_ref=c_ref(T_i)−c_ref(E)` uses the full native reference
   clock. Use the prepared model's rate-marginal pricing law and
   same dated coordinate mapping, with checked interval log
   differences and products. The achieved base ratio
   normalizes the shifted discount factors back to the
   reference curve at origin for fixed cash, even at a
   nonzero admitted base-fit residual. This is a deterministic
   basis projection under the selected rate model. It excludes
   hazard, survival, recovery, pricing OAS, and return-floor
   history. If the remaining strip is known conditional on the
   decision state, use achieved `B(E,T_i)` directly instead of
   training. On a deterministic rate axis with resolved strip
   this reduces to the deterministic reference law. Otherwise,
   train on independently simulated rate-marginal paths,
   replaying the benchmark's promised coupon and principal
   without call, put or default payoffs. Declare a separate
   ordered benchmark feature schema with only available
   rate and relevant contractual state. Omit hazard, including
   synthetic zero hazard features. If required benchmark cash
   depends on unresolved information outside that law, fail.
   Freeze the benchmark before fitting exercise. A missing or
   unfit active benchmark errors. Reject a negative benchmark
   prediction for a structurally nonnegative strip. Preserve signed
   estimates where the admitted benchmark terms allow signed cash.
   Do not clamp predictions. Apply the approved dirty
   make-whole rule `max(F,P_ref−B_accrued)+A_payable` to the
   raw conditional reference value, where `F` is the clean
   floor. Apply any record 05 return floor to the resulting
   dirty strike. Neither a pathwise maximum before expectation
   nor a substitute vanilla strike supplies this value.
   [Reference payoff and accrued](04-rates-tree-exercise.md#accepted-decision-for-checked-bond-rates-trees-and-exercise),
   [issue-basis protection](05-return-floors.md#accepted-decision-for-checked-issue-basis-return-floors)
   and [rate-marginal unit bonds](../models-numerical/rates-credit-coupon-folding.md).

4. **Fit continuation under the later frozen policy.** Work
   backward at exact live decisions with a genuinely unresolved
   hold value. For a training leg at decision `a`, use record
   06's realized affine continuation to the next decision `b`,
   `Q(a,b)*V_b+I(a,b)`, with intervening eligible cash and local
   recovery in `I`, plus any hold-specific current cash outside
   `I`. Exclude departure cash shared with exercise.
   `V_b` reflects the later frozen action and includes its
   eligible endpoint cash. Use the admitted factor-path
   population and equal weights for all qualified training legs
   at that decision, without moneyness or realized default
   filtering. Apply analytic survival and recovery through
   the response. Use only decision-time observations in the
   feature rows. Declare each fit's ordered fields, identities,
   currency and units before sampling. Active candidate fields
   include the signed short rate and effective hazard;
   economic and recoverable principal; identified locked or
   pending parent state, payable accrued and PIK; and the
   relevant cumulative MOIC cash or XIRR target PV when a
   return floor affects the right. Keep distinct parents
   distinguishable. Include relevant benchmark features only
   where qualified for this payoff and phase. A reduced
   declared schema approximates continuation conditional
   on its selected information. Establish feature adequacy
   separately from fit rank.
   [Accepted joint cash and survival](06-joint-coupon-replay.md#accepted-decision-for-checked-joint-bond-payoff-and-recovery-replay)
   and [Longstaff and Schwartz on continuation regression](https://galton.uchicago.edu/~mykland/346W05/Longstaff.pdf).

5. **Require one declared and fully admitted fit.** Elect one
   intercept, standardized linear or full standardized
   quadratic schema per active fit under the accepted regression
   contract. Use the full total-degree column set, including
   all interactions at degree two. Omit structurally irrelevant
   or common features only by the declared schema before
   sampling. With `p` columns require at least
   `max(2,p)` eligible independent base groups and at least
   `p` eligible leg rows. Pair legs can both supply rows
   while counting as one group. Freeze training-only scaling,
   the explicit relative rank threshold and positive SVD
   work limit. Use the checked full-rank solver for a
   multicolumn fit and the checked training mean for an
   expressly selected intercept. A sampled constant active
   feature, failed fit, insufficient groups or unrepresentable
   prediction is a contextual error. Remove automatic feature
   dropping, degree reduction, truncated singular directions,
   and mean or continue-only rescue rules. Use a separate
   structurally common origin expectation when the value is
   unknown at the origin. No regression is needed for a
   structurally known hold or benchmark. Report actual basis,
   rows, groups, columns, rank diagnostics and failures.
   [Accepted regression policy](../models-monte-carlo/24-regression-policies.md#accepted-decision-for-checked-continuation-regression)
   and [Glasserman and Yu on paths and basis size](https://doi.org/10.1214/1050516040000008461).

6. **Freeze a non-anticipating stopping rule.** At each
   decision use the current contractual exercise values and
   the frozen conditional hold estimate, or the exact hold
   when known. Compute the best payable put and call on a
   common basis and reject incompatible rights or unresolved
   priority. Apply holder put first. Call only if the post-put
   decision value strictly exceeds the full call strike plus
   the decision-only friction on exercise-phase principal.
   Use strict comparisons and keep hold at a tie with hold.
   Pay the actual chosen contractual redemption, or retain
   realized continuation on a hold path. Pay common cash once.
   Keep legally retained claims on their admitted branch basis.
   At an eligible common origin with unresolved hold, take
   the checked mean of independent continuation-training
   groups under the later policy, and freeze the root action
   before pricing. Use the independent benchmark-training mean
   for a required unresolved common-origin benchmark. Pricing
   draws never decide the origin action or update either fit.
   Preserve policy and payoff/source provenance across replay.
   [Put, call and friction contract](04-rates-tree-exercise.md#accepted-decision-for-checked-bond-rates-trees-and-exercise)
   and [joint exercise payoff](06-joint-coupon-replay.md#accepted-decision-for-checked-joint-bond-payoff-and-recovery-replay).

7. **Require explicit fixed-budget simulation and stable stage
   addresses.** For each required sampled stage, require its
   independent group count in `2..=10,000,000`, subject to
   fit and actual-work gates. Require explicit pairing,
   serial or supported native parallel execution, and one
   exact u64 root seed or explicit accepted instrument-ID and
   scenario-label derivation. Use Philox4x32-10 and the accepted
   indexed midpoint and reflected-leg sampler. Choose fixed
   base-ID ranges with starts 0 for benchmark training,
   10,000,000 for continuation training and 20,000,000 for
   pricing, within one selected root seed. Each stage's group
   offset starts at zero. Skipping a stage leaves its reserved
   addresses vacant. Antithetic legs share the same base ID
   and reflect the same step draw. A deterministic move may
   skip evaluation of the reserved slot. The terminal step
   draws nothing. Use the rate marginal for benchmark paths
   and the joint law for stopping and pricing. Keep stage,
   group, leg and step addressing stable across qualified common
   random number experiments, regardless of blocks or dispatch.
   Remove default counts, pairing and seed labels. No Sobol,
   Brownian bridge or budget changes are selected by this route.
   [Accepted joint sampler](../models-numerical/rates-credit-coupling-and-sampling.md),
   [random streams](../models-monte-carlo/02-random-streams.md)
   and [explicit simulation controls](../valuations-framework/model-resolution.md).

8. **Bound and reconcile actual replay work.** Check explicit
   limits for peak live bytes and work before allocation and
   draws, including stage samples, original and regenerated
   transitions, decision snapshots, parent/claim state,
   checkpoints, regression inputs, workspace and retained
   predictors. Use one deterministic bounded block plan on the
   validated shapes. Checkpoints bind the original factor
   address, exact joint/benchmark law, event and parent state
   and applicable history. Full and restarted paths reproduce
   the same decisions, values and checked errors on the
   qualified target. Blocks change storage and replay work
   while preserving rights, draws, sample counts and fit.
   Preserve stable
   aggregation and failure order by stage, group, leg and
   event/node under the accepted execution rules. A work,
   fit or data failure publishes no partial price or policy.
   Report actual grid, blocks, groups, physical trajectories
   and replay work separately. Rust owns private checkpoint,
   fit and random state. Existing Rust, Python, WASM and wire
   paths share checked controls, full-width seed identity and
   failure mapping. Reject unsupported parallel requests.
   [Deterministic execution](../../standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
   and [joint checkpoint identity](../models-numerical/rates-credit-coupling-and-sampling.md).

9. **Report fixed-budget policy value and conditional uncertainty.**
   Price using the independent pricing stage and frozen policies
   at the selected inputs. Form one checked time-zero payoff
   per leg on the native Money basis. Each complete antithetic
   pair contributes one group mean, and an unpaired leg one
   observation. Use the stable sample variance, standard error
   and two-sided 95% Student-t interval with `n−1` degrees
   of freedom for `n` independent pricing groups. Preserve
   the directly computed half-width. A constant realized
   sample gets a point interval under the shared estimator.
   The optional
   positive finite half-width tolerance is in PV currency and
   applies to the completed fixed run. A structurally exact
   payoff has no sampling interval. If a tolerance is supplied
   for such a payoff, report that sampling was unnecessary.
   Label the sampled interval as pricing uncertainty conditional
   on fixed inputs and the frozen benchmark and stopping
   policies, with nominal Student-t coverage approximate in
   general. It excludes regression, exercise approximation,
   benchmark, grid, calibration and model error. A fitted
   callable-puttable policy supplies no general one-sided
   bound on optimal value. Qualify economic error independently
   of sampling precision.
   [Accepted fixed-sample estimation](../models-monte-carlo/01-engine-and-estimation.md),
   [NIST Student-t interval](https://www.itl.nist.gov/div898/handbook/eda/section3/eda352.htm)
   and [Andersen and Broadie on separate valuation bounds](https://www.columbia.edu/~mnb2/broadie/Assets/primal_dual_ms_2004.pdf).

10. **Recompute the selected policy for OAS and affected risk.**
    Reuse only unaffected fit, projected state or benchmark
    policy. A spread trial changes risky continuation and
    recovery, and requires refitting the affected stopping
    policy and repricing at the same admitted group addresses.
    A reference-only or model shock rebuilds affected claims
    and fits. Keep historical state fixed under market shocks.
    Propagate fallible objective errors. Use the final fixed
    sample and the same admitted model basis to check the
    separately owned price residual and any final width gate.
    Root eligibility, sign changes, discontinuities and
    acceptance stay with record 12. A final price interval
    on reused draws at an inferred spread is a numerical
    diagnostic conditional on the selected run. It supplies
    neither uncertainty for the fitted OAS nor an independent
    holdout estimate. Preserve selected-model and contractual
    provenance for native price, quote, spread and risk callers.
    [Spread and inverse ownership](12-spread-quotes.md),
    [curve risk](15-curve-risk.md)
    and [embedded-option risk](18-embedded-option-risk.md).

## Observed behavior

### Choice, scope and controls

Exercise-contingent joint rates-credit value uses independent training and pricing samples. Default `mc_paths=20000` means independent estimators for each required stage, with antithetic pairs by default; at least two estimators are required. Instrument ID/scenario label seeds mix with distinct training, make-whole-training and pricing domains. Native parallel and WASM serial evaluation collect in stable path order. No Sobol or Brownian-bridge sampler is selected by this bond route.

### Algorithm

1. Generate decision snapshots at origin, exercise dates, terminal and redemption dates from record 06. Eight state features are short rate, hazard, outstanding, locked term-coupon amount, accrued cash, accrued PIK, cumulative distributions and cumulative target-discounted distributions.
2. For stochastic-rate make-whole calls, first train a separate conditional-value policy for remaining promised cash. Roll future reference cash plus principal with path rate discounts multiplied by deterministic reference-curve/base-curve interval ratios and reference spread. Exclude hazard and OAS; set hazard feature to zero. Deterministic-rate make-wholes instead use deterministic reference PV. Missing training policies use contractual price only while constructing training data, not as final pricing fallback.
3. Work backward through exercise decisions. Regress affine realized continuation from later trained policy on the eight current features using standardized quadratic polynomials including interactions. Fit over all training paths, not an in-the-money-only subset. The shared fitter drops constant dimensions; when there are too few samples, a solver fails or fitted coefficients are nonfinite, it tries quadratic without interactions, linear and intercept-only. Rank deficiency alone does not force a smaller basis: truncated SVD can retain the full basis with unresolved singular directions zeroed; see the [shared regression-policy record](../models-monte-carlo/24-regression-policies.md). Nonfinite data errors; there is no look-ahead use of realized continuation as a missing-policy replacement.
4. Apply holder put first, then issuer call if the post-put estimated continuation exceeds call plus friction. Training/pricing retains the realized future payoff when policy chooses hold. Current cash is added outside the exercise decision. Incompatible put/call barriers error. Terminal redemption is an exact known hold claim, requiring no continuation regression.
5. Freeze policies and evaluate the independent pricing sample. Average each primary/antithetic pair into one estimator, accumulate `OnlineStats` and report mean, standard error, CI and physical/stage counts. A requested 95% CI half-width is a final fixed-budget acceptance gate; failure errors rather than increasing paths automatically.

Training checkpoint blocks change storage only: use one block when decisions×physical paths fit `2^20` snapshots, otherwise blocks of about square-root decision count, retaining factor and product state. Replaying blocks must reproduce full-path events; path budgets are not truncated.

### Limits, alternatives and located evidence

Pricing confidence intervals condition on the fitted policy and do not measure regression, calibration, exercise-grid or model error. Make-whole regression also adds approximation error. Cubic refinement is supported by the shared fitter/tests but production selects quadratic; alternative nested conditional simulation or richer bases remain unadopted. OAS trial solves reuse seed domains and disable the final CI gate until the candidate spread is found.

Owning code: [stage orchestration](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/mod.rs), [policies](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/policy.rs), [snapshots/replay](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/replay.rs), [polynomial fitter](../../../../finstack-quant/models/src/monte_carlo/pricer/polynomial.rs). [LSMC tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/tests.rs) contain quadratic-surface recovery, cubic refinement, intercept fallback, independent-estimator accounting, stage reproducibility and block/full replay equality. These are manufactured-surface and structural checks. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) and [Longstaff and Schwartz](../../../REFERENCES.md#longstaff-schwartz-holder-stopping-and-independent-policy-pricing) provide general method provenance.

## Independent arithmetic and qualification

- At a common decision state, two equally likely future hold
  cash values of 60 and 140 give conditional hold of 100.
  With put 80 and call 110, the frozen action holds and
  realizes the original 60 and 140, giving mean 100. Looking
  ahead at each realized hold would select 80 and 110 and
  give 95.
- From the root, two equiprobable unspread path discount
  factors .80 and 1.00 have achieved base .90. Discount
  fixed cash 110 using reference DF .96 and zero reference
  spread. The normalized responses are about 93.86666667
  and 117.33333333. Their mean is 105.60. With clean floor
  100, accrued deduction 3 and payable accrued 2, the
  lawful dirty strike under this law is 104.60.
  Averaging the two floor-applied path strikes instead gives
  about 109.16666667. An active dirty return floor of 106
  raises the 104.60 strike to 106.
- Two active coordinates and full quadratic give six columns.
  Four independent antithetic group IDs supply eight legs but
  fail the six-group coverage gate. Separately, paired pricing
  legs `(2,6)` and `(0,4)` give independent observations
  4 and 2, standard error 1, and a 95% interval centered
  at 3 with half-width about 12.70620474.
- A call at 100 with true continuation 110 can be missed
  by a fitted 95 hold, producing 110. A put at 80 with
  true continuation 70 can be missed by a fitted 85 hold,
  producing 70. This verifies opposite error directions
  when the omitted decision changes sides.

Qualify independent finite-law exercise and reference-strip
benchmarks, origin and signed terminal cases, lagged cash and
retained claims, basis and conditioning errors, out-of-sample
pricing, exact stream separation, blocked replay and resource
failure, and residual and risk experiments. Compare grid
and training refinement independently of pricing-sample
width. Test the admitted contract on actual Rust and host paths.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
