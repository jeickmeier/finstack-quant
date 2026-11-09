# CORE-NUMERICS-09: Compensated aggregation, online moments and R7 quantiles

- Decision status: **Accepted — complete aggregation/moments/R7/mean-uncertainty package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Source inspected; independent exact-rational/scalar-expression examples and semantic reviews passed**. Production shifted/scaled state, host/caller and statistical qualification remain pending; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

This record owns ordered compensated sums, finite descriptive moments,
online update/merge state, paired covariance/Pearson/control-variate ratios,
R7 quantiles, explicit finite filters and Student-t mean uncertainty in
math/summation.rs and the corresponding parts of math/stats.rs.
[Record 10](realized-variance.md) retains adjacent-price log_returns and realized
variance/annualization decisions; [record 12](distribution-functions.md) retains
general distribution-function/inverse policies. Accepted analytics definitions,
NPV accumulation and financial/Monte Carlo consumer policies remain separately owned.

## Accepted decision: checked sums, moments, quantiles and mean uncertainty

Accepted on **2026-11-08** after whole-record review. Runtime,
caller/host reconciliation and independent numerical/statistical qualification
remain pending; observed source behavior is retained separately below.

1. **Require literal finite equally weighted observations and explicit pairing.**
   Ordinary moments and quantiles validate every supplied observation, not silently
   drop NaN/infinity. Paired APIs require equal lengths and indexwise pairs; no
   implicit truncation, pairwise filtering, date alignment or weights. Validate
   shape, values and probability/control domains before availability/endpoint
   shortcuts, including a singleton NaN. Values retain caller units; covariance
   has product units, variance squared units and correlation is dimensionless.
   No trading-date inference, annualization, ddof selector, robust/weighted mode
   or new missing-data policy framework is selected.

2. **Use one explicit availability/error contract and remove duplicate families.**
   Canonical scalar estimators return `Result<Option<f64>>`: Some is a finite
   available estimate, None is the defined insufficient/degenerate case below,
   and Err is invalid input or failed numerical calculation. mean_var returns
   `Result<(Option<f64>,Option<f64>)>` with fieldwise availability; a requested
   calculation failure still errors the call. CI uses `Result<Option<(f64,f64)>>`.
   Remove the zero-for-insufficient and parallel _or_nan APIs, replacing the
   needed sample_std_or_nan/median_or_nan names by std_dev/median. Do not retain
   compatibility wrappers. Online accessors use get_* names, including get_count,
   get_mean/get_variance/get_population_variance/get_std_dev/get_stderr,
   get_confidence_interval/get_ci_half_width and corresponding paired getters.
   Host absence maps to None/null, never fabricated zero or successful NaN.

3. **Retain two explicitly selected checked ordered summation algorithms.**
   kahan_sum and neumaier_sum return `Result<f64>`, preserve source order and return
   +0 for empty input. Keep their distinct compensation recurrences, not an
   automatic switch, sorting/reduction tree or exact accumulator. Validate each
   incoming scalar and required intermediate/result; stop at the first failure.
   NeumaierAccumulator keeps private finite sum/correction, valid new/Default empty
   identity, add->Result with atomic state commit, and borrowing get_total->Result
   without resetting the state. Remove
   the ambiguous consuming total alias; computed zero may normalize to+0.
   Preserve finite signed/subnormal results. Ordered intermediate overflow can
   fail even if the exact final sum fits; compensation does not guarantee exact
   cancellation, faithful rounding or one algorithm's universal superiority.
   Already accepted NPV's ordered Neumaier choice is unchanged. A prior caller
   product already rounded to zero is not detectable from the scalar zero supplied.

4. **Fix the estimators, denominators and availability separately.** Let n be
   the observation count, mu the arithmetic mean, M2=sum((x-mu)^2), and
   C=sum((x-mu_x)*(y-mu_y)). Mean is available for n>=1; population variance M2/n
   for n>=1, including singleton0; sample variance M2/(n-1), sample SD and sample
   covariance C/(n-1) require n>=2. Smaller histories return None, not zero.
   A genuine constant eligible sample has exact zero variance/SD/covariance.
   No epsilon-based constant classification. IID sampling with finite second
   moments is sufficient for unbiased sample variance/covariance; other sampling
   designs require qualification. The formula itself does not establish those
   assumptions or make sample SD unbiased.

