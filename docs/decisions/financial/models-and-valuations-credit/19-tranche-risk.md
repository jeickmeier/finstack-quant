# MODELS-AND-VALUATIONS-CREDIT-19: Tranche spread, quote, rate, correlation, recovery and default risk

- Decision status: **Accepted — complete checked tranche risk and default scenarios package, 2026-11-13**.
- Implementation status: Accepted target. The observed behavior appears below.
- Validation status: Source and primary references inspected. Independent arithmetic examples executed. Production repricing, replay, market benchmarks and host parity remain open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked tranche risk and default scenarios

1. **Use the accepted tranche valuation for every experiment.**
   Resolve the [CREDIT-17](17-tranche-loss-distribution.md#accepted-decision-for-checked-finite-pool-tranche-losses)
   pool, copula and loss controls and
   [CREDIT-18](18-tranche-projection.md#accepted-decision-for-checked-tranche-cashflows-and-settlement)
   cashflow and settlement basis once. Use signed native economic
   PV before Money finalization for every comparable endpoint
   and any required center. Hold valuation close, position,
   original strikes and weights, contract dates and cash cutoff,
   confirmed history and booked amounts fixed. Discount
   eligible booked cash on the scenario discount source for rate
   risk. Keep the copula, its shape controls and its maturity-bound
   correlation parameters fixed except for the requested
   correlation shock. Atomically associate the scenario with its
   changed inputs and retain the qualified base fit as provenance.
   A shock does not certify a new tranche market fit. Share the
   selected Rust model across direct, registered and prepared
   repricing, without substituting Gaussian or mixing rounded
   and raw PV. Reporting currency conversion and external price
   adjustments remain at their existing valuation boundaries.

2. **Report coupon risk, par and expected loss on their own bases.**
   Replace the tranche `spread_dv01` label with the existing
   `risky_pv01` on the required CREDIT-18 clean or dirty residual annuity
   basis. Report the nonnegative coefficient `A_basis/10000`
   where that annuity is admitted. An exact zero coefficient
   is available on an established basis. A negative clean
   coefficient does not qualify as risky PV01. The corresponding
   holder PV change per +1bp coupon is `−side × A_basis/10000`
   with `side=+1` for the protection buyer. Keep the selected
   basis and coupon sign convention in the result. Quote
   residual par as `10000 P/A_basis` in bp from those same
   legs, excluding booked upfront and separately confirmed
   cash. Require the live residual and resolved positive
   annuity for par. Zero protection under an admitted live
   model gives zero par. Remove the par cap and arbitrary
   small-annuity shortcut. Expected loss is unsigned,
   undiscounted future protection in native currency,
   `N × L_future(M)` from the same checked maturity node as
   projection. It excludes historical loss and recovery
   erosion. At maturity close or with exhausted live width,
   residual par is unavailable and future protection is zero
   after validating state. Economic PV retains eligible cash.

3. **Measure CS01 in exact credit par-quote space.**
   Resolve active hazards from the priced branch: the common
   hazard in homogeneous mode, or the positive live member
   hazards in named mode. A stored named-pool index curve
   without an active role is excluded. Use
   [CREDIT-13](13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk)
   and [CORE-MARKET-05](../core-market/hazard-quotes-and-risk.md)
   and [06](../core-market/calibration-provenance.md) for
   exact original observation identity, qualified zero-shock
   reproduction, derived par-risk inputs, recipe terms and
   fit acceptance. Freeze each recipe's discount dependency,
   documentation clause, conventions and recovery for spread
   shocks. An exact positive finite common step `h` in bp
   applies to every declared quote for the parallel measure
   or to one exact quote for a bucket. Report
   `(V_+−V_-)/(2h)` in holder currency per bp.
   Keep source-qualified parallel results and source-and-quote
   bucket keys, and identify the selected source set for a
   joint parallel result. Rebuild a shared source once per
   scenario only for complete compatible bindings. Update
   every affected active reference while preserving distinct
   issuer exposures and events. Reject missing replay,
   conflicting bindings and unresolved moves. Direct intensity
   changes have their own hazard-risk basis. The tranche
   experiment selects par-quote CS01. A finite sum of buckets
   is a separately labelled result.
   [White, §8](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf)
   and [Andersen, Sidenius and Basu](https://www.risk.net/derivatives/credit-derivatives/1530334/all-your-hedges-one-basket).

4. **Select the rate basis and credit response independently.**
   Parallel and bucketed DV01 each require typed rate-market
   quote rebuild or direct continuous-zero shock under
   [CORE-MARKET-03](../core-market/rate-shocks.md).
   Use exact rate recipe targets or the accepted parallel
   and triangular direct geometry, on the actual rate
   source clock. Also select frozen active intensities or
   replay all affected active hazards using unchanged
   original credit observations with their own declared
   discount dependencies. Freeze unrelated sources and
   retain quote provenance without claiming a fit on a
   changed rate market when hazards are frozen. For both
   parallel and buckets use the central CREDIT-13 positive-up
   rate stencil in holder currency per bp and retain the
   selected basis, targets and represented actions. A fixed
   future cash claim can retain rate risk after protection
   ends. Same-day included cash uses factor one. An exhausted
   layer can omit inactive credit replay after request validation.

5. **Separate direct base-correlation risk from market quote risk.**
   `correlation01` moves the selected base-correlation source
   additively under [CORE-MARKET-18](../core-market/base-correlation.md).
   Select all its knots for parallel risk or an exact stored
   detachment-percent knot for a bucket. Require finite positive
   decimal step `h`, and report currency per +.01 absolute
   correlation. The step applies to stored source levels.
   Interpolation transmits those moves to the active original
   strike queries. Use the same source identity with atomic
   scenario rebinding and preserve the chosen copula's
   interpretation of its correlation parameter. Hold other
   model and market inputs fixed. Use the whole-vector
   boundary rule below with closed domain `[0,1]`, including
   the checked endpoint laws in CREDIT-17. Source changes can
   fail the tranche consistency gates. Tranche quote shock
   and rebootstrap would be a separate calibration experiment.

6. **Reprice future recovery risk on the actual cash exposures.**
   Select common homogeneous recovery, one exact live named
   member, or all active named members. Select frozen
   intensity or quote-preserving replay. Frozen intensity
   changes only selected future cash recoveries and leaves
   source intensities and their calibration metadata fixed.
   In the quote-preserving experiment require matched base
   cash and calibration recovery on each affected binding,
   move them together and replay unchanged original credit
   observations. Every active user of an affected shared
   source must belong to the selected experiment and agree
   on the calibration recovery and complete recipe. Refuse
   a source spillover that would change a member outside the
   request. Keep the original historical loss, recovered
   principal and confirmed cash fixed. Reprice full protection
   and premium since future recovery changes tranche caps
   and outstanding balances. Use an explicit positive decimal
   step and the whole-vector rule below, reporting holder
   currency per +.01 absolute recovery. The cash domain is
   `[0,1]` under CORE-MARKET-19. Quote-preserving replay also
   requires its actual calibration domain, which excludes
   unit recovery for CREDIT-12 CDS recipes. Metadata
   availability alone does not select the experiment.

7. **Choose stencils before pricing and propagate their allowances.**
   Use central `.01(V_+−V_-)/(2h)` for correlation and
   recovery when both whole vectors fit their declared
   domains. If exactly one full direction crosses a parameter
   bound, use the other direction and its qualified center:
   `.01(V_+−V_0)/h` or `.01(V_0−V_-)/h`, both oriented
   positive-up. Refuse when both directions cross bounds.
   Retain nominal common step and represented moves for all
   stencils. Reject a lost required move, invalid target,
   unavailable source, failed replay or inconsistent price.
   Those failures never choose a different stencil or smaller
   bump. Preserve CREDIT-17/18's asymmetric allowances under
   signed stencil weights. For central difference, with
   endpoint intervals `[V_i−e_i^lo,V_i+e_i^hi]`, the
   lower allowance uses `e_+^lo+e_-^hi`, and the upper
   uses `e_+^hi+e_-^lo`, scaled by the same denominator
   and reporting unit as the result. Include checked
   arithmetic and disclose a direction unresolved against
   its allowance. When one leg is provably identical, it
   may cancel in checked raw differences. A structural zero
   requires established independence and validated inputs.
   A small or uncertain estimate alone cannot supply that
   status. Operational allowances and finite stencils do
   not establish derivative convergence at kinks or bounds.

8. **Separate gross protection from economic jump-to-default.**
   For a named pool target an exact eligible member, or
   request a keyed series of separate single-name experiments.
   Supply scenario recovery `R*` in `[0,1]` for each target.
   On `d=v+1` force that member's default, with loss
   `x=w_i(1−R*)` and recovered principal `y=w_i R*`
   in original-pool units. Start from accepted seasoned
   geometry. The isolated gross payout to the buyer is
   original tranche notional times the width of the
   intersection of `[L0,L0+x]` with the surviving tranche,
   divided by original tranche width. Top erosion from
   `y` reduces outstanding and is a principal effect.
   Apply CREDIT-18's close-of-day recognition and selected
   settlement date, and retain opening outstanding for
   accrual on `d`. Reprice the whole stressed tranche at
   the original valuation close. Keep the other marginals
   anchored at that close, keep their identities and
   hazards even if shared, and retain the selected copula
   and original-strike correlation values. This is an
   isolated default stress without contagion, a latent
   posterior update or a date roll. Count the forced loss
   only once. If showing a separate event row, allocate it
   first within the day and value remaining protection on
   the reduced capacity, preserving the full stressed
   projection. The cap identity
   `min(x+X,k)=min(x,k)+min(X,max(k−x,0))` also applies
   to recovered principal. Report signed gross protection
   cash and signed economic `jump_to_default=V_stress−V_base`
   separately in native currency. Zero gross can still
   change the mark through changed subordination. Exclude
   unsupported pre-coverage events and give validated
   zero new exposure for an exhausted layer, expired
   protection, known exited member or zero-weight member.
   Unknown identities error. A homogeneous pool can
   explicitly request an equal-weight gross cash proxy
   using `1/N_names` of original pool and the authoritative
   live count. Named economic JTD requires a roster. Retire
   the existing average gross payout under the JTD label.
   [Frey and Backhaus, §§1 and 4](https://statmath.wu.ac.at/~frey/publications/frey-backhaus-hedging-CDOs.pdf).

9. **Preserve experiment identity through the existing consumers.**
   One checked Rust owner supplies prices, source and issuer
   bindings, cash and quote bases, signs, units, stencils,
   moves, allowances and errors. Use exact key encoding for
   quote and detachment buckets and avoid aggregating distinct
   experiments under one undifferentiated metric ID.
   Attribution matches the actual replay and move basis.
   Portfolio aggregation combines only compatible additive
   results and preserves missing risk status. Par remains a
   per-position quote. Margin classification, factor mapping
   and aggregation belong to the margin owner. A parallel
   tranche CS01, coupon PV01 or named JTD cannot supply
   unimplemented margin-factor coverage. Calibration owns
   fit objectives and solver policy and uses the same raw
   selected valuation, retaining scenario errors.
   Use the resolved active bindings for attribution and risk.
   Static dependency declarations can remain conservative for
   invalidation. Reconcile the risk registry, native methods,
   prepared paths, reports, scenarios, Python/WASM, schemas, declarations and
   parity. Hosts only convert Rust inputs and results.

## Observed behavior

### Choice and rationale

Tranche risk mixes analytic unit-coupon calculations, quote-space rebootstrap and direct payoff loss. The selected pricing model is not propagated uniformly to all specialized metrics.

### Scope and input domain

Live CDS tranches and CreditIndexData, with optional issuer-specific curves. Quote CS01 requires replayable hazard calibration and a provider. Recovery/rate calculations conditionally replay available calibration.

### Algorithm

1. Spread DV01 prices premium rows at one basis point. Par spread divides minus constant protection/upfront PV by premium PV per bp, clips to [0,100000] bp and requires residual below 1e−6 × notional; negligible annuity returns zero.
2. CS01 gathers distinct active issuer hazard IDs if issuer curves exist, otherwise the index hazard. It rebuilds all quote bumps together for total risk and individually for components, replacing hazard references inside CreditIndexData. Bucketed CS01 delegates quote-node series and reinserts each rebuilt index.
3. DV01 bumps discount curves and replays eligible active hazards before dispatch repricing. Correlation01 shifts all base-correlation knots ± the configured correlation bump, clips knots to [0,1], and returns the central PV difference scaled to that bump.
4. Recovery01 bumps index-level recovery within bounds, reboots only the index hazard if it has calibration metadata, otherwise freezes hazard; scales the PV span to the standard recovery bump.
5. Jump-to-default averages the marginal loss increase from one constituent default, using issuer weights/recoveries or equal names. It returns a positive loss amount without subtracting current PV or applying position sign.

### Selection and controls

Quote/rate risk uses resolved sensitivity configuration and dispatch repricing. Spread, par, EL, correlation and JTD metric calculators instantiate `CdsTranchePricer::new()` (Gaussian). Recovery01 calls instrument.value, also defaulting to its base pricer. Bucketed DV01 uses the framework’s triangular key-rate calculator.

### Conventions

CS01/DV01 are PV change per basis point; correlation risk uses an absolute correlation bump; recovery risk uses absolute recovery fraction. Spread DV01 is coupon-annuity sensitivity, distinct from quote CS01.

### Limitations, failure and fallback

Heterogeneous Recovery01 updates neither issuer-specific recovery overrides nor issuer hazards, so its economic scope differs from active-hazard CS01/DV01. Boundary-clipped correlation bumps retain the central denominator. Custom-copula valuation can therefore be paired with Gaussian specialized risk outputs.

### Alternatives and tradeoffs

Consistent selected-pricer dispatch and per-name recovery replay are possible alternatives; frozen-hazard recovery and market-quote-preserving recovery are different risk definitions and should be named deliberately.

## Evidence

Owners: [sensitivities.rs](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/sensitivities.rs), [metric implementations](../../../../finstack-quant/valuations/credit/src/cds_tranche/metrics). [Tranche tests](../../../../finstack-quant/valuations/credit/tests/cds_tranche) locate finite-difference/repricing and sign evidence; these do not establish independent risk accuracy.

The [tranche instrument](../../../../finstack-quant/valuations/credit/src/cds_tranche/types.rs)
and [shared instrument pricing trait](../../../../finstack-quant/valuations/core/src/instruments/common_impl/traits/instrument.rs)
locate the Money boundary behind the existing generic raw pricing call.
[Credit attribution](../../../../finstack-quant/attribution/src/metrics_based/credit.rs)
locates the use of matched quote movements and the aggregate spread
approximation that requires basis qualification.

### Independent review evidence

- With original tranche notional `1,000,000`, two equally
  weighted names, `[0%,25%]`, Gaussian dependence, marginal
  PDs `.5`, recovery `.4`, zero coupon and unit discount, the
  buyer PV is `1,000,000[.75−asin(ρ)/(2π)]`. Each default
  alone exhausts the layer. A `.02` central correlation
  step at `ρ=.50` gives `−1,838.09` per point. At
  `ρ=.99`, the full backward step gives `−8,278.13`.
  Clipping the up side to one and using the nominal central
  denominator instead gives `−9,770.74`. With selected
  source levels `.01,.99`, neither whole direction admits
  a `.02` step.
- In a one-day independent `[25%,55%]` tranche with two
  equal-weight names, original tranche notional `1,000,000`,
  PDs `.10,.25`, recovery `.4` for both, zero coupon and
  unit discount, base buyer PV is `75,000`. Bumping the
  first name's cash recovery by ±.01 at frozen intensity
  gives Recovery01 `−1,250`. A stylized one-period replay
  keeping the first name's observation `(1−R)p=.06` gives
  about `−694.64` per point. The latter isolates the replay
  basis in arithmetic and does not use a production CDS recipe.
- For an independent three-name equity example with equal
  weights, recovery `.4`, PDs `.1,.2,.3`, layer
  `[0%,10%]`, original tranche notional `1,000,000`, zero
  coupon and unit discount, use stylized par-risk inputs
  `q_i=.6p_i`. A 100bp step gives parallel central CS01
  about `318.37963` versus `318.33333` for the sum of
  separately repriced quote buckets. Separately, a confirmed
  receipt of `50,000` due in 30 ACT/365F days at 5% continuous
  discount gives `−.409273497` currency/bp of direct DV01
  with a ±1bp step and no residual credit exposure.
- For a seasoned `[10%,20%]` tranche of `1,000,000`, take
  historical loss `.05`, no historical recovery, and
  positive-weight live names `.06,.08` with cash recovery
  `.5`. Remaining live weight `.81` has zero PD. Use
  independence, two remaining ACT/360 days, coupon `.036`,
  no elapsed accrual, protection factors `.99,.98` and
  coupon factor `.97`. Base cumulative PDs for the two
  names are `(.10,.20)` and `(.25,.30)`. Force the first
  name on day one at recovery `.5`. The isolated payout
  is zero since loss rises only to `.08`, below attachment.
  The checked scenario loss increments and opening balances
  give base buyer PV `14,546.388` and stressed buyer PV
  `59,009.88`, for economic JTD `+44,463.492`. The seller
  reverses that sign.
- Central pricing endpoints `10.08` and `9.92` with
  allowances `[−.01,+.03]` and `[−.02,+.01]` at a 2bp
  step give `.04` currency/bp with interval
  `[.035,.0525]`. Dated market replay, selected-model
  consistency under shocks, bump convergence, shared bindings,
  regulatory factor coverage and host parity require independent
  production qualification.

## Relationship to the accepted base-correlation boundary

[CORE-MARKET-18's complete accepted package](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks)
requires unclipped admitted source shocks and truthful actual action widths.
Correlation01 reports the PV effect per 0.01 absolute correlation, separately
from detachment/spread bp. The risk owner selects a supported central/reduced/
one-sided boundary scheme or refuses the request, without clipped nodes and a
nominal central denominator. Base/bumped valuation must use the same qualified
financial evaluator; default Gaussian metric substitution is not custom-model
risk parity. Record 18 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's complete
correlation/quote/recovery/default-risk methods or accuracy budgets.

## Relationship to the accepted credit-pool boundary

[CORE-MARKET-19's complete accepted package](../core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts)
requires actual active dependency selection. Under the accepted daily
timing model, named loss hazards supply the timing through their loss
increments. Shared CurveIds can be shocked/replayed
once while preserving separate issuer events and updating every active reference.
Zero-weight roster entries do not imply active exposure for every metric. Future
cash-recovery risk must target actual pool/member cash inputs and preserve the
separate historical-state assumption, rather than bump an unused common field
or rewrite observed history. Record 19 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this
record's named default, quote replay, recovery, rate or credit-risk
experiments, accepted separately above.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
