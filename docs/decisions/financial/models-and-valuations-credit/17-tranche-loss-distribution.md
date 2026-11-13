# MODELS-AND-VALUATIONS-CREDIT-17: Finite-pool tranche losses and base correlation integration

- Decision status: **Accepted — complete checked finite-pool tranche loss package, 2026-11-13**.
- Implementation status: Accepted target. The observed behavior appears below.
- Validation status: Source and primary references inspected; independent arithmetic and factor examples executed. Production pricing, calibration and host parity are unqualified.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked finite-pool tranche losses

1. **Bind the contract, composition, dependence law and source.**
   Require positive tranche notional and literal percent strikes
   `0<=A<D<=100`, including valid strikes and widths below 1%.
   Validate ordering on the supplied percent values, form the
   original width from `(D−A)/100`, and preserve the represented
   interval with checked geometry. No magnitude inference or
   minimum-width floor. Select Homogeneous or Named under
   [CORE-MARKET-19](../core-market/credit-index-composition.md)
   and explicitly select Gaussian, Student-t, random factor loading
   (RFL) or shared TwoFactorGaussian under
   [CREDIT-09](09-copula-kernels.md#accepted-decision-for-checked-copula-laws-and-latent-factors).
   The same selection serves standalone, registered and prepared
   routes; an absent copula is not Gaussian. Bind the base-correlation
   source to the exact pool/version, recovery and hazard basis,
   selected copula and parameters, market origin and contractual
   maturity under [CORE-MARKET-18](../core-market/base-correlation.md).
   A fit carries its actual provenance; a generic model key or copied
   correlation does not establish it. Preserve the selected literal
   correlation and PD endpoints. No source smoothing, clipping or
   cross-model substitution.

2. **Resolve seasoning from authoritative original-pool amounts.**
   At valuation close require recognized historical loss `L0`,
   recovered principal `G0` and the corresponding live pool
   share `F`, with coherent original-basis mass `L0+G0+F=1`
   under the composition representation budget. For a named pool
   reconcile exact original roster, live and exited identities,
   dates and recognized loss/recovery against original member
   weights. Keep each supplied live weight; shared hazard curves
   do not merge names. Preserve zero-weight identity. For a
   homogeneous pool require original count `N` and remaining
   count `n`, so a live name is still `1/N` of original
   pool and exited gross share is `(N−n)/N`. Form `F` from
   live exposures or the count ratio, not from a residual that
   can erase a small live balance. Do not redistribute rounding,
   infer past defaults from loss divided by today's LGD, or replace
   historical recoveries in a future recovery scenario. If recognized
   event amounts or pool state are unresolved, refuse the affected
   request. Cash entitlement, unpaid amounts and settlement timing
   belong to [CREDIT-18](18-tranche-projection.md).

3. **Use local dated marginal PDs and a named interim convention.**
   For each live member, use `p_i(t)=−expm1(−J_i(v,t))` on the
   admitted hazard source clock from valuation close `v` through
   the requested date `t`, under
   [CORE-MARKET-04](../core-market/hazard-survival.md). The curve
   may have an earlier base if both endpoints are supported.
   The loss engine takes strictly ordered requested live dates
   through maturity, with the valuation close as its zero-future
   baseline. It does not restart conditioning at coupon dates,
   run live PD backwards, or infer an expired state from rounded
   survival. This is a re-anchored live-pool law on those marginals;
   it does not infer a latent-factor posterior from past defaults.
   For each required cap use the contractual-maturity base
   correlation at its **original** percent strike on every
   projection date. This fixed-maturity reuse is the selected
   interim approximation. No time interpolation, maturity
   substitution or lookup at effective strikes. If coverage
   excludes pre-start defaults, refuse to use the valuation-close
   live pool as a future-start pool; a separately qualified
   forward-pool model is outside this package. No discount
   lookup is required for loss itself.

4. **Separate bottom-up loss from top-down principal erosion.**
   Write `a=A/100`, `d=D/100`, and `w=(D−A)/100`. The live
   interval sits between historical bottom loss and top recovery.
   For either original fractional strike `k` let `b(k)` and
   `u(k)` be the lengths of the live interval below and above it:
   mathematically `b(k)=clamp(k−L0,0,F)` and
   `u(k)=clamp((1−k)−G0,0,F)`. These are contractual intersections,
   with checked geometry and exact boundary identities. Prior
   tranche loss is `min(L0,d)−min(L0,a)`; prior recovery erosion
   is `min(G0,1−a)−min(G0,1−d)`. Both use original pool units.
   For future defaults use the same indicators `I_i` in loss
   `X=Σ w_i(1−R_i) I_i` and recovered principal
   `Y=Σ w_i R_i I_i`, where each `w_i` is a live original
   weight. With `C_X(k;ρ)=E_ρ[min(X,k)]` and likewise for
   `C_Y`, compute future tranche loss
   `C_X(b(d);ρ_D)−C_X(b(a);ρ_A)` and recovery erosion
   `C_Y(u(a);ρ_A)−C_Y(u(d);ρ_D)`. Subtract on the same pool
   basis before dividing by the original width. Only loss
   creates protection, while both reduce outstanding. Preserve
   historical amounts when the live layer is exhausted: future
   erosion is exactly zero there. Use zero-cap, uncapped marginal
   mean, deterministic and full-pool identities where established.
   With identical endpoint laws, evaluate the direct band payoff
   where needed to resolve narrow-width subtraction.

5. **Keep finite conditional default laws.** Conditional on
   the selected factors, use independent per-name Bernoulli
   events. For a homogeneous live pool use the checked finite
   [binomial PMF](../core-numerics/binomial-mass.md). For a named
   pool with exactly equal positive increments in the evaluated
   loss or recovery channel, a Poisson-binomial count recursion
   preserves each name's own conditional PD and the actual common
   increment. Select it when its state and work preflight passes.
   Otherwise select exact conditional subsets when their count is
   at most 200,000 and their state-visit preflight passes, or bounded
   grid convolution when the exact paths do not fit per-node limits.
   Select the path before quadrature from the admitted geometry.
   Strip only established deterministic events or exact zero
   contributions from the calculation, preserving the roster and
   probability basis. No tolerance-based homogeneity, positive
   weight pruning, large-pool approximation, Monte Carlo or
   alternate method chosen after failure.
   [Laurent and Gregory](https://www.maths.univ-evry.fr/pages_perso/crepey/Credit/cdo_laurent_gregory.pdf).

6. **Bound the approximate grid separately from integration.**
   When the grid is required, use an explicit finite positive
   step `h` in original-pool fractions. An increment
   `h(k+α)` uses adjacent offsets `k,k+1` on default with
   weights `1−α,α`; keep the no-default probability separately.
   Collapse grid points at or above the cap into an absorbing
   cap-payoff state. This must preserve the capped payoff of the
   unconstrained rounded recursion, including upper splits at
   the last bin. Check integer indices, shifts, contribution
   geometry and finite nonnegative mass. For each capped loss
   or recovery leg report a one-sided grid allowance:
   `0 <= exact−grid <= (h/2) sqrt(Σ p_i α_i(1−α_i))`
   in exact arithmetic, with unconditional marginal `p_i`.
   The adjacent split is mean preserving given the defaults;
   concavity of the cap and the expected absolute rounding
   error give this bound. Carry both caps' allowances through a
   band or base subtraction. This bound does not include factor
   integration or arithmetic. Limit each grid or subset to
   200,000 states and each conditional law calculation to
   5,000,000 state visits. Require explicit positive aggregate factor
   evaluation and conditional work limits across dates and
   legs. Reject unavailable resolution, excess work or a grid
   that cannot meet the declared allowance. Do not silently
   rescale PMF mass or discard a required split.
   [Andersen, Sidenius and Basu](https://www.risk.net/derivatives/credit-derivatives/1530334/all-your-hedges-one-basket)
   describes conditional recursion on a discretized loss distribution.

7. **Integrate the selected law with its actual factors and limits.**
   At exact `ρ=1` each supported law uses a common uniform
   default threshold; sweep the sorted marginal PD breakpoints
   and integrate the resulting step payoff exactly in law.
   Gaussian independence at `ρ=0` uses the unconditioned
   finite-pool law. Shared TwoFactorGaussian reduces to the
   same one normal for this one-pool expectation because both
   common coordinates and their shares are common to every
   name. This is not a sector-specific model. For interior
   Gaussian and conditional normal axes use checked open-node
   adaptive Gauss–Legendre order 8 under
   [CORE-NUMERICS-04](../core-numerics/quadrature.md) on
   `[−10,10]`, with initial panels of width at most one.
   For conditional score `(c_i−b z)/s`, where `c_i` is
   the appropriate threshold (multiplied by the square root
   of scale for Student-t), split at the interior points
   `(c_i+j s)/b` for `j=−8,0,8` when `b,s>0`.
   Reject required unresolved geometry. Use literal PD
   endpoints and separate analytic
   omitted normal mass `2Φ(−10)<1.524e−23` per truncated
   axis, scaled by the applicable payoff bound.
   For RFL, use CREDIT-09's checked shared loading, evaluate
   its zero and unit atoms at their full normal masses using
   independence and the common-uniform sweep, and integrate the
   continuous loading region within the normal bounds, splitting
   at its clipping breakpoints. Constant RFL reduces to Gaussian.
   For Student-t, integrate the inner normal conditionally on
   the shared Gamma scale under CREDIT-09; the normal is unused
   at `ρ=0` but the scale remains common. Compare normalized
   Laguerre orders 32 and 64 for that scale and use order 64
   only if the difference and inner estimator allowances pass
   the declared gate. No separate per-name scale, normal
   substitution or claim that order agreement controls true
   mixing error. [Demarta and McNeil](https://onlinelibrary.wiley.com/doi/10.1111/j.1751-5823.2005.tb00254.x).

8. **Budget actual work and economic resolution.**
   Require an explicit finite positive tolerance for each
   normalized loss and recovery result, expressed as a fraction
   of original tranche notional, and explicit adaptive depth and
   aggregate work limits. Convert the tolerance to original-pool
   units with the original width. Allocate per-component
   quadrature budgets before evaluations, with deterministic
   proportional panel shares and separate half budgets for
   inner and outer estimator terms on nested axes; propagate
   the actual conditional estimates and outer comparison without
   assuming cancellation. Charge the representation discrepancy in
   the live pool mass to arithmetic qualification where it affects
   the payoff; it does not authorize changing weights or prior
   amounts. Count actual callbacks and conditional
   state visits, including both Student-t orders, across the
   whole request. Use the core binomial mass gate on its own
   path. For the other positive recurrences and enumeration
   require compensated total mass within `32(n+1) EPSILON`
   of one for `n` contributing names. This is a sanity gate,
   not a price error bound. Use ordered compensated
   accumulation and stable scaled products. Carry grid, omitted
   mass, operational estimator and arithmetic allowances
   separately through subtraction, date differences and width
   normalization. Check the combined allowance against the
   requested tolerance; refuse unresolved arithmetic and
   exhausted work. State the selected paths, settings, actual
   work and allowances. A quadrature difference is an operational
   estimate; narrow-band or rare-tail accuracy still requires
   independent qualification.

9. **Reject inconsistent tranchelets at every requested date.**
   On the common original-pool basis, both future erosion
   components must be nonnegative, their sum must fit within
   the surviving tranche width, and both must be nondecreasing
   from the valuation baseline across ordered dates. Carry the
   appropriate one-sided and symmetric allowances into these
   comparisons. Preserve a feasible computed result and disclose
   its uncertainty near a boundary. An infeasible result beyond
   the applicable operational allowance is a model-consistency
   failure under the selected profile; an infeasible result
   overlapping the bound is numerically unresolved. Do not
   publish either as qualified loss, or floor a component,
   replace it with a cumulative maximum, or cap it to make
   outstanding positive. Do not drop checks at interim dates
   or on the recovery side. These checks apply to the requested
   dates and tranche; passing does not establish a globally
   arbitrage-free loss surface or joint dynamics.
   [Brigo, Pallavicini and Torresetti, sections 3–5](https://arxiv.org/html/0912.5427v3#S3).

10. **Reuse one checked result across product and host routes.**
    The immutable Rust owner returns dated future loss and
    recovery erosion, historical-inclusive cumulative fractions,
    and outstanding on the original tranche basis. Preserve
    that state at zero live width. The expected-loss metric is
    unsigned future loss Money from the same maturity node,
    valuation close and admitted model, without a separate
    Gaussian or maturity-only shortcut. CREDIT-18 owns payment
    dates, cash entitlement, accrual and protection timing and
    must consume the checked loss increments without repairing
    them. [CREDIT-19](19-tranche-risk.md) owns risk experiments
    on the same selected engine; calibration owns fit objectives
    and must use the identical valuation and allowance basis.
    Reuse preparation only for matching pool/state, date,
    source, full law and numerical controls. Reconcile direct,
    registry, prepared calibration, projection, metrics, Python
    and WASM routes, strict restore, declarations, schemas,
    parity, docs and fixtures. No new host model or numerical
    implementation. Stochastic recovery, bespoke mapping and
    full dynamic loss or cash-waterfall modeling are separate.

## Observed behavior

### Choice and rationale

Base-correlation equity-loss differences use finite binomial pools or conditional heterogeneous convolution, rather than an automatic large homogeneous pool approximation. Exact small-pool state enumeration retains individual losses around tranche payoff kinks. Alternative copulas are explicitly constructible; adoption remains pending.

### Scope and input domain

CDS tranches with percent attachment/detachment, positive tranche notional, complete issuer curves when heterogeneous, issuer weights summing to one within 1e−9, and credit curves based on valuation date for live pricing. Detachment at or below 1 percent is rejected as fraction-like input, which also rejects legitimate sub-1-percent tranches.

### Algorithm

1. Infer historical defaulted pool fraction from realized loss divided by floored LGD. Erode the original pool bottom by losses and top by recoveries, divide surviving attachment/detachment by surviving pool factor.
2. Read base correlations at original attachment/detachment. Exponential boundary smoothing operates around .01/.99 with width .005; probabilities are clipped at 1e−12 before copula thresholds.
3. Conditional homogeneous defaults follow a finite binomial PMF. Sum capped loss `min(k/N × LGD,K)` and separately capped recovered amounts. Heterogeneous pools with equal PD/recovery/weights within 1e−9 take the homogeneous path; otherwise enumerate all 2^N states when within 200,000 states, else convolve a .001 portfolio-fraction loss grid with linear split of each name’s exact loss between adjacent bins. Reject grids at the size limit or above 5,000,000 issuer-grid visits.
4. Gaussian factors use adaptive Simpson over [−10,10], segmented at unit widths, absolute portfolio-fraction tolerance 1e−10 and depth 20. RFL and multi-factor use nested adaptive normal integration with that budget. Student-t delegates to its product Gauss rule; the adaptive tolerance is not its error guarantee. These tranche-specific choices override generic Gaussian/RFL/multi-factor quadrature in [copula kernels](09-copula-kernels.md).
5. Subtract capped equity expectations at detachment and attachment, scale by surviving pool and original width. At maturity negative loss differences beyond the integration error budget produce a base-correlation arbitrage error; smaller numerical residuals clamp to zero. Interim negative loss differences are floored. Recovery-side negative differences are floored without the strict maturity loss-side contract. Successive expected losses and recovery writedowns are made nondecreasing, then writedown is capped by remaining loss capacity.

### Selection and controls

Default/new pricer is Gaussian; `with_copula` explicitly builds an alternative. Presence of issuer curves selects heterogeneous calculation. Grid and integration controls are constants, not public overrides. The .001 convolution grid has discretization error separate from the factor-quadrature budget.

### Conventions

Attachment and detachment are percent (3 means 3%); recovery, default probability and correlation are decimal fractions. Realized loss is a fraction of original pool, not tranche notional. Hazards use each curve's own clock subject to the live base-date restriction; discounting uses relative factors on the discount curve's own axis. EL fractions used for cashflows include prior loss; the separate expected-loss metric reports modeled future loss amount.

### Limitations, failure and fallback

Grid convolution is discretized even though some diagnostics call it exact. Clamping and monotonicity repairs can hide interim base-correlation inconsistency; only maturity loss receives the strict tranchelet arbitrage test. Recovered-principal top-down erosion assumes index-level recovery for realized state. Smoothing also prevents direct pricing of exact independent/perfect-correlation limits.

Invalid copula parameters, incomplete issuer coverage, resource limits, nonfinite integrands and failed adaptive convergence return errors. No large-pool or Monte Carlo fallback is selected. Loss/recovery repairs and clipping do not prove general base-correlation arbitrage freedom.

### Alternatives and tradeoffs

Large-pool asymptotic approximation, FFT convolution and Monte Carlo are possible but unselected here. Explicit finite pools preserve granularity; the bounded grid controls work at a measurable discretization cost.

## Evidence

Owners: [expected_loss.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/expected_loss.rs), [heterogeneous.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/heterogeneous.rs), [integration.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/integration.rs), [sensitivities.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/sensitivities.rs), [engine.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/engine.rs), [pricer.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/pricer.rs). [Pricing tests](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/tests.rs) and [tranche integration tests](../../../../finstack-quant/valuations/credit/tests/cds_tranche) are invariant/regression evidence, not a complete external accuracy grid. See [family selection](0031-tranche-loss-model-selection.md).

- [Instrument dispatch](../../../../finstack-quant/valuations/credit/src/cds_tranche/types.rs) and [family registration](../../../../finstack-quant/valuations/credit/src/family.rs) establish default-pricer reachability. Pricing tests include homogeneous/heterogeneous agreement, maturity arbitrage errors, survival-weighted timing, recovery writedown, curve rebasing and explicit state-enumeration/factor-grid comparison for heterogeneous tail loss.
- The [loss reference fixture](../../../../finstack-quant/valuations/credit/tests/fixtures/production_tranche_loss_reference.json) identifies SciPy binomial CDF, QUADPACK and composite Gauss–Legendre values. Its located evidence does not qualify provenance or the full copula/grid domain under [ADR 0014](../../project/0014-financial-reference-evidence.md); no generators were rerun.
- Technical sources: [Laurent–Gregory](../../../REFERENCES.md#laurent-gregory-2005-factor-copulas), [Gaussian copula](../../../REFERENCES.md#li-2000-gaussian-copula), [Student-t](../../../REFERENCES.md#demarta-mcneil-2005-t-copula), [random factor loadings](../../../REFERENCES.md#andersen-sidenius-2005-rfl) and [finite-pool/multi-factor loss methods](../../../REFERENCES.md#andersen-sidenius-basu-2003).

### Independent review evidence

- Exact rational checks verified the seasoned payoff partitions
  and pathwise capacity under a common law on 55,055 scenarios.
  With historical loss `.12`, recovered principal `.08`,
  live weights `(.30,.50)`, recoveries `(.25,.60)` and
  independent PDs `(.20,.50)`, the `[10%,85%]` tranche has
  historical loss `.02`, future loss `.145` and future
  recovery erosion `.123`, all in original pool units. The
  expected remaining balance is `.616` of tranche notional.
- Two independent names with PD `.5`, increments `.0006`
  and `.0014`, and cap `.0007` give exact capped expectation
  `.000500` and `.000455` on a `.001` split grid. The
  absorbing-cap recurrence agrees with full rounded enumeration
  and preserves mass. Exact count probabilities at conditional
  PDs `(.2,.4,.7)` are `(.144,.468,.332,.056)`; common
  increment `.005` and cap `.008` give expectation `.005444`.
- Two names with PD `.10` and loss `.30` per name give
  expected loss capped at `.30` of `.057` at Gaussian
  independence, about `.0300297043` at `ρ=.999999` by
  independent split quadrature, and `.030` at perfect
  correlation. Separately, Student-t with `ν=4`, `ρ=0`
  and marginal PD `.025` gives joint default probability
  about `.002714`, versus `.000625` under independence.
- For two equal-weight names with recovery `.4` and
  `[3%,5%]`, use Gaussian `ρ_A=0`, `ρ_D=1`. Equal PDs
  of `.10` and then `.50` give exact tranchelet loss
  `−.0007` and then `+.0025` of original pool. In a
  different `[20%,32%]` example with the same correlations,
  raising the two PDs from `(.30,.05)` to `(.31,.15)`
  raises both capped expectations, yet tranche loss falls
  from `.024` to `.0133`. Two equal PDs of `.8`, strikes
  `[1%,2%]` and the reverse correlations give positive loss
  `.0112`, greater than the available `.01`. Mirroring
  the first example with `[95%,97%]` and reversed
  correlations produces recovery erosion `−.0007` at
  the interim date. These exercise distinct failure gates.
- Open evidence includes primary dated market cases and fit
  replay, near-one correlation and narrow layers, large named
  pools and work limits, RFL atoms and Student-t mixing tails,
  projection cash and native/host route equivalence.

## Relationship to the accepted base-correlation boundary

[CORE-MARKET-18's complete accepted package](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks)
requires checked detachment-percent queries without magnitude-based fraction
inference, actual maturity/pool/model/recovery association, and exact admitted
source parameters. Implement supported endpoint limits or refuse them rather
than smoothing into an interior band. Common pool-basis capped equity losses
must be subtracted before tranche-width normalization; increasing rho alone
does not establish consistent losses. Interim-date reuse and original-versus-
effective strikes remain explicit model choices. Record 18 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's complete loss/recovery floors, quadrature,
convolution, realized-state mapping or arbitrage acceptance methodology.

## Relationship to the accepted credit-pool boundary

[CORE-MARKET-19's complete accepted package](../core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts)
requires explicit pool mode and authoritative member probability/cash-recovery/
weight inputs, strict lookup and the stated compensated mass gate. Preserve
zero-weight roster identity and distinct default events even when hazards are
shared. An approximate-homogeneity shortcut is not exact source equivalence;
qualify an approximation explicitly or preserve the actual law. Reconcile named
PDs with accepted hazard date/context/conditional-probability semantics rather
than clipped 1-sp with default context. Record 19 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's full copula/convolution/quadrature/loss-recovery model.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