5. **Keep Welford/Chan algebra in a private shifted/scaled centered state.**
   OnlineStats and OnlineCovariance retain O(1) state, valid new/Default/reset
   empty identities and Clone. Store centered means in shifted coordinates and
   use private scaling/compensation for centered square/cross moments, without
   repeatedly rounding an internal mean back to a large global offset. For an
   update use the real equivalent weighted increment delta²*n_old/n_new for M2,
   and delta_x*delta_y*n_old/n_new for C, with the corresponding weighted mean
   update and scaled products. Do not use delta*(x-rounded_new_mean) to decide
   a varying sample has zero spread. Preserve per-axis evidence that all actual
   finite observations are identical, separately from computed small/zero moments.
   Do not require raw unscaled M2, C, scale products or an unrequested variance to
   fit binary64. This is one internal representation, not a public scale/cache/
   arbitrary-precision framework or guarantee for every finite sequence.

6. **Make update/merge fallible, atomic and exact in count admission.**
   update and merge return Result; validate all inputs and compute/check the full
   candidate state, including origins/scales/compensation/count, before commit.
   A failed update/merge leaves the original state usable. Keep usize counts with
   checked addition and cap estimator counts at min(usize::MAX,2^53), so integer
   counts/n-1 convert exactly before ordinary floating weights are formed.
   Do not wrap, saturate or silently approximate a larger count. For groups a,b,
   N=na+nb, w=nb/N, q=na*(nb/N), merged mean is a stable convex combination,
   merged M2=M2a+M2b+q*(mu_b-mu_a)^2 and cross moment is
   Ca+Cb+q*delta_x*delta_y. Use unrounded shifted means/scaled products, not
   na*mu_a+nb*mu_b or an overflowing raw delta-square/count product. Empty merge
   is identity; reset restores that state. Merge concatenates the represented
   observations; callers own disjointness/independence. No dedup/merge provenance
   scheme, new state loader/serde or public mutable/from_parts bypass.

7. **Evaluate requested quantities without unnecessary range prerequisites.**
   Batch mean uses a canonical scaled compensated mean (Neumaier in the chosen
   normalized coordinates), not a requirement that raw sum(x) or M2 fit. mean_var's
   mean uses that same canonical batch calculation rather than a second API's
   distinct Welford mean. Batch moment fields use the same centered estimators
   as the online algebra; separate passes are allowed rather than force a false
   one-pass identity. Return a requested SD/SE directly from scaled moments;
   do not require its squared variance first, or materialized SD before SE.
   Restore exponents/divisors together
   before materializing covariance/beta scale products/ratios. Invalid data errors
   before availability; a genuinely unrepresentable requested nonzero output or
   detectably unusable state is a computation/range error, not None, zero or NaN.
   Ordinary rounding loss is not claimed generally detectable or eliminated.

8. **Define Pearson correlation and control beta without degenerate defaults.**
   Pearson requires n>=2 and both histories genuinely varying; otherwise None.
   Compute C/(sqrt(M2x)_sqrt(M2y)) using scale-normalized ratios without forming
   unscaled variances or their product. Return only finite values in[-1,1]; an
   out-of-range computed value, even a small excursion, is a numerical failure,
   not invalid observations or a silently clamped estimate. No blanket clamp,
   epsilon variance floor or underflow-as-constant rule. Control-variate beta is
   C/M2y for n>=2 and genuinely varying Y; constant control gives None because
   the coefficient is not identified, while constant X/varying Y can give0.
   Its sign corresponds to X-beta_(Y-E[Y]), not a financial regression/hedge
   mandate. A consumer may explicitly select no adjustment; core does not infer it.

