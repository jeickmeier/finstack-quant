# MODELS-AND-VALUATIONS-CREDIT-07: Monthly pool defaults and per-name copula sampling

- Decision status: **Accepted — full package on 2026-11-12**.
- Implementation status: Current behavior observed in the source below. The accepted contract requires Rust and product caller reconciliation.
- Validation status: Independent scalar, quadrature and draw-assignment examples evaluated. Production, host, market and product qualification were not run.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked pool default rates and period copula resolution

1. **Select one validated model and admit the proper monthly context.**
   Keep explicit deterministic, single-systematic copula, static lognormal
   intensity and capped factor-rate choices. Remove the implicit `2%` CDR
   and `None` as the representation of a selected deterministic model.
   Use checked model admission, strict restoration and fallible operations.
   A monthly rate applies to the eligible surviving balance over one model
   month. An applied month index, where required, is positive and checked.
   Delegate a deterministic cashflow assumption to the accepted cashflow
   owner. Conditional curves require no available-fraction input, and
   original-balance targets require the actual available fraction. A
   stochastic multiplier requires a conditional base rate, and cannot
   reinterpret an original-balance target as a probability. Constant
   copula and intensity laws have no intrinsic aging state. Convert an
   annual effective CDR by the accepted twelve-equal-month survival
   conversion `−expm1(log1p(−CDR)/12)` with exact endpoints. An annual
   continuous intensity uses one-twelfth of a year as its modeled interval.
   Calendar, seasoning and legal-period mapping remain product-owned.
   [Annual rate conversion](../cashflows/10-prepayment-and-mortality.md),
   [conditional rates and original-reference targets](../cashflows/11-default-and-recovery-assumptions.md).

