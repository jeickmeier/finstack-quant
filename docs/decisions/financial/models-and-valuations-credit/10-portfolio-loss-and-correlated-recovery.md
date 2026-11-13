# MODELS-AND-VALUATIONS-CREDIT-10: Portfolio loss simulation, tail statistics and correlated recovery

- Decision status: **Accepted — complete checked portfolio loss and correlated recovery package, 2026-11-12**.
- Implementation status: Current behavior observed below. The accepted contract requires Rust, host and product reconciliation.
- Validation status: Source and selected tests inspected. Independent algebra and integration examples evaluated. Production, host, calibration and sampling qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked portfolio loss and correlated recovery

1. **Model one fixed horizon with explicit inputs.** The caller supplies the
   same horizon and probability measure for all name PDs, an explicit portfolio
   law, literal u64 seed (zero is valid), path count in `1..=1,000,000`,
   confidence in `(0,1)`, and a severity selection. Each exposure has a
   nonempty already-trimmed unique ID (literal case-sensitive UTF-8), finite
   nonnegative notional in one common caller-supplied unit, finite PD in
   `[0,1]` and a finite signed loading vector of the same positive dimension
   as every other exposure. The caller fixes and preserves factor coordinate
   order. Validate all supplied data, chosen models and required addresses
   before producing any paths, including for zero notional or zero PD.
   An empty portfolio with valid controls and severity returns the requested
   number of +0 losses, without draws. A recovery coordinate must be in range
   for a nonempty portfolio; an empty roster has no factor dimension to check.
   This is a static loss model: no FX, discounting, exposure evolution,
   default-time or migration model, cash settlement, or implied horizon or
   probability measure.

