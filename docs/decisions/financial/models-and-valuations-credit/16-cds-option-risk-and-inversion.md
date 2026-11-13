# MODELS-AND-VALUATIONS-CREDIT-16: CDS option risk and volatility inversion

- Decision status: **Accepted — complete checked CDS option risk and implied volatility package, 2026-11-13**.
- Implementation status: Accepted target; current behavior differs as recorded below.
- Validation status: Source inspected and independent arithmetic probes completed. Production pricing, replay and host parity have not been qualified.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked CDS option risk and implied volatility

1. **Use one pricing contract and resolved scenario context.** All
   experiments consume the raw holder PV, branch and sources selected by
   [CREDIT-15](15-cds-option-quadrature.md#accepted-decision-for-checked-cds-option-valuation).
   Freeze notional, legal and payment dates, exercise and settlement
   terms, strike cash and annuity, factors and historical losses, and
   unrelated market inputs. On spread, rate and recovery shocks hold
   the resolved annual model volatility fixed, including when it was
   obtained from a surface. Rebuild affected dated legs, survival and
   front-end protection, then refit the state mean where required.
   Apply identical risk definitions to spread and clean-price strikes.
   No volatility requery or premium reinversion during a nonvolatility
   shock. Compute from raw scenario PV before rounding Money for display.

2. **Select central quote CS01 and spread gamma.** Use
   [CREDIT-13](13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk)
   par-risk replay for explicit bound quote IDs, a common finite
   positive bump `h` in bp, parallel or one-exact-quote shocks, and
   unchanged original recipe conventions, documentation clause and
   market dependencies. Use the qualified zero-shock par-risk center
   for any stencil that needs it. Report signed central
   `CS01=(V_+−V_-)/(2h)` in currency per bp. For parallel
   `spread_gamma` report `[(V_+−V_0)+(V_-−V_0)]/(h/10000)^2`
   in currency per squared decimal spread. Keep qualified curve and
   quote IDs, the nominal common step and the represented moves.
   Reject invalid or unresolved moves and missing or failed required
   replay. Remove the Black screen Delta, fixed-bump change-in-Delta
   Gamma and strike-dependent rebootstrap formulas. Bucket sums have
   no exact finite equality to the parallel experiment.

3. **Use a matched hedge diagnostic and a separate forward par display.**
   When explicitly requested for a live residual, the reference is
   the protection-buyer clean delivered forward before strike or
   historical cash, on the same discounted surviving notional basis
   as CREDIT-15: `B=N f D(v,z) Q(v,e) F`, with `f=1` for
   single-name and the current live factor for an index. It excludes
   front-end protection, strike, confirmed index loss and other
   cash outside the clean residual. Use the same quote endpoints as
   option CS01. Report `underlying_spread_dv01=(B_+−B_-)/(2h)`
   in currency per bp and `hedge_delta=(V_+−V_-)/(B_+−B_-)`
   in reference units per option position as priced. The latter is
   dimensionless and the negative ratio gives the offsetting units
   for that stencil. Require a qualified, nonzero hedge span;
   an absent residual or a denominator indistinguishable from zero
   under the stated allowances makes the ratio unavailable. The
   reference is neither a spot CDS nor a published screen ratio.
   Separately, let `A_z=A_dirty,e/D(e,z)−α` and
   `P_z=P_e/D(e,z)` from the dated clean legs. Report
   `forward_par_spread=10000 P_z/A_z` in bp with a resolved positive
   `A_z`. It is the coupon that zeros the clean residual, excluding
   strike and front-end or confirmed loss. An absent residual or an
   invalid coefficient is unavailable. Do not replace it with the
   loss-adjusted displayed forward or the calibrated spread location.
   [White, sections 8.2–8.4](https://quant.opengamma.io/CDS-Options-OpenGamma.pdf#page=35)
   provides market context for the distinct risk and display bases.

4. **Specify annual model Vega and one-day diffusion Theta.**
   `model_vega_01` is holder currency per +.01 absolute annual
   model volatility, using an explicit finite positive decimal
   bump `h_σ` with central formula
   `.01(V(σ+h_σ)−V(σ−h_σ))/(2h_σ)`. If exactly one full side
   is outside `[0,5]`, use the other side and the same qualified
   center, orienting the slope positive-up and scaling by `.01/h_σ`.
   Check domain before pricing; neither side admissible or a
   represented move lost to rounding is an error. No clipping or
   fallback from failed pricing. Refit the mean at each new variance.
   `diffusion_theta_1d` is `V(T')−V(T)` in holder currency,
   with `T'=(days_to_expiry−1)/365` using the civil-day count for an
   eligible live branch. Keep all dated cash, the dated mean target,
   discount, survival and annual model volatility fixed and refit
   the location at `T'`. On the last live day the comparison uses
   the deterministic mean at zero variance, without inferring a
   fixing or moving any contract date. This is a fixed-market
   diffusion change; full date roll, funding carry and expected
   lifecycle transitions require their own policies.

5. **Keep rate and recovery basis explicit.** Parallel and bucketed
   DV01 each require the two independent CREDIT-13 selections:
   typed rate-quote rebuild or direct continuous-zero shock, and
   frozen active hazards or refit of affected hazards to unchanged
   original observations. Respect exact targets and rate geometry,
   source dependencies and conventions. Use the same checked central
   positive-up rate stencil in currency per bp. Recovery01 requires
   frozen intensity or original-quote-preserving replay explicitly.
   Reprice the option in either mode: recovery can change both the
   dated anchor and the bounded state law. Synchronize future pricing
   recovery and the affected source view; a frozen intensity view
   makes no claim to reproduce its old recipe at the new recovery.
   Leave strike and realized events fixed. Use the CREDIT-13 whole
   scenario recovery stencil, with explicit positive decimal step,
   central when both sides admit `[0,1)`, and positive-up one-sided
   only when exactly one whole direction is outside that domain.
   Report currency per +.01 absolute recovery. No automatic
   freeze, clipped shocks, or affine CDS-leg shortcut for an option.

6. **Follow lifecycle and actual dependencies for each measure.**
   Validate requests before giving structural zero. A fully
   depleted index, confirmed knockout or resolved exercise has no
   live diffusion risk. Fixed unpaid cash can carry rate risk; a
   same-day entitled amount has relative discount factor one.
   A physically delivered trade still represented by the option
   follows the delivered product's risk owners and includes any
   remaining eligible exercise cash, without duplication. Stop
   including those dependencies at transfer. A live zero-vol
   option can have nonzero Vega; an intrinsic or linear payoff
   at the base inputs need not be insensitive to changes in
   curves or recovery. Prove independence for the selected
   experiment before bypassing its pricing dependencies. A zero
   due to tails, rounding or unresolved arithmetic is not an
   exact zero. Do not substitute zero for unavailable IV, missing
   hedge or forward par display, or failed pricing.

7. **Invert only an identified total model premium.** Require
   finite nonnegative total premium and a positive repricing
   tolerance in the contract currency, with the same valuation
   close, long-option notional and raw settlement and front-end
   basis as CREDIT-15. Use currency-checked monetary inputs.
   Exclude portfolio weights and presentation adjustments. Require
   no pre-existing volatility or surface. For `T>0`, a mean
   strictly inside support, nonzero stochastic exposure and a
   hurdle strictly inside the cash support, the exact model PV is
   strictly increasing in `σ` for payer and receiver alike.
   With the mean refit, the two lognormal quantile functions for
   different positive widths cross once. The increasing mark
   preserves the crossing; equal means then give strictly more
   expected positive-part payoff at every interior hurdle for the
   wider distribution. At zero variance the same comparison follows
   from the deterministic mean. Test the single-name hurdle `G`
   against `[−cH,g]` and the index hurdle `G−L` against
   `[−fcH,fg]` without dividing by the live factor.
   At or outside support, at the exact lower mean, or with no
   stochastic exposure, price is independent of volatility. A
   matching flat live premium gives a not-identifiable error,
   a definitely inconsistent one gives out-of-range, and an
   unresolved comparison reports numerical indeterminacy.
   Resolved lifecycle branches have no implied model volatility.

8. **Use the closed interval and independently check the answer.**
   Evaluate the qualified model at `0` and `5` annual decimal
   volatility, using exact zero-variance pricing at zero. Reject
   a target definitely outside the endpoint range. Accept a literal
   endpoint only when its residual overlaps zero under the
   numerical allowances and meets the repricing gate below;
   otherwise an unresolved endpoint comparison fails. If both
   endpoint comparisons overlap, the range is unresolved. For a
   resolved interior bracket, use shared checked
   [Brent](../core-numerics/brent-root.md) directly in `σ` with
   absolute coordinate tolerance `1e−10` and at most 100 updates.
   Use actual checked PV residuals. Use an interior sample to
   update the bracket only when its residual sign is resolved
   under its allowance. If its sign overlaps zero, it can stop
   the owner search as a candidate only if it meets the repricing
   gate; otherwise return numerical indeterminacy. This owner
   stop preserves the actual residual and does not supply a
   synthetic zero to Brent. No positive floor, seed or arbitrary
   root choice. Independently
   reprice the actual returned value, requiring
   `abs(PV−target)+allowance <= premium_tolerance` in common
   currency units. Propagate domain, arithmetic, mean-fit,
   integration, work and price resolution failures. The premium
   gate gives no guarantee of volatility accuracy near flat Vega.

9. **Carry numerical qualification through the reported experiments.**
   Use CREDIT-15's support-scaled interior estimator budget and
   omitted-tail allowance plus the observed forward mean-fit
   residual allowance in each repricing. At fixed volatility,
   coupling two spread locations with the same normal shock orders
   their marks pointwise. The change in expected positive-part
   payoff is bounded by the mean change times the live factor
   (one in the single-name inner payoff); apply the same notional,
   survival when applicable, and discount scales as pricing.
   Include relevant checked rounding allowances. Carry scenario
   allowances through the absolute stencil weights and check
   hedge span resolution before dividing. Label estimates whose
   differences cannot resolve a sign under those allowances, and
   fail if a required comparison or arithmetic cannot be resolved.
   A materially negative live model Vega against the monotonicity
   rule is a pricing inconsistency; do not clip it. Operational
   integration allowances are not certified true-error bounds or
   claims of convergence to derivatives.

10. **Reconcile Rust, registry, hosts and risk consumers together.**
    Use one Rust owner and common replay context for direct and
    registered risk. Expose the standalone typed `implied_vol`
    operation through both hosts; remove the CDS-option generic
    premium-override metric route that prices a base PV first.
    Do not change unrelated products' overrides. Replace ambiguous
    CDS-option generic Delta/Gamma/Vega/Theta, underlying spread
    DV01 and par-spread entries with the selected names and units.
    Keep rate, credit and recovery quote risk basis-qualified.
    Record active branch and source identities, native strike and
    resolved volatility basis and coordinate, shock targets,
    quote recipes, represented moves, stencil, units and
    numerical qualification. A potential source declaration does
    not imply runtime dependence. Portfolio and attribution may use
    only compatible additive units and matched shock bases: hedge
    ratio is not a currency risk, diffusion Theta is not carry, and
    point-vol Vega is not exposure to an unrelated surface move.
    Reconcile existing bindings, strict errors and parity, schema
    and metric catalog, reporting, attribution, fixtures and
    examples. Hosts only convert values and propagate Rust results.

## Observed behavior

### Choice and rationale

Public option risk mixes screen conventions and repriced sensitivities. Spread-strike Delta/Gamma are Black screen quantities, while clean-price strikes use quote-rebootstrap hedge ratios; these are not uniformly derivatives of the quadrature PV.

### Scope and input domain

Live CDS options under record 15. Price-strike Delta/Gamma and CS01 require replayable hazard quotes and a recalibration provider. Volatility inversion accepts finite nonnegative currency premium and positive model volatility within [1e−6,5].

### Algorithm

1. Spread Delta is Black N(d1) (payer) or N(d1)−1 (receiver), using the loss-adjusted displayed forward, strike, sigma and expiry time. Nonpositive inputs or near-zero σ√T return zero. Spread Gamma is Delta(forward+5bp)−Delta(max(forward−5bp,1e−12)), without dividing by the bump.
2. Clean-price Delta divides the option PV difference by the synthetic underlying CDS PV difference under symmetric ±1bp par-quote hazard rebootstrap. Both legs hold native strike and resolved sigma fixed. Gamma is the difference of these hedge ratios under outer ±5bp quote bumps. Missing provider or near-zero underlying difference fails.
3. Underlying SpreadDv01 is one-sided +1bp quote-rebootstrap CDS PV change; it is not the symmetric denominator used by Delta. Option CS01 uses shared quote-rebootstrap logic. DV01 shifts only discount rates and freezes the hazard curve, while Recovery01 changes contract recovery and replays available hazard calibration.
4. Vega is PV(σ+.01)−PV(σ), recalibrating the spread mean at each sigma; it errors if the bump exceeds 5. Theta reduces diffusion time by 1/365.25 year with calibrated m, schedules, curves and settlement DF held fixed; it is not a full date roll.
5. Implied volatility Brent-solves premium residual in log sigma, tolerance 1e−10, bounds [1e−6,5]; endpoints must bracket the target. Expiry at/before as-of returns zero before pricing, even though direct PV rejects post-expiry valuation.

### Selection and controls

Strike kind dispatches screen risk. Direct CdsOption.delta/gamma passes no provider, so clean-price strikes require the metric engine for these quantities. Standard sensitivity controls set rate/CS01 bumps; Delta/Gamma/Vega screen bumps are fixed constants. Bucketed DV01 delegates triangular key-rate framework logic.

### Conventions

Delta is a dimensionless hedge ratio; Gamma is the change across the full ±5bp screen interval, not currency per decimal-spread squared. Vega is currency change per +1 volatility point; Theta is currency per one fixed-model day. CS01/DV01 are currency per bp.

### Limitations, failure and fallback

Screen Delta returns zero in the zero-vol limit rather than deterministic exercise Delta. CS01/Delta/Gamma return zero at expiry whereas cash-settled PV can still carry payoff; IV also returns zero after expiry. Quote replay for price-strike Delta/Gamma and underlying SpreadDv01 leaves doc-clause/valuation-convention overrides absent, unlike generic option CS01. Fixed mean Theta and frozen hazard DV01 need explicit consumer interpretation.

### Alternatives and tradeoffs

Full PV bump Greeks, analytic differentiation through the mean calibration, and full market-date-roll Theta are alternatives requiring a separately named convention. Current supported screen and quote-risk measures should remain distinguished.

## Evidence

Owners: [option metric implementations](../../../../finstack-quant/valuations/credit/src/cds_option/metrics), [pricer.rs](../../../../finstack-quant/valuations/credit/src/cds_option/pricer.rs), [bloomberg_quadrature.rs](../../../../finstack-quant/valuations/credit/src/cds_option/bloomberg_quadrature.rs). pricer.rs lifecycle tests and the metric implementation formulas provide behavioral evidence; [Bloomberg CDSO](../../../REFERENCES.md#bloomberg-cdso) and [Black 1976](../../../REFERENCES.md#black-1976) provide convention/background pointers, not proof of this implementation’s parity.

[Direct option methods](../../../../finstack-quant/valuations/credit/src/cds_option/types.rs),
[generic scalar pricing](../../../../finstack-quant/valuations/src/pricer/json.rs)
and [metric registry](../../../../finstack-quant/valuations/core/src/pricer/registry.rs)
locate the differing replay and prior PV requirements.

### Independent review evidence

- Refit and integrate the CREDIT-15 unit model independently using split
  composite Simpson over `[-12,12]` and bisection, with `H=5`,
  `g=.6`, `c=.01`, `T=1`, mean `.04`, `G=.04605548253282`
  and unit `N D Q`. Payer premiums at `σ=0,.2,.4,.8,5` are
  approximately `0,.0042198219,.0107003300,.0230054964,.0651451130`.
  Bisection recovers `.4` to the shown precision. At `σ=.01`,
  the positive premium is about `1e−16`, below the retained
  model's tail allowance. With `G=.61` the payer premium is
  structurally zero across the whole interval.
- In a synthetic continuous flat forward example with `N=10m`,
  `D=1`, `H=5`, `g=.60`, `c=.01`, `T=1.2`, `σ=.40`
  and strike cash frozen at `(.02−c) A(.02)`, apply a ±1bp
  stencil around spread `.0225`, with `F=(s−c)A(s)` and
  `Q=exp(−sT/g)`. Refit the mean each time. Payer and receiver
  CS01 are approximately `+2741.96` and `−1375.82` currency/bp
  against reference `+4029.72` currency/bp. Hedge ratios are
  `.680435` and `−.341417`. This checks formula and units,
  without qualifying dated market replay.
- In a matched index example with `N=1m`, `D=.98`, `H=5`,
  `g=.6`, `c=.01`, `f=.85`, `L=.02`, `F_*=.03`,
  `G=.0455`, `σ=.40`, and `T=1`, independent split quadrature
  gives payer and receiver PV about `9810.80`. Central Vega with
  step `.01` is `+239.11` currency per vol point. Shortening
  the diffusion clock one day and refitting gives `−13.11`
  currency for both; holding the calibrated location fixed instead
  introduces about `2.10` currency of spurious parity difference.
  A separate confirmed `10000` cash claim at half a year and
  continuous rate `.04` gives direct ±1bp DV01 about `−.49010`
  currency/bp, with no diffusion dependency.
- In a separate continuous flat single-name illustration at zero
  volatility, let `N=1m`, `D=.98`, `H=5`, `c=G=.01`,
  `R=.4`, expiry one year, and intensity `λ=.03` at center.
  Use `Q=exp(−λ)`, `F=[(1−R)λ−c](1−exp(−λH))/λ`
  and a full remaining payer front-end window. The `.01`
  recovery stencil yields about `−1614.35` with frozen intensity,
  versus `−60.30` if flat par spread `.018` is held and
  `λ=.018/(1−R)` is refit. This isolates the basis choice.
- Production evidence remains open: check quote center and actual
  endpoints on qualified dated replay, both strike kinds, survival,
  default and settlement branches, both rate and recovery bases,
  bump-size convergence and error amplification, payer/receiver
  parity, IV endpoint/flat/near-flat cases and exact host parity.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