2. **Use one checked single-systematic copula law for a period.** For
   one eligible name, require an explicitly prepared marginal `p` in
   `[0,1]` on the supplied period and probability basis. A homogeneous
   monthly request can use the selected base CDR's converted marginal.
   Validate the chosen family, finite common normal factor `Z`, and its
   typed mixing input. Gaussian needs only `Z`. Student-t needs `Z` and
   positive finite shared `W=χ²_ν/ν`, with `2<ν<=1e8` in this pool profile
   to share the checked mixing-inverse domain. Random factor loading
   (RFL) needs `Z` and its shared normal loading shock. The selected RFL
   kernel derives the common loading. Require finite correlation in
   `[0,1]` with exact admitted endpoints. Correlation here refers to the
   selected copula parameter. Reject `MultiFactor` on this single-systematic
   route. CREDIT-09 owns the checked threshold, mixing transform and RFL
   calibration. A different factor shape or unsupported family fails.
   [Copula kernels](09-copula-kernels.md), [Li](https://www.ressources-actuarielles.net/EXT/ISFA/1226.nsf/0/34e84cb615c8b4eac12575fe006a9759/%24FILE/li.defaultcorrelation.pdf),
   [Demarta and McNeil](https://onlinelibrary.wiley.com/doi/10.1111/j.1751-5823.2005.tb00254.x).

3. **Preserve literal probability endpoints and common conditioning.**
   After validating the request, `p=0` gives no default and `p=1` gives
   default without threshold inversion. For interior `p`, use the chosen
   marginal inverse with no probability floors. Gaussian realization
   compares `sqrt(ρ)Z+sqrt(1−ρ)ε` with the normal threshold `c`. Student-t
   compares the same numerator with `c sqrt(W)` using the t threshold.
   RFL uses the common derived loading `B` in `BZ+sqrt(1−B²)ε` and the
   normal threshold. The comparison is inclusive. Conditional probabilities
   integrate out only `ε` under the same `Z` and mixing state. If the
   residual scale is zero, use the literal deterministic comparison.
   Gaussian `ρ=0` recovers the marginal exactly in conditional mode.
   Student-t `ρ=0` still conditions on shared `W`. Check range and arithmetic
   before producing results. The one-period expectation under the reference
   factor law is the supplied marginal, with the stated kernel assumptions.

4. **Select finite names or conditional fractions explicitly.** Require
   explicit copula granularity. `PerName` realizes one indicator per
   eligible name. Rename `LargeHomogeneous` to `ConditionalFraction`,
   which applies each name's own conditional probability and uses no
   idiosyncratic draw. It can use a common marginal for a homogeneous
   pool. For normalized weights `w_i` and conditional probabilities `q_i`,
   it yields conditional mean `Σw_i q_i`. For independent conditional
   idiosyncratic shocks the lost variance is `Σw_i² q_i(1−q_i)`.
   Granularity is an explicit scenario choice, with exposure concentration
   and nonlinear tranche or waterfall effects qualified by the product.
   Remove the implicit `PerName` default. The other model variants in
   this record return rates. A request for copula name events requires
   a selected copula. [Gordy's granularity analysis](https://www.federalreserve.gov/pubs/feds/2002/200255/200255pap.pdf).

5. **Keep random draws aligned by period, name and role.** Use the
   checked Philox profile. Use no mixing draw for Gaussian. Use one open
   midpoint uniform for a Student-t scale or one normal for the RFL
   loading shock per period when that shared input is needed. Both modes
   consume the same prepared period realization when comparing their
   outputs. Per-name idiosyncratic normals are independent under the
   modeled RNG law. Assign them by stable original name and period,
   independent of surviving roster, order changes or deterministic cases.
   Separate systematic, shared mixing, idiosyncratic default and recovery
   roles. For a paired evaluation reuse the common mixing realization and
   negate corresponding `Z` and same-name `ε`. Survivor compaction cannot
   shift other draws. Validate all supplied inputs before evaluation and
   publish results in request order as one complete batch. An empty name
   list has an empty result and consumes no draws. Paired reuse supplies
   no guarantee of variance reduction. [Philox and owner draw addressing](../models-monte-carlo/02-random-streams.md),
   [checked chi-squared mixing](../core-numerics/chi-squared-inversion.md).

6. **Name the stateless lognormal intensity transform by its actual law.**
   Use finite annual base intensity `λ0>=0`, one finite signed log shock
   loading `b`, and explicit finite standardized `Z`. Use
   `ℓ=log(λ0)−log(12)−b*(Z+b/2)` and
   `q(Z)=−expm1(−exp(ℓ))`. Evaluate the complete scaled expression and
   preserve meaningful tails. Exact zero intensity gives zero, and zero
   loading gives the ordinary monthly constant-intensity probability.
   Admit a rounded probability endpoint only when established by the
   checked calculation. Positive `b` increases stress at lower `Z` and
   negative `b` reverses the direction. Under a standard-normal reference
   factor, the compensator gives mean intensity `λ0`. Retire separate
   loading and volatility fields whose only role is their product, and
   remove the synthetic asset correlation. Move mean reversion to explicit
   product factor dynamics, preserving the live caller's intended choice
   of horizon-persistent or evolving stationary factors. The transform
   has no temporal state or inferred seasoning effect.
   [Lando on intensity-based survival](https://mx.nthu.edu.tw/~jtyang/Teaching/Risk_management/Papers/Models/On%20Cox%20Processes%20and%20Credit%20Risky%20Securities.pdf).

7. **Integrate the conditional intensity probability for expected MDR.**
   Evaluate `E_Z[q(Z)]` for a one-month standard-normal reference factor.
   Use the exact zero-intensity and zero-loading cases above. For the
   nontrivial integral require explicit finite symmetric cutoff `A>0`,
   omitted-mass budget in `(0,1)`, and checked adaptive Gauss–Legendre
   order, absolute estimator tolerance, maximum depth and total
   evaluation budget. Require `2Φ(−A)` within the omitted-mass budget.
   Integrate `q(z) φ(z)` on `[-A,A]`, with distinct interior splits at
   zero and at `ℓ=0` when present. Use checked split geometry and bound
   the total work and interior estimator across panels. Report the
   integrated estimate, omitted normal mass, interior estimator and
   work. Omit the tails without rescaling. Normal mass bounds the omitted
   expectation because `0<=q<=1`. The interior estimator carries its own
   accuracy limits. Remove the transform of mean intensity as expected
   MDR. A one-month expectation does not establish multimonth incidence
   under survivor selection or persistent factors.
   [Checked quadrature](../core-numerics/quadrature.md),
   [lognormal Laplace transform](https://data.math.au.dk/publications/thiele/2013/math-thiele-2013-06.pdf).

8. **Make the factor-rate cap and expected rate explicit.** Require a
   checked conditional base MDR `d` at the supplied age, finite signed
   log shock loading `b`, and an explicit monthly cap `c` in `[0,1]`.
   Evaluate `min(c,d exp(−bZ))` in scale. Exact `d=0` or `c=0` gives
   zero. With `b=0` return `min(c,d)`. In the remaining case let
   `τ=abs(b)` and `k=log(c/d)/τ`. The standard-normal expectation is
   `d exp(τ²/2) Φ(k−τ)+c Φ(−k)`. For `a=τ−k>=0`, evaluate the first
   term with the scaled normal tail as `c exp(−k²/2) [exp(a²/2) Φ(−a)]`.
   Check range, restore the capped value where binding, and propagate
   invalid base assumptions. Remove the implicit `0.50` cap, epsilon
   zero shortcut, missing-factor-as-zero and synthetic correlation
   accessor. The capped shock can change the unconditional monthly rate
   relative to `d`. It owns neither asset correlation nor a temporal law.

9. **Reconcile product clocks, scope and existing host state.** The
   structured-credit owner selects factor dynamics, eligible survivors,
   chronological aggregation, draw addresses and exposure/recovery
   treatment. Keep one authoritative period copula plan where selected.
   Remove the redundant scalar monthly copula evaluation with incompatible
   shared factors and the caller-wide `.50` default cap. Reconcile
   absolute survival division and date clipping in instrument hazard
   marginals with the accepted checked local hazard operation. An explicit
   instrument hazard marginal remains authoritative where the product
   selects it. Pay attention to distinct one-period normalized-factor and
   month-by-month conditional survival laws. Any scalar correlation
   precedence or reduction of an authored structure belongs to CREDIT-08
   and the product. Keep kernels Rust-owned. Reconcile strict model and
   granularity wires, structured-credit Python and WASM payloads and
   overrides, schemas, declarations, fixtures and parity. Reject invalid
   or unsupported inputs before delivering product results.
   [Stochastic structured-credit waterfall](../valuations-structured-credit/15-stochastic-waterfall.md),
   [instrument collateral](../valuations-structured-credit/08-instrument-collateral.md),
   [local hazard](../core-market/hazard-survival.md), [pool correlation](08-prepayment-and-pool-correlation.md).

## Observed behavior

- Spec constructors clamp CDR, loadings, volatility and correlation, and
  deterministic `build` returns `None`. The copula paths clip threshold
  probabilities and exclude exact unit correlation in finite-name mode.
  Monthly and period copula routes can both run, with different factor
  representations. The product also applies a general `.50` MDR cap.
- The monthly intensity transform is stateless and uses the mean-intensity
  transform for expected MDR. Its `mean_reversion` is discarded by the
  monthly kernel constructor. The live product reads that field and uses
  positive values for its stationary AR(1) factor, with transition
  coefficient `exp(−κ/12)`. Zero retains a horizon-persistent factor.
  The factor-rate path maps invalid deterministic MDR to zero and has its
  own `.50` cap. Its spec and model return different correlation proxies.
- Per-name and conditional-fraction product paths compact the surviving
  roster. Sequential default, mixing and recovery draws can lose alignment
  when antithetic partners have different survivors. Instrument hazard
  marginals can derive probability from absolute survival division, with
  a fallback to certainty when the start value rounds to zero.

## Evidence

Owners: [Pool default implementations](../../../../finstack-quant/models/src/credit/pool/default), [product factor selection](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pricing_methods.rs), [period and monthly planning](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/stochastic/pricer/engine.rs), [pool flow paths](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flow_source.rs), and [instrument hazard flows](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_flows.rs). Inline tests cover sampled conditions, expectation checks and the live mean-reversion route. References: [Gaussian copula](../../../REFERENCES.md#li-2000-gaussian-copula), [Intensity models](../../../REFERENCES.md#lando-1998).

### Independent review evidence

- A CDR of `1e−12` converts to monthly probability about
  `8.333333333337e−14`. At zero Gaussian correlation the present threshold
  floor gives about `1e−10`, a factor of about 1,200. For a Student-t
  threshold of `−1` and zero Gaussian correlation, conditional PD is
  `0.3085375387` at `W=.25` and `0.02275013195` at `W=4`. Shared scale
  matters even when Gaussian correlation is zero.
- With annual intensity `.9` and log loading `2`, independent Simpson
  integration gives expected MDR `0.04934877723`. The mean-intensity
  transform gives `0.07225651367`. Two months with the same factor give
  expected incidence `0.08285948461`. Compounding the monthly mean gives
  `0.09626225265`. A separate closed-form persistent Gaussian example
  at `p=ρ=.5` has `q(Z)=Φ(−Z)`, uniform on `[0,1]`. Its two-month
  incidence is `2/3`, while compounding the monthly marginal gives `3/4`.
- With base MDR `.04`, cap `.5` and log loading `1`, the analytic
  capped-rate expectation is `0.06464501025`, independently reproduced
  by finite normal integration. At `b=1e200,Z=−5e199` the compensated
  intensity log shock is zero, while separating the two quadratic terms
  can produce `∞−∞`.
- For two equal independent names with `q=.5`, the expected loss of a
  unit-notional portfolio's first `.25` loss slice is `.1875`. Applying
  the tranche payoff to conditional mean portfolio loss gives `.25`.
  One name with weight `.5` and `q=.1` contributes `.0225` conditional
  variance regardless of how many small names are added. If the first
  name is absent in one leg, compacted survivor indexing assigns the
  next name a different slot. Stable name and role addresses preserve
  the paired assignment. After prior integrated hazard `800`, another
  interval with integrated hazard `.02` has conditional default
  probability `0.01980132669` even though both absolute survivals
  round to zero.
- Qualification still requires numerical tail and depth cases, exact
  endpoint and seeded pair cases, multimonth and finite-pool product
  evidence, existing host and wire checks, and credit/recovery ownership
  reconciliation. The examples are independent scalar oracles.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
