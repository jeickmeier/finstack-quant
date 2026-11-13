# MODELS-AND-VALUATIONS-CREDIT-04: Structural default, asset calibration, survival export and simulation

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Current behavior observed in the source below. The accepted contract requires Rust and caller reconciliation.
- Validation status: Independent scalar formula and boundary examples evaluated. Production, host, market and empirical qualification were not run.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked structural default models and calibrated outputs

1. **Select an explicit law and probability basis.** Admit terminal GBM,
   continuous first-passage GBM with barrier `B(t)=B exp(g t)`, terminal
   compound-Poisson lognormal jumps on GBM, and the driftless CreditGrades
   survival approximation. Require `V,B,σ` finite and strictly positive,
   with asset value and barrier on the same monetary basis and annual decimal
   asset volatility. Specify finite annual continuous rates and growth
   where applicable. For GBM the log drift is `b−q−σ²/2`, with `b=r` under
   the risk-neutral pricing law and explicitly supplied physical total-return
   drift `b=μ` under the physical law. For a physical jump query supply the
   jump parameters for that measure. Keep discount `r` separate from physical
   drift. CreditGrades uses its native approximation with zero arithmetic
   asset drift and a flat barrier. `r` can discount a derived spread, while
   payout and physical drift are absent from that survival law. Remove
   implicit law and payout selection from construction and state restoration.