9. **Retain one generic R7 quantile definition with checked rank arithmetic.**
   Require finite p in[0,1], independently of analytics tail-confidence domains.
   For sorted zero-based observations, h=(n-1)*p, j=floor(h), f=h-j and
   Q=(1-f)*x_j+f*x_(j+1); n=0 gives None and n=1 the supplied value. Validate
   p/all data first; p=0/1 and exact order-statistic/constant-support cases preserve
   their observations. Apply the same exact-binary64-input rank-residual and
   stable convex-interpolation contract already accepted in
   [analytics 07](../analytics/07-historical-tail-risk.md#accepted-decision-historical-tail-numerical-evaluation-and-failures): no
   lost consequential fractional influence from rounded near-integer rank,
   unsafe endpoint subtraction or false zero underflow. Estimator count admission
   is capped as in decision6; indices are checked. No probability fuzz/clamp,
   epsilon rank snapping, alternate quantile type or vendor-default inference.

10. **Keep mutable and sorted quantiles as checked forms of that estimator.**
    quantile takes mutable scratch and may retain efficient partial selection;
    validate all inputs before reordering. Later numerical failure may leave
    scratch permuted, and callers must not treat that as committed financial
    state. quantile_sorted validates the entire finite nondecreasing sequence
    on every public call, not just selected entries. It remains immutable, even
    though validation is O(n). median delegates to R7 p=.5 on a scratch copy.
    No unchecked public sorted alias, cached-order object, extra estimator family
    or different missing-data behavior. Existing statrs quantiles are not a
    silent replacement because their selected estimator differs.

11. **Keep finite filters explicitly separate from ordinary estimators.**
    finite_count deliberately excludes NaN/±infinity and returns the exact usize
    count. Rename finite_min_or_nan/finite_max_or_nan to finite_min/finite_max,
    returning `Option<f64>`: None if no finite entry survives. These filtering
    helpers need no numerical Result/estimator-minimum rule; they do not validate
    a statistic, establish independent observations, align pairs or authorize
    dropping data for mean/variance/covariance. No filtering estimator variants.

12. **Select one explicit Student-t interval for an equally weighted mean.**
    For n>=2, SE=s/sqrt(n) using sample SD. Two-sided confidence1-alpha has
    half-width h=-t_inv(alpha/2,n-1)*SE and bounds mu±h. Keep Student-t throughout;
    no automatic normal switch. Both get_confidence_interval(alpha) and
    get_ci_half_width(alpha) require finite 0<alpha<1, validated before None or
    constant shortcuts. Remove the hidden .05/95% half-width choice. Compute
    half-width directly, not from rounded endpoints. Genuine constant eligible
    samples give SE0/h0/point bounds without an unneeded critical-value query;
    this is not evidence that the population mean is known without uncertainty.

13. **Make interval numerical and inferential limits explicit.** Preserve
    lower-tail inversion to avoid rounding1-alpha/2 to1. Resolve required tiny-tail
    arithmetic stably or report computation/range failure; do not clamp alpha,
    accept alpha/2 rounded0 or swallow inverse failure as missing history.
    Positive SE/half-width may not underflow to a false zero. With positive h,
    require finite bounds strictly on each side of the returned mean; collapsed
    endpoints or overflow fail the interval, while a representable standalone
    half-width/SE can remain available. No bounds clipping to observation support.
    Exact Student-t coverage requires a fixed-count IID nondegenerate Gaussian model;
    otherwise approximation/finite-variance assumptions need owner qualification.
    Antithetic pair averages and independent randomized-QMC replicate means, not
    their dependent constituent draws, are the relevant units when selected by
    that estimator. No autocorrelation correction, optional-stopping coverage,
    weighted-replicate rule or confidence-sequence method is adopted here.
    General inverse-CDF endpoint/saturation policies remain with record12.

14. **Reconcile callers and existing host surfaces without changing owners.**
    Propagate Result/Option signatures, canonical names, getter renames and explicit
    alpha through Rust, tests, benchmarks, PyO3/stubs, WASM exports/declarations,
    examples and parity in the same implementation slice. Keep actual Rust-only
    online state/CI helpers unbound; no new host accumulator classes. Host ordinary
    estimates map None/null and genuine errors through existing centralized input
    ValueError/domain versus computation RuntimeError and WASM categories, with
    no local zero/NaN adapters. Existing analytics availability/identities,
    minimum-history, correlation endpoint/Gram-matrix rules and accepted NPV
    accumulation stay authoritative at their owners. Model Monte Carlo ci_95
    labels, stopping, control-beta omission, replicate weighting and product
    confidence policy remain separate choices; remove implicit core .05 without
    silently making explicitly labelled product intervals configurable.

15. **Qualify accuracy, reproducibility and state validity honestly.** Shared
    routines preserve the declared observation/merge order, but different merge
    trees/batch paths, floating rounding, elementary functions and targets need
    not give identical bits. Remove unconditional cross-platform/market-vendor
    equivalence, cancellation-free and universal scale-invariance/ULP claims.
    Neither computed zero in a compensated sum nor finite moments certify exact
    real cancellation/optimality, and count limits do not make floating weights
    exact. Private state checks and bounded shifted/scaled reference examples
    do not establish all-domain numerical or confidence coverage. No arbitrary
    epsilon repair of negative moments, PSD/correlation matrices or failed
    calculations. Methodology acceptance does not execute or qualify a financial
    risk report, confidence claim, control variate or Monte Carlo stopping rule.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Empty ordinary sample | Mean/population/sample estimates unavailable; empty ordered sum remains+0. |
| One finite observation | Mean/population variance available (variance0); sample variance/SD/SE/CI unavailable. |
| Singleton NaN/inf or unequal paired lengths | Input error before insufficient-history fallback. |
| True constant n>=2 | Sample variance/SD/SE0 and pointCI; Pearson/constant-control beta unavailable. |
| Xconstant,Yvarying | Sample covariance0, Pearson unavailable, control beta0. |
| Kahan/Neumaier on[2^54,1,-2^54] | Their declared folds may differ (0/1); no automatic switch or exact-sum claim. |
| Ordered sum[MAX,MAX,-MAX] | Checked intermediate failure permitted despite finite exact final sum. |
| mean[MAX,MAX] or merge of equal MAX constants | Finite meanMAX; do not require overflowing raw sum/weighted mean. |
| [1e16,1e16+2] | Shifted/scaled centered handling targets sample variance2; raw rounded-mean recurrence's4 is not the selected calculation. |
| [nextUp(1),1] | Genuine variation; do not classify rounded M2zero as a constant sample. |
| Four observations +/-2^511, twice each | Population variance2^1022 and sample variance(4/3)*2^1022 are representable even when rawM2is not. |
| Tiny varying observations with unrepresentable squared-unit variance | Requested variance errors; independently representable mean/SD/correlation need not require it. |
| [-MAX,+MAX] at n=2 | Mean0 and SE=MAX are representable although sample variance/SD are not; SE must not materialize either first. |
| X=Y=[-2^256,+2^256] or[-2^-270,+2^-270] | Correlation1 without overflow/underflow of the variance product. |
| Computed Pearson outside[-1,1] | Numerical error, not blanket clamp or bad-input classification. |
| Update/merge with invalid pair, count overflow/cap or required state failure | Error and unchanged original accumulator state. |
| R7[1,2,3,4],p=.25/.5 | 1.75/2.5; generic p0/1 return exact endpoints. |
| R7[-MAX,+MAX],p=.5 | Finite0, not infinity from endpoint subtraction. |
| Public sorted input[2,1] or containing a nonfinite unselected element | Input error; no trusted bypass. |
| Invalid p/alpha with empty input | Input error, not None. |
| finite_min/max/count on[NaN,+inf,3,-2] | Some(-2)/Some(3)/2; ordinary moments reject that sample. |
| Sample[1,3],alpha=.05 | SE1, df1 half-width about12.7062047362; bounds about[-10.7062047362,14.7062047362]. |
| Positive finite h collapses a CI endpoint or bounds overflow | Interval error; finite standalone h/SE may remain available. |
| Tiny alpha whose required tail cannot be resolved | Computation error; no p0/normal/NaN shortcut. |
| Repeatedly inspecting a fixed-count Student-t interval to stop simulation | No sequential-coverage guarantee from this core formula. |

## Observed choice and rationale

Kahan/Neumaier accumulation reduces rounding loss, and Welford recurrences avoid subtracting two large raw second moments. Parallel-merge formulas allow partial samples to be combined. R7 quantiles choose a particular finite-sample interpolation convention. These are observed choices; accepting their empty/constant/missing-data policies for financial reporting remains separate.

## Observed algorithm

1. `kahan_sum` subtracts accumulated rounding correction from each input before adding and updates that correction. Neumaier chooses the correction formula by comparing magnitudes of old sum and new input and returns sum plus correction; neither rejects nonfinite data.
2. Online statistics update `n←n+1`, `δ=x−mean`, `mean←mean+δ/n`, `M2←M2+δ(x−mean)`. Sample variance is `M2/(n−1)`; population variance is `M2/n`. Merging groups adds `δ² n_a n_b/(n_a+n_b)` to the two M2 values and combines weighted means.
3. Online covariance tracks two means/M2 values and `C←C+δx(y−new_mean_y)`; merge adds `δx δy n_a n_b/(n_a+n_b)`. Sample covariance divides by n−1. Correlation is covariance divided by `sqrt(var_x var_y)`, clamped to [-1,1], with exact zero denominator returning 0. Control-variate beta is covariance/var_y, or 0 for exact zero var_y.
4. Batch `mean` uses Kahan; `mean_var` uses online mean. These can differ in rounding. R7 uses `h=(n−1)p`, lower index floor(h), and linear interpolation to the next order statistic. The mutable helper partitions to select only needed order statistics; the sorted helper trusts sorting and finiteness.
5. Standard error is sample standard deviation/sqrt(n), NaN for n≤1. A two-sided `(1−alpha)` interval uses `−t_inv(alpha/2,n−1)` times the standard error; `ci_half_width` fixes alpha=0.05.

## Observed selection, controls and conventions

Callers select batch, online, population/sample, or sorted quantile APIs explicitly. Probabilities and alpha are decimal fractions; moments retain observation units. No finite observations are dropped by moment estimators. Only `finite_min_or_nan`, `finite_max_or_nan`, and `finite_count` filter nonfinite entries.

## Observed limitations, failure and fallback

Default mean/variance/covariance helpers return 0 for insufficient samples; `_or_nan` helpers deliberately differ. Unequal paired lengths return NaN. Invalid quantile p or nonfinite unsorted data returns NaN. The sorted helper validates only length and p. Online methods accept nonfinite data and arithmetic overflow can propagate. A Student-t interval is not a justified interval for correlated path draws merely because it is calculable; the calling estimator must establish independent replicates.

## Observed alternatives and tradeoffs

Sample versus population normalization and NaN versus zero insufficient-data behavior are implemented alternatives. Weighted/robust estimators, autocorrelation-adjusted standard errors and alternative quantile types are not provided by these functions. Compensated folds improve roundoff but do not guarantee identical answers across different merge orders.

## Evidence

- [Statistics implementation and inline tests](../../../../finstack-quant/core/src/math/stats.rs), [summation implementation](../../../../finstack-quant/core/src/math/summation.rs), [summation tests](../../../../finstack-quant/core/tests/math/summation.rs).
- `confidence_intervals_account_for_estimated_small_sample_variance` uses the df=1 Cauchy closed form (1e-10 absolute) and a tabulated df=15 value; table provenance is not documented there. `quantile_matches_r7_interpolation` is a hand-worked convention check. Merge/batch agreement checks consistency, not independent coverage of every scale.
- [Welford](../../../REFERENCES.md#welford-1962),
  [parallel centered moments](../../../REFERENCES.md#pebay-2008-moments),
  [ordered summation analysis](../../../REFERENCES.md#higham-1993-summation) and
  [Hyndman–Fan quantiles](../../../REFERENCES.md#hyndman-fan-1996-quantiles)
  now provide precise methodology/reference roles. Installed statrs 0.18
  quantile code uses type8, not the selected type7; its general statistical
  helpers also have different missing/range policies. Reusing that dependency
  alone would not qualify or preserve this package.
- Source expressions demonstrate variance4 rather than2 on [1e16,1e16+2],
  false zero spread for [nextUp(1),1], overflow from merging equal MAX constants,
  and wrong zero Pearson from extreme variance products. Required production
  qualification must cover the new private shifted/scaled implementation rather
  than treat Welford/Chan naming as an accuracy certificate.
- [Python statistics](../../../../finstack-quant-py/src/bindings/core/math/stats.rs),
  [Python summation](../../../../finstack-quant-py/src/bindings/core/math/summation.rs),
  [WASM math](../../../../finstack-quant-wasm/src/api/core/math.rs) and the
  [parity ledger](../../../../finstack-quant-py/parity_contract.toml)
  establish the actual scalar host surfaces and Rust-only online scope. Reconcile
  checked results, optional availability and removed aliases through those
  surfaces, retaining ordinary and explicitly filtering helper distinctions.
- [Monte Carlo estimates](../../../../finstack-quant/models/src/monte_carlo/estimate.rs)
  expose ci_95 and currently request alpha=.05 explicitly; those labels are
  separate from removing the core half-width default.
  [Simulation stopping](../../../../finstack-quant/models/src/monte_carlo/engine/pricing.rs)
  repeatedly checks core half-width and uses online/merge state. Its sequential
  coverage, antithetic/control/RQMC units and replicate weighting remain owner
  reconciliation/qualification, not automatic consequences of core formulas.

The [official R manual](https://www.stat.ethz.ch/R-manual/R-patched/library/stats/html/quantile.html)
identifies type7; its probability slack/missing/fuzz behavior is not adopted.
[Sandia's moment report](https://www.sandia.gov/research/publications/details/formulas-for-robust-one-pass-parallel-computation-of-covariances-and-arbitr-2008-09-01/)
supports incremental/parallel centered-moment algebra, not the proposed private
representation's full-range accuracy. [Higham's summation paper](https://nhigham.com/wp-content/uploads/2023/10/high93s.pdf)
supports conditional order/compensation claims.
[NIST mean intervals](https://www.itl.nist.gov/div898/handbook/eda/section3/eda352.htm)
provides the Student-t formula; [RQMC interval research](https://www.gerad.ca/fr/papers/G-2023-67.pdf)
distinguishes independent replicates from their dependent draws, and
[time-uniform confidence-sequence research](https://arxiv.org/abs/1810.08240)
establishes that sequential coverage is a separate problem. These primary sources
were consulted on 2026-11-08. Availability/error carriers, count limits, strict
correlation/CI reporting and API changes are scoped project choices.

Independent exact-rational/scalar-expression checks on 2026-11-08 confirmed the
offset/adjacent-value variances, overflowed raw-M2 versus finite restored variance
case, equal-MAX mean and direct extreme-SE identity. They confirmed the R7
near-integer rank h=3*binary64(1/3)=1-2^-54: on [-MAX,MAX,MAX,MAX], its value is
nextDown(MAX), not MAX from the rounded integer rank. The df1 Cauchy identity
gives SE1 and half-width12.706204736174703 for [1,3],alpha=.05; bounds are
[-10.706204736174703,14.706204736174703]. These examples validate mathematical
targets and known scalar-expression failures, not a new production kernel or
statistical coverage. Independent semantic reviews passed.

Observed current correlation can return NaN/zero from overflow/underflowed
variance products; no scale-normalized kernel is implemented yet. Production
Rust/host/model tests were located or inspected, not executed. Shifted/scaled
update/merge and query accuracy, atomic errors/counts, rank residual/support,
CI tail/range/coverage and actual consumer qualification remain pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
