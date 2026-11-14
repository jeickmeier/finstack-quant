# VALUATIONS-OTHER-FIXED-INCOME-11: Revolver three-factor paths, survival and default integration

- **Decision status:** Accepted 2026-11-14 — full checked revolver simulation and default valuation package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below. Reconciliation to the accepted contract is pending.
- **Validation status:** Live source and primary references inspected. Independent process, mean fit, credit, and estimator arithmetic checked. Contract, market, model, runtime and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked revolver simulation and default valuation

1. **Select the actual valuation route and claims.** Use the authorized
   valuation and information cut, lender share, currency and prepared
   [record 10 cash and exposure states](10-revolver-cashflows.md#accepted-decision-for-checked-revolver-contractual-cashflows).
   The registered Discounting route requires deterministic funding
   and a static discount source, with explicit No Default or Frozen
   Hazard credit. MonteCarloThreeFactor requires the authored modeled
   funding and joint-model configuration. An omitted model can resolve
   once from the authored funding variant to its actual registry key.
   A named model must match the request. Remove automatic Monte Carlo
   execution under Discounting and implicit market-driven selection of
   rate, credit or utilization inputs. Require the pricing or scenario
   measure and provenance. A physical stress scenario carries its
   scenario label and needs separate authority to support fair value.

   Set the simulation anchor `A` to the later of valuation date
   `V` and the authorized opening date. A future opening can use
   an explicitly bound deterministic preactivation discount and, when
   needed, borrower survival bridge from `V` to `A`. Apply those factors
   to the compatible continuation and issue claims. Require a qualified
   dated treatment for each actual earlier cash claim. The ordinary
   future-opening profile has no funded or contingent exposure before
   its authorized activation. Reject an unresolved prior binding right
   or a stochastic preactivation dependence that the bridge cannot
   represent. Match actual opening balances without issuing synthetic
   historical cash. A proven closed position returns zero only after
   complete lifecycle admission. An origin-only claim uses entitled
   pending cash with no dummy step or random draw.
   [Shared registry and lifecycle](../valuations-framework/pricing-lifecycle.md),
   [shared model resolution](../valuations-framework/model-resolution.md).

2. **Use the accepted joint utilization law and phased funding.**
   Reuse the checked dedicated transition in
   [MODELS-MONTE-CARLO-11](../models-monte-carlo/11-credit-line.md).
   From opening spread `s_a` and a frozen positive initial reference
   `s_ref`, an active `β` sets the interval target to
   `clip(θ + β(s_a−s_ref)/s_ref, 0, 1)`. A zero link uses `θ`
   directly. Use the accepted clamped Gaussian endpoint with actual
   step width and frozen target. Exact zero utilization volatility
   preserves mean reversion and the spread response. A fixed drawn
   path follows its expressly selected contractual or deterministic
   funding rule. The clamped transition defines discrete boundary
   mass and grid-dependent behavior.

   Maintain latent utilization separately from held funded dollars
   between eligible funding nodes. Positive commitment initializes the
   latent state from actual drawn over commitment. At a qualified
   funding node, map the receiving candidate to the incoming limit
   and cap it by exact available funded capacity `C−L`. Restore the
   selected funded balance once, keeping the exact Money cap when it
   binds. Post the exact signed increment under incoming terms, then
   apply the contractual phases and use resulting `D/C` for the next
   interval. At zero commitment require zero drawn and LC use, with
   no division or discretionary funding. If positive commitment can
   resume, require an authorized restart from actual funded state.
   The limit-change leg preserves funded dollars where feasible.
   A step-down or LC increase needs the authorized repayment or
   scenario treatment in record 10 when existing par no longer fits.
   Suppress a discretionary update at terminal closure. Integration
   knots alone never generate funding. Empirical drawdown evidence
   can motivate a link. Its magnitude, sign, regime and pricing
   measure require supplied and qualified inputs.
   [Empirical corporate credit lines](https://www.bde.es/informes/be/docs/dt0821e.pdf).

3. **Bind exact grids, clocks and correlation.** Use ACT/365F from
   `A` for model dates and the separate native clocks for their
   sources. Prepare one stored model/integration grid with exact
   funding, fixing, coefficient, exposure and active cash events
   and explicitly selected refinement. Use authored positive
   refinement limits, node and work budgets. Keep all integration
   endpoint states or their checked increments during each path.
   Remove the hidden weekly refinement followed by coarser
   reconstruction and the synthetic terminal tail. Cash and exposure
   use their exact dated phases. A reporting-only cut partitions
   existing results without altering the pricing/funding grid or
   random draws. A new simulated state needs an expressly changed
   economic observation grid.

   Elect identity or a strict `3×3` latent correlation in
   `[utilization, rate, credit_spread]` order. Use the accepted
   checked PSD factor at zero pivot tolerance, applied once. Remove
   inferred correlations, repair and the shorthand override.
   Inactive innovation coordinates have zero off-diagonal links.
   No Default and Frozen Hazard have no spread-to-utilization link
   in this profile. A deterministic active spread can still affect
   utilization through the explicit level link. Retain the shared
   three endpoint normal slots and the fourth independent bank slot
   after them on every positive interval in the pricing layout,
   including inactive coordinates. Negate all assigned normals on
   the reflected leg. This declares finite-step latent coupling.
   [Canonical time grids](../core-numerics/time-grids.md),
   [checked matrix factors](../core-numerics/matrix-factorization.md),
   [joint correlation and bank slots](../models-monte-carlo/11-credit-line.md).

4. **Bind discounting and contractual rate projection separately.**
   Elect source-relative static discounting or fitted centered
   Hull–White with its complete authored or qualified fitted inputs.
   Fitted paths start with `x_A=0`. Use the joint Gaussian endpoint
   and rate integral with the exact deterministic shift. Discount
   using the path log bank account, retained at all required cash
   nodes. Do not reconstruct it from endpoint short rates. Exact
   zero fitted volatility preserves the curve fit and recovers the
   matched relative discount. Coupon type cannot select the bank
   account. Remove the explicit-level/theta override of this dated
   fitted pricing law.

   For coupons select the record 10 fixed rate, qualified
   deterministic index projection, or fitted stochastic term
   projection. Use actual published fixings at the information
   cut first. Model live term fixings at the authorized exact
   observation node on actual index value dates and contractual
   index accrual. Reconstruct positive gross from the conditional
   discount-bond ratio and deterministic multiplicative index
   basis. Apply the payment-date discount on the path once.
   Retire the additive short-rate basis proxy and any extra
   payment-measure correction on the modeled fixing. Keep
   deterministic coupon projection with stochastic discounting
   identifiable as an explicit model combination. Overnight
   honors record 10's original blocks and allocation with known
   or deterministic projected rates. Sampled overnight requires a
   separately qualified block-rate path and is unsupported here.
   [Dated fitted rates and term projection](../models-analytic/rate-clock-and-projection.md),
   [joint bank sampling](../models-analytic/hw1f-monte-carlo-discounting.md),
   [Hull and White](https://doi.org/10.1093/rfs/3.4.573).

5. **Elect credit authority and preserve literal spread laws.**
   No Default has unit survival and no active modeled default
   funding. Frozen Hazard uses the exact named issuer source and
   native local integrated hazard, including with modeled
   utilization. Spread Driven uses a literal constant, checked CIR,
   or the explicit mean-anchored CIR preparation below. A literal
   constant stays unchanged, including zero. Use the accepted QE
   transition for CIR with positive mean reversion, nonnegative
   level, volatility and initial state, and Feller as a diagnostic.
   Zero CIR volatility retains deterministic mean reversion.

   For Spread Driven define the annual model-clock intensity
   `λ=s/(1−R)` under the explicitly selected credit-triangle proxy,
   with facility recovery `0≤R<1`. Recovery of one is admissible
   with qualified Frozen Hazard. Finite model spread is distinct
   from loan margin, OAS and a tradable CDS quote. Use one actual
   credit authority per run. An unused curve cannot provide
   survival or imply a simulated factor. Reject missing active
   authority. Remove default `.1` mean reversion, `.3` volatility
   and correlation, synthetic CIR for Constant, and parameter,
   survival and LGD floors. A chosen zero-intensity model remains
   a valid limit even when the payoff has default funding terms.
   [Local dated hazard](../core-market/hazard-survival.md),
   [checked CIR QE](../models-monte-carlo/08-cir-qe.md),
   [Cox, Ingersoll and Ross](https://doi.org/10.2307/1911242).

6. **Keep Market Anchored as a one-horizon mean fit.** Require an
   explicit frozen ACT/365F issuer hazard source, actual anchor
   `A`, and covered authored date `T` strictly after `A` and
   no later than legal maturity. Set `Δ=(T−A)/365`,
   `s0=(1−R)λ(A+)` and `s̄=(1−R)J(A,T)/Δ`. Resolve the
   forward-side intensity from the first active source cell
   after the anchor. The source's left/ending point quote at a
   knot is a separate query. Other source clocks remain
   admissible for qualified Frozen Hazard. No silent clock or
   horizon conversion, tenor cap or cumulative survival ratio.

   With authored `κ>0` set `x=κΔ` and
   `a=−expm1(−x)/x`. Solve
   `θ=(s̄−a s0)/(1−a)` in scale with a stable complement.
   Enforce attainability, including exact zero `θ`. Negative
   or numerically unresolved `θ` fails. Require a positive
   finite absolute mean-spread residual tolerance and verify the
   returned parameters against the same target. Use the accepted
   `σ_CIR=ν sqrt(s̄)` with explicit annualized fractional
   volatility `ν`. Preserve the source, actual horizon, inputs,
   represented mean and residual. This fits the continuous CIR
   mean integrated intensity at one horizon. Report any
   simulated survival or market repricing separately. Curves,
   volatility and correlation require provenance and applied
   model evidence.
   [Accepted hazard anchoring and volatility map](../models-and-valuations-credit/05-hazard-and-toggle.md).

7. **Apply conditional survival once to each eligible cash claim.**
   Value signed borrower-survival cash on each factor path at
   `cash × D(V,t) × exp(−J(V,t))`. Independently qualified
   unconditional cash uses its own discount and entitlement
   without borrower survival. Owner terms classify future funding,
   upfront fees, interest, LC fees and delayed payments by claim.
   Include upfront cash once through the canonical ledger.
   Entitled same-day cash at the valuation phase uses factors
   of one. A recognized default, missed payment or known workout
   requires its own qualified claim treatment.

   Use direct local source integrals for Frozen Hazard. Spread
   Driven defines linear intensity between every actual model
   integration endpoint, with checked trapezoid integrals over
   full steps and integrated-linear partials where needed.
   Never linearly interpolate the cumulative hazard of that
   linear-intensity law. Form local conditional default mass
   from `−expm1(−H)` and carry cumulative log survival in scale.
   Require coverage through the last active contingent payment
   and default exposure. A later fee can require survival after
   recoverable principal is zero. Remove absolute survival
   ratios, previous-day exposure proxies and an implicit flat
   integrated-hazard tail. Compute every factor/product at its
   claim or event before aggregation.
   [Owner credit treatment](../cashflows/15-credit-adjusted-pv.md),
   [conditional intensity valuation](https://www.math.ku.dk/bibliotek/arkivet/preprints-fra-ims/1994/preprint_1994_-_no_9_lando__david_-_on_cox_processes_and_credit_risky_bonds.pdf).

8. **Integrate prompt default cash on prepared exposure phases.**
   Let `N` be eligible funded par plus distinct covered pending
   clean principal not already in funded stock. Exclude accrued
   cash, fees and premiums. A pending covered redemption can
   remain exposed after economic retirement through its
   authorized payment phase. Let `U` be eligible undrawn
   borrowing capacity net of LC use, and `L` the disjoint
   eligible contingent LC face at the lender share.
   Each exposure starts and ends at its own authorized phase.
   In the ordinary profile, draw availability expiry removes
   `U`. An independently surviving contingent obligation needs
   separate authority. Use authored fractions `q_U,q_L∈[0,1]`
   for funded conversion at default. These are valuation
   assumptions and have their own provenance.

   Select the prompt par-funded, prompt fractionally recovered
   profile with net signed lender event amount
   `K=R N+(R−1)(q_U U+q_L L)`. Converted exposure is funded
   at par and recovered once. Already funded exposure remains
   in `N` only. Require consistent rights, recovery, currency
   and outstanding phases. No implicit right to draw on default.
   Known reimbursement, collateral, different recovery
   seniorities, delayed recovery and actual remedy cash use
   their own qualified claim law. Keep prudential EAD,
   accounting expected loss and observed default separate.
   [Fractional recovery of par](https://www.darrellduffie.com/uploads/1/4/8/0/148007615/duffiedefaultabletermmodels1998.pdf),
   [filed LC reimbursement terms](https://www.sec.gov/Archives/edgar/data/40570/000147793223003960/job_ex101.htm),
   [regulatory EAD framework](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/36/inforce/2023-01-01/published/2022-12-08).

   Integrate on exposure-constant windows split by actual
   model, relevant source and economic boundaries. Before drawing,
   bind default-window endpoints needing fitted bank factors to
   exact model knots. A numeric source split can contribute a
   numeric knot without inventing a legal date or funding event.
   The open-window stock is after the left phase and before
   the right event,
   with zero continuous-default mass on event instants.
   For each window use local integrated hazard `H`, local
   log discount increment `Δlog D` and endpoint
   `G_i=log D(V,i)−J(V,i)`. Select the constant-equivalent
   hazard and log-linear discount bridge for the default
   weight. Use the checked analytic contribution
   `K H exp(max(G_a,G_b)) φ(|Δlog D−H|)`, with
   `φ(z)=−expm1(−z)/z` and `φ(0)=1`. This integrates the
   selected equivalent window law exactly. For curved paths
   it is a default-time approximation and requires refinement
   evidence. Preserve the sampled bank integral and selected
   conditional hazard values at cash
   nodes. A positive local hazard cannot disappear because
   endpoint survival values agree at represented precision.
   Apply the shared 100,000 civil-cell and 1,000,000 split-
   window bounds where using the dated static credit grid and
   enforce the admitted total path work on modeled grids.
   [Shared local integration and limits](../valuations-bonds/03-pv-models.md).

9. **Use explicit fixed-budget Philox estimation.** Require
   `2..=5,000,000` independent estimators, explicit reflected
   pairing selection and a literal `u64` seed or explicitly
   elected instrument/scenario FNV-1a derivation. Remove
   default run count, pairing and seed labels. Preflight
   counts, shapes, step and capture work, and reservations.
   Use absolute Philox stream ID equal to the independent
   estimator index, accepted normal profile and step-major
   slots. No hidden early stopping, replacement samples or
   partial estimate. Preserve the checked deterministic
   chunk and error order across execution modes.

   Discount each signed eligible claim and add modeled
   default contributions on the same path before forming
   its native PV. One ordinary path or one complete
   reflected pair mean is one observation. Report N and
   N or 2N physical paths, Bessel standard deviation of
   those observations, standard error and the fixed
   95% Student-t interval with N−1 degrees of freedom.
   Relative SE is unavailable at exact zero mean. A
   structurally deterministic or origin-only value reports
   that branch and actual work. If a requested modeled run
   produces constant observations, report a point interval
   with its conditional interpretation. For general payoffs
   the interval describes sampling uncertainty under the
   selected grid/model and is an approximation. Qualification
   needs separately declared timing and model tolerances.
   Compare refined grids with verifiably matched or coupled draws
   or a qualified independent reference. A shared seed
   cannot establish identical paths on changed layouts.
   [Accepted estimation](../models-monte-carlo/01-engine-and-estimation.md),
   [Philox provenance](../models-monte-carlo/02-random-streams.md).

10. **Separate expected schedules, diagnostics and pricing results.**
    Expected cashflows are the survival-unweighted performing-path
    projection on the run measure. Average signed eligible cash and
    principal across all physical legs with absent amounts contributing
    zero. Do not condition the sample on observed survival. Preserve
    full original parent/fee/funding identity, economic and payable
    dates and phases, with deterministic ordering and compatible
    context. Use a checked widened exact sum and divide once by the
    integer count before Money restoration. Publish rate diagnostics
    conditional on defined observations, with count and convention,
    or leave unavailable. Do not infer an amount from mean balance
    and mean rate. A projected event collection does not establish
    complete realized balances or recoverable stock. A tracked
    mean schedule requires a proven principal reconciliation at its
    published precision. Keep conditional cash, default PV and
    discounted pathwise pricing separate. Later quote and theta
    consumers must qualify this projected basis for their purpose.

    The normal price and expected schedule use the same admitted
    execution and stream aggregation without full capture. Existing
    `price_with_paths` is explicit unpaired all-path diagnostic
    capture, limited to 100,000 paths and 512,000,000 retained
    bytes under the accepted capture contract. A capture request
    with antithetics fails. Preserve exact estimator membership,
    complete recorded nodes and roles, cash and PV, run counts
    and replay metadata. Checked immutable restore validates
    grid, shape, domains, identity, units and membership.
    Reconcile Rust, Python and WASM typed pricing, schemas,
    UI and portfolio/attribution consumers. Full-width seeds
    remain exact across host and wire formats. Record 12 owns
    quote eligibility, credit and rate risk, and expected loss
    and draw-option counterfactuals. Its comparator must state
    whether credit-linked utilization changes. A common seed
    alone establishes no fixed-utilization counterfactual.
    [Path capture contract](../models-monte-carlo/28-capture-and-simulation-contracts.md),
    [cashflow export](../valuations-framework/cashflow-export.md),
    [revolver quote and metric owner](12-revolver-quotes-risk.md).

## Observed behavior

### Choice and rationale

[Unified dispatch](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/unified.rs) automatically chooses deterministic event valuation or Monte Carlo from `draw_repay_spec`, including through the registered Discounting key. Explicit `MonteCarloThreeFactor` requires stochastic inputs. Positive loan-equivalent/LC exposure requires a default model. The model combines utilization, rates and credit-spread factors, then values each schedule with conditional survival and a separate recovery/default-funding leg. Source rationale motivates adverse selection; the default parameter values remain adoption choices.

### Path algorithm and controls

[Path generation](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_generator.rs) starts at `max(issue,as_of)` with current utilization. The process clock is Actual/365 Fixed, distinct from coupon day count. Weekly-or-finer internal steps evolve the shared credit-line process; only contractual observation states are retained. Philox uses a separate deterministic stream per estimator. Optional antithetic sampling negates all normals and keeps pairs adjacent. Embedded [run defaults](../../../../finstack-quant/valuations/core/data/defaults/product_mc_defaults.v1.json) are 10,000 independent estimators, antithetic false; model overrides select count/seed/antithetic.

Utilization is bounded mean-reverting diffusion with spread sensitivity. At `abs(sigma)<1e-8`, the adapter restores the old utilization after every simulation step, freezing drift and spread response as well as volatility. At observations, cap utilization to `1−LC/commitment`. Fixed coupons have fixed rate state. Floating rates default to a deterministic forward curve. Positive-sigma Hull–White fits drift and initial short rate to the discount curve; zero-sigma mode uses caller initial/theta values. Positive-sigma overnight RFR is rejected because daily fixing paths are absent.

Credit choices are explicit CIR, Constant or MarketAnchored. CIR initial/theta/kappa are floored at `1e-8`; Feller violations warn and continue. Constant spread is actually mapped to CIR `(kappa=.01, theta=spread, sigma=.001, initial=spread)`, so positive constants still diffuse. MarketAnchored uses current hazard and conditional average hazard over remaining/capped tenor; with `LGD=max(1−R,1e-6)`, map `s0=LGD×lambda0`, `sbar=LGD×average_lambda`, and choose CIR theta to match the mean-average spread. CIR sigma is fractional spread volatility times square-root reference spread. Without explicit config, hazard-bearing facilities use kappa `.1`, fractional vol `.3` and utilization-credit correlation `.3`; without hazard, Constant(0) and no correlation. Full correlation matrix overrides the utilization-credit shorthand.

### Pricing algorithm

[Single-path pricing](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_pricing.rs) discounts contractual cashflows including signed future funding by `DF(as_of,t) × S(t)/S(as_of)`. Deterministic paths use the hazard curve or unit survival. Stochastic paths integrate `lambda=s/LGD` trapezoidally between retained observations, linearly interpolate cumulative hazard to cashflow dates, and set `S=exp(−H)`. Cumulative hazard is held flat beyond the final observation.

Positive-sigma rates select bank-account discounting: integrate piecewise-linear retained short rates exactly by trapezoids and exponentiate minus the integral; extend short rate flat beyond the grid. Otherwise use the static discount curve. Weekly simulation refinement therefore does not retain weekly rate/hazard integration accuracy.

Default/recovery integration merges monthly dates, cashflow observations, draw/repay and commitment/LC events, ending at **legal maturity**. For each conditional default-probability increment, use average endpoint DFs and average pre-event drawn exposure. Add
`PD × DF_average × [R × drawn_average + (LEQ × undrawn_average + LC_default_draw) × (R−1)]`.
The second term funds contingent draws at par and recovers only fraction R. Upfront fee is separately discounted without survival only if issue is strictly after `as_of`.

[Aggregation](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/stochastic.rs) averages path PVs. Complete antithetic pairs are one independent observation; sample variance is Bessel-corrected and the 95% mean interval uses Student-t. At least two independent estimators are required. Expected cashflow export averages amounts by date/reset/kind, with absent flows contributing zero; displayed rates average only present rate observations.

### Limitations and alternatives

This is conditional-intensity expected cashflow valuation, not sampled stopping at realized default times. MarketAnchored matches an average spread target, not the entire survival term structure. Recovery stops at legal maturity even if adjusted terminal cash payment is later, while path hazard freezes after final observation. Correlation-driven draw cost is modeled but not a borrower optimal-control solution. Deterministic hazard valuation is supported; exact integrated-rate paths, full survival calibration and endogenous optimal draw policy are unimplemented alternatives.

### Evidence

Colocated path tests cover anchor mapping and curve-fit selection; `path_estimate_uses_sample_stderr_and_student_t_interval` checks a hand-computed two-observation interval. [MC tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/mc.rs), [parity tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/revolving_credit_parity.rs) and [default-risk tests](../../../../finstack-quant/valuations/fixed-income/tests/default_risk.rs) are located evidence, not rerun or universal model validation. [Philox](../../../REFERENCES.md#salmon-2011-philox), [Monte Carlo](../../../REFERENCES.md#glasserman-2004-monte-carlo) and [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives) supply context.

## Independent arithmetic and qualification

- From utilization `.2`, target `.8`, mean reversion `1`
  and zero volatility, the half-year state is
  `.43608160417242`. A subsequent opening-spread change
  that raises the target to `.9` gives `.61861926932587`
  at year one. The unlinked target gives `.57927233529713`.
  For commitment 100, LC use 20, drawn 30 and candidate
  utilization .9, the admitted draw takes par to 80 with
  lender cash −50. A same-day limit increase to 120 leaves
  dollars at 80 and resets the utilization ratio to 2/3.
- A half-year fitted-rate step at zero mean reversion with
  absolute volatility `.02` gives state variance `.0002`,
  state/integral covariance `.00005` and integral variance
  `.00001666666667`. The independent residual variance is
  `.00000416666667`. This last load is needed even after
  coupling the rate endpoint with utilization and spread.
- A one-year mean fit at `κ=.1` with initial intensity
  `.10` and target mean `.02` needs a negative CIR
  long-run level. The minimum feasible mean is about
  `.095162582`. A separate continuous CIR intensity
  example with initial and long-run levels `.04`, mean
  reversion `.5`, volatility `.2` and horizon five years
  has expected integrated intensity `.2` and affine
  survival `.824463866`. The exponential of minus the
  mean integrated intensity is `.818730753`. [CIR affine law](https://doi.org/10.2307/1911242)
- Linear intensity rising from zero to .08 over one year has
  integrated hazard .01 at half-year and .04 at one year.
  Linear interpolation of the cumulative integral gives .02
  at half-year. If the local integrated hazard is `1e−20`,
  its default probability is of that order even when
  represented survival is one.
- With flat discount rate .04 and intensity .10, future issue
  at year one pays contingent lender cash −40 and fee +3.
  Through legal maturity at year two, eligible funded par 40,
  unused capacity 50, LC face 10, recovery .4 and conversion
  fractions .5 and .8 give prompt default cash −3.8. Through
  the authorized payment at year three, recoverable pending
  par 40 gives prompt default cash 16. At year three payable
  contingent principal and coupon total 42 and independent
  cash is 1. The total valuation is `−2.86321950561557`.
- From reflected pairs `(90,110),(124,126),(71,79)`, the
  three independent observations are `100,125,75`. Mean
  is 100, SE about 14.43375673, and the 95% interval is
  about `[37.89655721,162.10344279]`. Two scenario cash rows
  of 100 and 200 with factors .90 and .80 average to PV 125.
  The product of the separately averaged amounts and factors
  is 127.5.

Standalone checks cover the stated limits and formulas. Complete
qualification requires contract and observed status fixtures,
market and empirical parameter evidence, independent price and
survival comparisons, discount and default-time convergence at
funding/capacity and pending-pay boundaries, capture replay and
typed host results. Repository pricing and host suites were not run.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