2. **Define time, touch and terminal distance precisely.** Evaluate PD and
   survival at finite nonnegative elapsed model years. At `T=0`, terminal GBM
   and terminal jumps give the indicator `V<=B`. For `T>0` they test only
   the endpoint, so starting below the barrier need not imply certain
   terminal default. First passage includes an initial touch or breach at
   every horizon, including zero. For `x=log(V/B)>0`, `ν=b−q−σ²/2−g` and
   `s=σ sqrt(T)`, first-passage PD for positive `T` is
   `Φ(−(x+νT)/s) + exp(−2νx/σ²) Φ(−(x−νT)/s)`.
   In terminal GBM use `PD=Φ(−d2)`, `d2=(x+(b−q−σ²/2)T)/s`. Expose
   `d2` as distance to default only for positive-horizon terminal GBM on
   the selected measure. For CreditGrades use its own zero-time rule below.
   Batch PD preserves request order and fails as a unit on invalid input or
   unresolved evaluation. Physical model PD and this standardized log
   distance carry no empirical EDF mapping. [Merton 1974](https://onlinelibrary.wiley.com/doi/full/10.1111/j.1540-6261.1974.tb03058.x),
   [Black and Cox 1976](https://onlinelibrary.wiley.com/doi/full/10.1111/j.1540-6261.1976.tb01891.x).

3. **Check probability arithmetic and retain meaningful tails.** Compute log
   moneyness without an overflowing ratio, using a near-equality `log1p`
   form. Scale `σ sqrt(T)` and the standardized arguments before squaring
   or dividing. Evaluate normal tails and complements directly or in log
   space. For reflection, let `a=(x+νT)/s`, `c=(x−νT)/s`. When `c>=0`,
   combine the quadratic terms as `exp(−a²/2) [exp(c²/2) Φ(−c)]` via a
   scaled normal-tail evaluation. Keep stable positive sums and differences
   for PD and survival. Remove failure-to-zero and blanket clipping. A
   valid rounded endpoint is allowed in a scalar probability. Operations
   that need its logarithm use the resolved model log-tail and fail if the
   required result is unresolved. [Core special functions](../core-numerics/distribution-functions.md).

4. **Bound the terminal jump mixture by omitted mass.** Require finite
   intensity `λ>=0`, mean log jump `mJ` and log-jump deviation `sJ>=0`.
   Use `κ=expm1(mJ+sJ²/2)`, `θ=λT` and integrated compensation `−θκ`.
   Given count `n`, terminal log moneyness is normal with mean
   `x+(b−q−σ²/2)T−θκ+n mJ` and variance `(σ sqrt(T))²+n sJ²`.
   Exactly zero intensity or identically zero log marks reduces to GBM.
   For an active jump mixture require an explicit finite omitted-mass budget
   in `(0,0.5)` and an explicit maximum term count in `1..=1,000,000`.
   Select the smallest `K` within that count budget with checked numerical
   `log Pr(N>K)<=log(budget)`, using the shared bounded Poisson tail
   evaluator. Sum weights on `0..=K` by a checked mode-centered recurrence
   anchored at `min(floor(θ),K)`, sweeping down and then up with stable
   accumulation. Retain the smaller of the two accumulated complementary
   tails and restore the other stably from it. Report the number of terms
   and omitted mass or its log. There is no rescaling of captured weights.
   Failure to meet the work or tail gate fails the operation. For a bounded
   unit payoff, exact omitted mass bounds absolute truncation error. The
   computed mass gate does not certify floating-point accuracy, relative
   tail accuracy or a derived spread tolerance.
   [Merton 1976](https://pages.stern.nyu.edu/~dbackus/Disasters/Merton%20jumps%20JFE%2076.pdf),
   [Core Poisson tail contract](../core-numerics/pseudorandom-and-bridge.md).

5. **Retain the CreditGrades origin mass and separate recoveries.** For
   finite log-barrier uncertainty `ℓ>0` set `xC=log(V/B)+ℓ²`, require
   `xC>=0`, and set `A=hypot(σ sqrt(T),ℓ)`. Use
   `S(T)=Φ(−A/2+xC/A)−exp(xC)Φ(−A/2−xC/A)` with a checked log difference
   for survival and the complementary positive sum for PD. At `xC=0`
   survival is zero. Evaluate the formula at `T=0` and retain its origin
   default mass, which belongs to the approximation. For `ℓ=0` use the
   flat first-passage law under zero arithmetic asset drift, including
   immediate default at `V<=B`. Retain the simplified spot-equity helper
   `B=D Lbar`, `V=E+B`, `σ=σE(E/V)` for positive same-basis inputs and
   barrier mean recovery `Lbar` in `(0,1]`. This fixes reference equity
   to current spot equity. Direct construction takes the barrier itself.
   Obligation recovery `R` in `[0,1]` enters the loss or spread calculation
   independently of `Lbar`. Remove the forced recovery equality.
   [CreditGrades technical document, Chapter 2 and Appendix A](https://www.msci.com/downloads/web/msci-com/research-and-insights/blog-post/credit-in-the-covid-crisis-contagion-valuation-default/CGtechdoc.pdf).

6. **Use one terminal GBM equity reprice and a bounded equity fit.** For
   positive maturity use `E=V exp(−qT) Φ(d1)−B exp(−rT) Φ(d2)` and
   `σE E=σ V exp(−qT) Φ(d1)`, where `d1=d2+σ sqrt(T)`.
   Reuse checked Black–Scholes call and delta arithmetic. Implied equity
   value and volatility are jointly available only for terminal GBM. For
   fitting require positive observed `E,σE,B,T`, an explicit positive
   iteration budget and finite nonnegative relative residual budgets for
   equity value and equity volatility. Start from `V=E+B` and
   `σ=σE(E/V)`. At each fixed-point step use the prior state's `d1,d2`
   to set `V'=(E+B exp(−rT) Φ(d2))/(exp(−qT) Φ(d1))` and
   `σ'=σE E/(V' exp(−qT) Φ(d1))`. Reprice both from the actual
   candidate and return only when both absolute errors divided by the
   respective observed values meet the inclusive budgets. Report the attempt
   count and actual residuals. Remove equity-fraction and CDF floors and
   fail checked range, ill-conditioning or exhaustion as computation
   errors. Select the liability barrier explicitly. Retain the separately
   requested KMV-style `short debt+0.5 long debt` proxy for finite
   nonnegative same-basis debt values and a finite positive result.
   [Checked Black–Scholes contract](../models-analytic/spot-vanilla-and-greeks.md),
   [Crosbie and Bohn](../../../REFERENCES.md#crosbie-bohn-2003-kmv).

7. **Fit a target PD on its explicitly chosen measure.** With terminal GBM,
   positive `T` and `0<p<1`, compute
   `log B=log V+(b−q−σ²/2)T+σ sqrt(T) Φ⁻¹(p)`. Return only a finite
   positive represented barrier whose same-measure reprice passes a caller
   supplied finite nonnegative absolute log-tail residual budget. Compare
   `log PD` for `p<=0.5` and `log S` otherwise. Report the selected measure
   and residual. A physical fit uses `μ` in its equation and preserves `r`
   for the risk-neutral model.

8. **Export checked survival on a named clock and origin basis.** Require
   already ordered distinct positive tenors, caller-supplied base date,
   complete source clock/context and explicit obligation recovery. An
   unconditional continuous export requires unit origin survival. An
   explicitly selected alive-conditional CreditGrades export uses
   `log S(T)−log S(0)` with resolved positive `S(0)` and discloses the
   raw origin mass. Recognize positive CreditGrades uncertainty as an atom
   even if rounded PD at zero is zero. Reject an unconditional atom-bearing
   export and any export with zero survival at the origin. Never assign an
   origin atom to the first positive interval. For terminal laws require
   explicit selection of a proxy fitted to the sampled terminal marginals.
   Require nonincreasing finite log survival at the requested knots.
   Compute ending-interval intensity from adjacent log-survival difference
   over actual positive width and use the canonical intensity builder. An
   equal pair gives zero intensity. Reject increases and unresolved rates.
   Keep the source's constant final intensity beyond the last knot. A
   terminal proxy establishes sampled-knot consistency only.
   [Canonical hazard source](../core-market/hazard-survival.md).

9. **Keep two zero-coupon spread definitions.** The exogenous maturity
   recovery spread is `−log(1−PD(T)(1−R))/T`, using the native model law and
   the stable `log1p` or complementary `R+(1−R)S(T)` form. The endogenous
   terminal debt spread is `−log(E_Q[min(V_T/B,1)])/T` for terminal GBM or
   terminal jump diffusion. Given normal log moneyness with mean `m` and
   deviation `s`, its bounded normalized factor is
   `Φ(m/s)+exp(m+s²/2)Φ(−(m+s²)/s)`. Compute the factor directly in log/scale
   and use its complementary normalized put loss near par. For jumps apply
   the same bounded mixture rule and report its truncation diagnostic. Avoid
   discounted assets minus equity subtraction. Require positive maturity,
   valid recovery when used, positive price factor and a finite annual
   decimal spread. Zero loss produces zero spread when established. Both
   definitions use a terminal payoff convention for the spread.
   [Merton's terminal debt payoff](https://onlinelibrary.wiley.com/doi/full/10.1111/j.1540-6261.1974.tb03058.x).

10. **Make the CDS spread and its calibration explicitly approximate.**
    Rename the time-only operation `cds_proxy_spread`. Require positive
    maturity, an explicit positive interval count on the checked uniform
    grid, explicit recovery and the same survival admission and origin
    selection as the hazard export. Interpret the grid as elapsed ACT/365F
    years with premium accrual `α_i=Δt_i*365/360`. Discount continuously at
    flat `r`. For interval default mass `d_i=S_(i−1)−S_i`, use
    protection `(1−R) Σ DF(mid_i)d_i` and risky annuity
    `Σ α_i[DF(t_i)S_i+0.5 DF(mid_i)d_i]`. Compute differences and sums
    stably and require a finite positive annuity. Remove the synthetic
    anchor date, default-mass clipping and inferred quarterly count.
    `from_cds_proxy_spread` fits terminal GBM only, with explicit positive
    quote in basis points, recovery below one and a caller-selected finite
    positive volatility bracket. Use the identical proxy and bounded
    fallible Brent with explicit controls. Admit one exact endpoint or
    opposite endpoint signs. Reject a bracket with two exact roots at its
    endpoints. Propagate invalid trials. Require the actual returned
    volatility to reprice within the explicit absolute quote tolerance in
    basis points and report the residual. Remove the fixed scan and early
    grid-root selection. A passing bracket fit carries no global uniqueness
    claim. Contractual CDS pricing/calibration belongs to its instrument
    owner. [Core Brent contract](../core-numerics/brent-root.md),
    [CDS legs](12-cds-legs.md).

11. **Simulate checked asset knots with explicit replay.** Retain
    risk-neutral GBM asset paths for either barrier type and terminal
    jump-diffusion asset paths.
    Asset knots carry no barrier monitoring, default classification or
    recovery settlement. Reject CreditGrades path requests. Require
    positive physical path and step counts, positive horizon, explicit
    `u64` PCG seed and stream, and explicit antithetic choice. Pairing
    requires an even path count and adjacent complete pairs. Use actual
    adjacent widths on the canonical uniform grid and checked scaled
    lognormal restoration. For active jumps on each interval draw
    `N~Poisson(λΔt)` with the shared checked count inverse, and use the exact
    conditional aggregate `N mJ+sqrt(N) sJ W` for independent standard
    normal `W`. Draw the diffusion normal, then count uniform, then one
    mark normal only if needed by at least one member. For a pair negate
    both normals and use reflected uniform `(1−2^−53)−U` on the same
    PCG grid for the partner count. Use that partner's own count in its
    mark aggregate. Process groups in path order and intervals in time
    order. Zero-effect jumps consume no jump draws. Check the count, step
    and allocation products, cap stored path values at 64,000,000 and
    reject incomplete batches. Report the actual knots and run controls.
    The grid and paired draw law are part of replay. Finite RNG resolution,
    target math and changed pair covariance limit replay and variance claims.
    [Core PCG and Poisson](../core-numerics/pseudorandom-and-bridge.md),
    [core time grids](../core-numerics/time-grids.md),
    [checked lognormal update](../models-monte-carlo/04-lognormal-diffusions.md).

12. **Keep reporting, restoration and product responsibilities explicit.**
    A report uses a validated supplied model or an explicit equity fit
    request, including the selected barrier. Require a nonempty list of
    positive finite report horizons in caller order and valid shared
    maturity, recovery and applicable numerical controls up front.
    Required PD or source-fit failure rejects the report. Unavailable
    measures carry a typed reason, including the affected horizon when an
    applicable measure fails. A failed series has no partial array.
    Report model law, probability basis, fit and applicable numerical
    diagnostics. Remove automatic default-point and grade selection.
    Empirical mapping and compatible scale selection belong to CREDIT-02.
    Restore only fully validated explicit model state, with unknown fields
    and unsupported combinations rejected. Validate canonical path grid,
    flat shape, common initial asset and positive finite values on restore.
    Rust owns the calculations and errors. Reconcile Python, WASM, schemas,
    generated declarations, parity, the UI report and its fixtures. Preserve
    exact seed/stream in both hosts. Propagate checked PD to the bond PIK
    trigger. Bond cash flows, default monitoring and CDS quote valuation
    retain their own owners. [PD and scale decisions](02-pd-scales-and-rating-data.md).

## Observed behavior

- Nonpositive horizons yield zero PD or infinite distance. First-passage
  arithmetic can turn NaN into zero PD. Terminal jumps sum through
  `max(20,ceil(θ+10 sqrt(θ)))` without a validated omitted-mass or work gate.
  CreditGrades discards the origin mass at zero time and uses `ℓ²/T` in
  an intermediate expression. Distance-to-default outputs can describe
  a diffusion statistic for a different default law.
- Equity fitting uses 100 fixed-point iterations and relative parameter
  changes of `1e−8`. Implied equity admits nonterminal diffusion and
  uses fixed small-value floors. Target-PD documentation can conflate
  physical drift with discount rate. CDS proxy fitting scans 129 points
  over `[0.01,2]`, can skip unusable trials and can return an exact grid
  root before assessing other brackets.
- Hazard export sorts times and derives survival from `1−PD`. It ties
  CreditGrades barrier recovery to loss recovery. The first interval
  is based at unit survival even when CreditGrades has an origin atom.
  The CDS spread uses an approximately quarterly grid and a synthetic
  anchor. Terminal debt uses discounted asset value minus equity.
- Asset simulation uses seed-only PCG, nominal steps and individual jump
  marks. It can consume work for an unused antithetic leg and uses GBM
  drift for CreditGrades without its barrier. The report supplies a
  default KMV-style barrier and a default master-scale grade. The UI
  displays these grades. The bond engine also consumes structural PD
  for its PIK trigger. [UI worker](../../../../finstack-quant-ui/registry/shared/file/services/models.ts),
  [bond PIK trigger](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/engine.rs).

## Evidence

Owners: [Merton module implementations](../../../../finstack-quant/models/src/credit/merton). [default_probability.rs](../../../../finstack-quant/models/src/credit/merton/default_probability.rs) includes first_passage_growing_barrier_matches_black_cox_1976 formula comparison and jump_diffusion_pd_matches_monte_carlo; [calibration.rs](../../../../finstack-quant/models/src/credit/merton/calibration.rs) includes equity/CDS roundtrips; [simulation.rs](../../../../finstack-quant/models/src/credit/merton/simulation.rs) includes GBM/jump mean and seed reproducibility cases; roundtrips and simulation comparisons establish internal consistency only. References: [Merton 1974](../../../REFERENCES.md#merton-1974), [CreditGrades](../../../REFERENCES.md#finger-2002-creditgrades), [KMV](../../../REFERENCES.md#crosbie-bohn-2003-kmv).

### Independent review evidence

- At `V=100,B=80,σ=.2,r=.05,q=.01,T=2`, continuous first-passage PD
  is about `0.3830148949` for a flat barrier and `0.5034054032` for
  `g=.05`. At volatility `1e−200` the squared volatility underflows in
  binary64. The current reflection expression can become `∞−∞` and
  turn a near-certain crossing into zero. At Poisson mean `3`, the
  current cutoff `K=21` omits about `1.59705e−12` probability mass.
- CreditGrades with `E=25,D=80,Lbar=.4,σE=.5,ℓ=.3` has PD about
  `0.0361517749` at origin and `0.1000206695` at one year. The one-year
  PD conditional on survival at origin is `0.0662644728`. A separate
  terminal GBM example with `V=100,B=80,σ=.2,r=.1,q=0` gives PD
  `0.0906145386` at year three and `0.0207581482` at year twenty.
  That sampled terminal curve fails survival admission.
- Terminal equity with `V=100,B=80,σ=.25,r=.05,q=.02,T=1.5` gives
  `E=25.5334938064` and `σE=.8059563251`. At `V=100,σ=.25,r=.04,μ=.08,
  q=.01,T=1,p=.05` the physical fit yields `B≈68.9034865130` and retains
  risk-neutral PD `0.0687913013` when using the original `r`. The
  risk-neutral target would give `B≈66.2017421625`. In a separate
  scalar reproduction at `V=100,B=101,σ=.15,r=.10,q=.08,T=.33`, the
  existing `1e−8` step gate passes with an equity-volatility relative
  repricing error of approximately `1.58548e−8`.
- For the CDS proxy with `V=110,B=100,r=.03,q=.08,T=5,R=.4` and 20
  intervals, a `2,000 bp` quote has roots near `σ=.1103819574` and
  `σ=.2501900528` in separate brackets. For terminal GBM with
  `V/B=exp(2.02),σ=.2,r=q=0,T=1` the normalized put loss is about
  `1.4664e−25`, while subtraction of asset and equity values erases the
  annual spread in binary64.
- A `T=.9` three-step grid shows why storing the exact endpoint and
  actual widths matters. The reflected PCG endpoint remains in `[0,1)`.
  The compensated compound-Poisson asset law at `V=100,r−q=.03,T=2`
  has mean `106.1836546545`. A finite scaled asset can survive an
  intermediate exponent overflow, for example `1e−300 exp(800)`.
- The independent examples and inspected tests establish specific
  formulas and counterexamples. Qualification still requires tail and
  solver stress cases, contractual quote comparisons, fixed-stream and
  statistical path evidence, host/state tests and empirical suitability.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