2. **Use one direct loading-vector law for each selected family.** Support
   explicit Gaussian and Student-t (`2<ν<=1e8`) selections. Retire the
   portfolio-specific `MultiFactor` alias and reject other families in this
   route; the shared sector-factor law and RFL remain in
   [record 09](09-copula-kernels.md#accepted-decision-for-checked-copula-laws-and-latent-factors).
   For independent standard normals `Z` in authored factor order and
   independent name residuals `ε_i`, form
   `X_i=β_i·Z+sqrt(1−Σβ_ij²) ε_i`. The pre-mixture pair correlation is
   `β_i·β_j`; preserve signs and original coordinates. Compare the exact
   sum of squares of represented loadings with one. Reject over-unit
   vectors without a tolerance, projection or normalization. A zero vector
   uses only the residual; an exact unit vector uses only the systematic
   dot product. Obtain a positive near-unit complement before rounding
   the norm to one, with a scaled square root where needed. Use checked
   dot products and affine arithmetic. No divide-by-norm and multiply-back
   round trip or flooring small positive residuals.

   Use literal no-default and certain-default branches at PD zero and one.
   For interior PD, Gaussian defaults at `X_i<=Φ⁻¹(p_i)`. Student-t defaults
   at `X_i<=t_ν⁻¹(p_i) sqrt(W)` for one positive finite `W=χ²_ν/ν` shared by
   the whole path. Use the checked record-09 thresholds, mixing and scaled
   comparison. Residuals, common normals and the mixing draw have distinct
   logical roles. Zero loading does not remove Student-t dependence from
   the shared scale. The finite midpoint normal generator limits reachable
   shocks. For example, Gaussian zero loading with a threshold below the
   negative maximum normal radius can yield no defaults despite a positive
   input PD. [Demarta and McNeil](../../../REFERENCES.md#demarta-mcneil-2005-t-copula).

3. **Separate fixed severity, recovery and default factors.** Select one
   mutually exclusive severity mode: a required per-name fixed LGD in
   `[0,1]`, or one explicit common recovery specification evaluated for
   each default. The common-recovery mode takes no fixed per-name LGD
   inputs. Select the common factor coordinate explicitly when stochastic
   recovery has nonzero systematic loading. Independent recovery
   (`a=0`) and constant recovery have no coordinate. Sample using the
   named coordinate of the unscaled common Gaussian vector `Z`, also
   for Student-t, and an independent recovery normal for that name when
   needed. No use of the normalized default projection or implicit first
   factor. A name with zero default loading may share a recovery factor
   with other names if the caller selected it. Its own default and
   recovery are independent in the ideal law because its default uses
   only its default residual and, for Student-t, the independent scale.
   Recovery can still depend on the default of other names.

4. **Make the logistic-normal parameters and exact cases explicit.** Keep
   `Constant(rate)` for any finite rate in `[0,1]`, and a stochastic
   `LogisticNormal(center, logit_scale, systematic_loading)` with
   `0<center<1`, finite `logit_scale=s>0` and signed `a∈[-1,1]`.
   Deterministic choices use the constant variant. Remove the generic
   40% default, the named market-standard stochastic preset and the legacy local
   probability-scale parameter and cap. Compute
   `R(z,e)=sigmoid(logit(center)+s[a z+sqrt((1−a)(1+a)) e])`, where
   `logit(center)=ln(center)−log1p(−center)`. The center is the zero-shock
   recovery and unconditional median. The scale is in logit units and
   `a` is a latent Gaussian loading, not realized recovery SD or
   correlation. Return the exact center at zero displacement and use the
   exact inactive-component branches for `a=0,+1,−1`. Validate every
   supplied shock even for inactive components. Evaluate tails and
   products with stable scaling. Where the logit or its displacement
   exceeds working range, use a proven endpoint rounding or fail the
   computation; a spurious intermediate overflow cannot justify saturation.
   Compute LGD from the complementary sigmoid. Carry the notional into
   severity scaling before an intermediate tail rounds to zero. Distinguish
   true zero severity from a positive loss amount that cannot be
   represented as a positive binary64 result.

5. **Keep checked recovery expectations for the existing consumers.** The
   conditional mean holds the stated factor fixed and integrates only the
   recovery residual. Unconditionally, the combined shock is standard
   normal and the mean is independent of `a`. For affine logit
   `B+dε`, conditional `B=logit(center)+s a z`,
   `d=s sqrt((1−a)(1+a))`; unconditional `B=logit(center), d=s`.
   Constants, exact `d=0`, and exact `B=0` have their model branches; at
   `a=0` the conditional and unconditional means agree. Evaluate the
   smaller expectation using sign symmetry. For `0<d<=1` integrate
   `φ(x)[S(−|B|+dx)+S(−|B|−dx)]` over `[0,10]`, with `S` the
   sigmoid. For `d>1` integrate
   `S′(x)[Φ((−|B|+x)/d)+Φ((−|B|−x)/d)]` over `[0,36]`. The latter
   identity follows by integrating the sigmoid as a logistic CDF.
   Compute normalized arguments in scaled form and use the checked
   [CORE-NUMERICS-04](../core-numerics/quadrature.md#accepted-decision-checked-quadrature-budgets-and-measures)
   adaptive Simpson rule: absolute estimator tolerance `1e−12`, depth
   24 and at most 100,000 integrand evaluations per expectation. The
   omitted mass is bounded by `2Φ(−10)` and `2S(−36)` respectively.
   The estimator and truncation bounds do not certify true quadrature
   error or relative accuracy in tiny tails. Reject invalid estimates
   and work exhaustion without clipping or substituting the center.
   Obtain the larger complementary expectation from the evaluated
   smaller one. Expected recovery conditional on a default is a different
   measure. In general `E[L_i]=notional_i E[1_{D_i}(1−R_i)]`; using
   `PD_i(1−E[R_i])` assumes independence that the common factors may
   violate. Empirical evidence supports considering dependence, without
   calibrating this law or any input.
   [Altman et al.](../../../REFERENCES.md#altman-et-al-2005-recovery).

6. **Address draws by path and role.** Use the accepted
   [Philox profile](../models-monte-carlo/02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation)
   with the literal seed as key. Derive each stream ID by the accepted
   FNV-1a pair function on (identity, role). The role labels, all prefixed
   `portfolio-loss/v1/`, are `shared-mixing`, `systematic-factor`,
   `idiosyncratic-default` and `idiosyncratic-recovery`. The identity
   is empty for mixing, the zero-based factor index as canonical ASCII
   decimal for systematic normals, and the literal exposure ID for name
   roles. Reserve and check distinct streams before use for every name
   in both roles, every factor and the shared mixing slot even if a
   selected branch uses no draw there. Collisions fail, without rehash
   or replacement. In each stream use Philox block `path_index` (the
   two raw uniform slots starting at `2*path_index`). Use the first
   open uniform for the mixing inversion and the first reflected
   Box–Muller component from both uniforms for each normal. Do not use
   the companion in another slot. This aligns surviving name draws
   across roster and severity changes with the same seed, path, ID,
   factor coordinates and roles. It does not establish independence of
   finite pseudo-random streams or promise the old sequence. Native
   serial and parallel runs follow the same operation order per path
   and publish ascending path indices. Pin integer words and uniform
   midpoints across targets. Qualify transformed results under the
   existing cross-target tolerance rules and test threshold-near cases
   explicitly, since a changed comparison can change a whole name loss.

7. **Sum checked nonnegative path losses and fail atomically.** For each
   default, compute its checked severity amount. Sum in ascending
   lexicographic UTF-8 byte order of IDs with checked, compensated
   arithmetic. Keep exact zero contributions and canonical +0. No
   saturation, missed draws on a different logical role, discarded
   positive contributions through premature product underflow, or
   partial published sample. Invalid inputs, arithmetic and allocation
   failures propagate. After successful admission, report the lowest
   failing path index regardless of worker completion order. Preserve
   original path order, including ties and zeros.

8. **Define the empirical loss estimators on one sample.** The supplied
   sample constructor and restoration accept the same cap as simulation,
   finite nonnegative ordered losses and an explicit finite
   `0<c<1`. Normalize signed zeros and keep original order. Arithmetic
   mean estimates expected loss. For the ascending order statistics
   `x`, let `r=Nc`, `k=ceil(r)`, `m=N−r`. VaR is `x[k−1]`
   (zero-based). ES is
   `(Σ_{j=k}^{N−1} x[j]+(k−r)x[k−1])/m`. Interpret the represented
   binary64 confidence exactly for rank and boundary mass, as in the
   [analytics probability convention](../analytics/07-historical-tail-risk.md);
   this loss quantile uses the inverse empirical CDF rather than
   interpolation. Use the same exact rank and mass geometry for both
   statistics. Ties consume only the mass required, and an exact rank
   boundary has zero fractional VaR weight. If `m<1` the ES is the
   sample maximum; a one-observation or constant sample returns that
   value for all three estimators. Compute scaled checked means and
   weighted tails within the sample support, with `ES>=VaR`. Reject
   arithmetic failure or positive results lost to underflow. No
   confidence floor or output repair. These are positive-loss measures
   in the sample's unit. No CI, convergence control or claim about
   extrapolation beyond the finite sample or generator resolution.
   [Rockafellar and Uryasev](../../../REFERENCES.md#rockafellarUryasevGeneralLoss).

9. **Compute static tranche summaries from the same path sample.** Require
   explicitly supplied same-unit `P>0` and finite fractions
   `0<=A<D<=1`. Reject any sample loss outside `[0,P]` for this
   operation. Let `N_tr=P(D−A)` be the original tranche notional. The
   path amount is `min(max(L−PA,0),N_tr)`, and the path fraction is
   that amount divided by `N_tr`. Compare the represented loss against
   the exact products `PA` and `PD` for the clipping cases and
   frequency counts: `L>PA` breaches, `L>=PD` fully writes down.
   Compare in amount units before rounding any displayed fraction;
   a fraction that rounds to one does not establish a detachment hit.
   Use scaled differences and ratios to preserve thin tranches and
   small positive amounts, with checked positive representability of
   tranche notional and required positive outputs. Aggregate amounts
   and fractions from the same underlying payoff at the portfolio
   result's confidence and divide each event count by `N`. No implicit
   rescaling of the pool, fallback notional, product settlement rule
   or percentage-point interpretation.

10. **Keep one Rust authority and the existing host routes.** Retain one
    checked simulator, one checked constructor for supplied losses,
    one tranche summary operation and checked recovery evaluation on
    existing routes. Build immutable validated state atomically.
    Restore portfolio result snapshots from ordered losses and confidence
    alone, with aggregates recomputed in Rust and unknown fields rejected.
    This snapshot is a sample, not a replay request. Exact replay needs
    the frozen complete selected inputs, factor order and profile. Tranche
    statistics are derived output only; remove independent restoration
    of unverified aggregate fields. Reconcile Python and WASM constructors,
    errors, stubs, exports, parity, wire schemas, tables and examples.
    Preserve exact Python integer seeds, JS safe integer or BigInt input
    and exact Rust-parsed raw JSON integer tokens. Reject lossy JS
    numbers. Hosts convert shapes and report Rust results.
    Structured credit must supply the recovery law explicitly or
    construct a constant from the deal's validated contractual rate
    where that product already selects it. Remove the generic scenario
    tree default and reconcile conditional-recovery and sampled-recovery
    callers. Product factor dynamics, normalization, aggregation,
    eligibility, default periods, recovery payout timing, antithetics
    and settlement stay with their owners. Preserve the distinct
    [credit workout and beta recovery](06-recovery-and-lgd.md),
    [pool defaults](07-pool-defaults.md) and
    [dated CDS tranche loss](17-tranche-loss-distribution.md) decisions.
    This package does not select calibration data, regulatory capital
    measures, or the law a particular portfolio should use.

## Observed behavior

### Choice and rationale

Independent path streams simulate correlated obligor defaults and optional logistic-normal recoveries; empirical loss VaR/ES and tranche clipping summarize simulated distributions.

### Scope and input domain

Nonnegative finite exposure notional, probability/LGD in [0,1], unique trimmed IDs, equal nonempty factor-loading dimensions and squared norm≤1 (with epsilon tolerance). Positive bounded path count and confidence strictly between zero and one. RFL is rejected; MultiFactor requires two loadings.

### Algorithm

1. Each path uses Philox(seed,path index), one shared Student-t mixing draw where selected and independent standard-normal systematic factors. For name i form normalized loading dot product, rho=loading squared norm, add idiosyncratic copula shock, and default when latent≤marginal threshold.
2. Loss sums notional×LGD for defaulted names. Without a recovery model use exposure LGD; with it sample recovery using normalized systematic projection (or first factor for zero loading) and an independent recovery normal when needed. Recovery shocks are drawn consistently for every name before checking default.
3. Correlated recovery is sigmoid(logit(m)+[vol/(m(1−m))]×[corr×Z+√(1−corr²)ε]). Endpoints/zero volatility are deterministic. The supplied mean parameter is the zero-shock center; actual expectation is numerically integrated and cached. Conditional expectation integrates residual normal risk. Adaptive Simpson uses tolerance 1e−12/depth24 with a normal correction on [0,10] for small scaled variance and a logistic-density transformation on [0,36] for large variance.
4. Loss mean uses scaled compensated summation. VaR uses the ascending order statistic ceil(confidence×N)−1 after rounded multiplication. ES uses a separately rounded (1−confidence)×N upper-tail mass, including fractional weight on the boundary observation, and caps its normalized result at one. Tranche losses clip `(loss/poolNotional−A)` to [0,D−A] and divide by width; attachment breach uses >A and full writedown ≥D after the initial pool-fraction division.
5. Native parallel paths preserve deterministic indexed streams and report the lowest-index failure; WASM fills serially. Recovery draws are interleaved with name default draws. Deserialization rebuilds and exactly compares carried aggregate statistics, while Rust result fields are public.

### Selection and controls

Caller supplies copula, path count, seed, confidence, factor loadings and optional RecoverySpec. RecoverySpec default is constant .40; named stochastic preset centers .40, vol .25, correlation .40. No confidence-interval calculation or automatic simulation convergence control is selected.

### Conventions

Exposure losses are money amounts in a common external unit; portfolio tranche A/D are fractions, unlike CDS tranche percent inputs. Positive recovery correlation lowers recovery in adverse negative systematic states; logistic-normal volatility parameter is a local scale, not exact output standard deviation.

### Limitations, failure and fallback

No temporal default schedule, discounting, contagion or obligor-state migration is modeled. Unconditional expected recovery differs from recovery conditional on default. Copula MultiFactor in this simulator uses supplied loadings, not the observed kernel’s fixed 60/40 decomposition. Zero-loading exposures can still have common recovery-factor dependence.

### Alternatives and tradeoffs

Semi-analytic finite-pool convolution is supported elsewhere for tranches; importance sampling, QMC and rare-event estimators are unimplemented here. Mean-centered calibrated beta recovery is a separate LGD model, not the same stochastic law.

## Evidence

Owners: [portfolio_loss.rs](../../../../finstack-quant/models/src/correlation/portfolio_loss.rs) and [Recovery implementations](../../../../finstack-quant/models/src/correlation/recovery). [portfolio_loss.rs tests](../../../../finstack-quant/models/tests/portfolio_loss.rs) and inline tests locate deterministic streams, formulas/moments and fractional tail-mass checks; they are not a portfolio loss model validation. References: [QRM](../../../REFERENCES.md#mcneil-frey-embrechts-qrm), [recovery dependence](../../../REFERENCES.md#altman-et-al-2005-recovery), [Student-t mixtures](../../../REFERENCES.md#demarta-mcneil-2005-t-copula) and [empirical expected shortfall](../../../REFERENCES.md#rockafellarUryasevGeneralLoss).

Current routes: [Python](../../../../finstack-quant-py/src/bindings/models/correlation/mod.rs),
[WASM](../../../../finstack-quant-wasm/src/api/models/correlation.rs),
[models wire schemas](../../../../finstack-quant/models/src/schema.rs),
[structured-credit selection](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pricing_methods.rs),
[conditional recovery in the scenario pricer](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/stochastic/pricer/engine.rs)
and [sampled recovery in pool paths](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flow_source.rs).

### Independent review evidence

- Exact rational arithmetic gives a squared loading norm for binary64
  `(.6,.8)` that exceeds one by about `4.44e−17` even though it rounds
  to one. Conversely `(nextDown(1),nextDown(2^-26))` has positive residual
  variance about `3.697785493e−32` even though its norm also rounds to one.
- For Student-t with `ν=5`, two zero-loading names at marginal threshold
  `−1` have marginal PD about `.1816087338` and joint default about
  `.03868053147`, compared with the independent product `.03298173220`.
  Independent Simpson integration of the common-scale law gives these
  values. The shared scale preserves dependence even in this case.
- Independent Gaussian quadrature with center `.40`, logit scale
  `25/24`, recovery loading `.40`, default loading `.60` and PD
  `.20` gives unconditional recovery about `.41793411`, recovery
  conditional on factor zero about `.41579597` and recovery conditional
  on default about `.34905192`. Changing only the recovery loading to
  `−.40` leaves the first two means unchanged and gives about `.48857309`
  on default. At logit `+800`, high precision evaluation gives about
  `3.66787458418e−48` in loss on notional `1e300` even when binary64
  recovery rounds to one and binary64 standalone LGD rounds to zero.
- With losses `[0,0,4,4,10]` and confidence `.75`, exact empirical
  VaR is `4` and ES is `8.8`. For `[0,2,4,8]` at the same confidence,
  VaR is `4` and ES is `8` with zero weight on the VaR observation.
  With nine zeros and one `100`, binary64 `.9` is strictly greater
  than `9/10`, so both VaR and ES are `100`; the rounded product
  `.9*10` would select VaR zero.
- At pool notional `100`, binary64 attachment `.03` puts loss `3`
  above attachment by `2^-53`. Binary64 detachment `.1` puts loss
  `10` below detachment by `5*2^-53`. Rounded division would report
  equality in both comparisons.
- An independent integer Philox calculation for ID `A`, seed `42`
  and path `1` gives stream `0x7fa8cd3f945ece29` and first word
  `0x5319075c` for the default residual, and stream
  `0x2f0521a180c42bab` and first word `0xc64b37c7` for recovery.
- These checks exercised formulas and boundary decisions independently
  of the production kernels. Seed/profile replay, numerical tail and
  quadrature convergence, host parity, structured-credit integration,
  and calibration or empirical qualification remain open.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
