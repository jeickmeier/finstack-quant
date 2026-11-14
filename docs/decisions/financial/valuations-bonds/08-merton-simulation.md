# VALUATIONS-BONDS-08: Structural GBM default simulation, PIK elections and loss outputs

**Decision status:** Accepted — complete checked structural bond simulation and outputs package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, stochastic convergence, market and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked structural bond simulation and outputs

1. **Elect the exact structural model and separate discount basis.**
   Require explicit MertonMc and a complete compatible validated
   model on ACT/365F. Admit terminal GBM at a flat terminal
   barrier and first-passage GBM at `B(t)=B_0*exp(g*t)`, with a
   separate monitoring election for first passage. Require positive
   finite firm assets, same-basis barrier and annual asset volatility,
   finite continuous model rate `r_m`, payout `q`, and barrier growth
   where applicable. Use the risk-neutral structural log drift
   `r_m−q−σ²/2` with the selected constant parameters and checked
   exact lognormal updates on actual grid widths. Choose an explicit
   qualified default-free dated discount source, or an expressly
   selected continuously compounded flat discount basis. Dated
   sources use native relative factors with full clock context
   and the accepted native-coordinate map at fractional model knots.
   Use that one basis for holder cash, recovery and benchmark PV.
   Distinct model-rate and discount bases define a constant-rate
   dynamics and dated-discount approximation. Disclose both and
   derive neither from the other. Remove empty curve fallback,
   endpoint-zero extrapolation and inconsistent direct/registry
   basis. This is an exogenous bond recovery payoff on structural
   assets. Endogenous terminal debt payoff, jumps, CreditGrades,
   empirically mapped default rates and generic OAS belong to
   other explicitly adopted laws.
   [Bond model dispatch](03-pv-models.md),
   [structural probability laws](../models-and-valuations-credit/04-structural-default.md),
   [checked GBM transition](../models-monte-carlo/04-lognormal-diffusions.md)
   and [Merton's original terminal debt model](https://onlinelibrary.wiley.com/doi/full/10.1111/j.1540-6261.1974.tb03058.x).

2. **Admit one exact contractual bullet coupon program.**
   Use the authored generated couponless, fixed or step-up
   program with exact original parents, stub accruals and rate
   changes, payment entitlements, contractual election phases
   and terminal dates. Require covered pre-default valuation state,
   including seasoned current accreted par, relevant parent-start
   balance, frozen historical elections and paid or pending cash.
   Initial face, current accreted principal, firm assets and
   barrier are distinct. Admit cash, PIK, exact cash-fraction
   split and authorized whole-parent toggle. Select the contractual
   program or one explicit authorized model election schedule
   under the shared stable origin and clock. Uniform Cash elects
   cash for every applicable parent. A stepped schedule has an initial
   mode at zero and strict ordered knots. Resolve legally
   distinct rates or rounding for cash and PIK through their
   own complete parent terms when admitted. Exclude floating
   coupons, amortization or later draws, embedded calls, puts
   and return floors, and supplied custom schedules in this
   route. Remove the scalar initial-rate approximation and
   projection of a custom placeholder into a different coupon law.
   [Bond schedule and entitlement](01-schedule-and-holder-cashflows.md),
   [shared PIK controls](../valuations-framework/shared-contract-controls.md)
   and [filed toggle terms with distinct rates](https://www.sec.gov/Archives/edgar/data/1550043/000119312512238523/d350029dex101.htm).

3. **Use exact event nodes and distinguish default from cash coverage.**
   Bind an explicitly qualified last default-test date separately
   from later cash coverage. For a live numerical horizon require
   an explicit positive seed step count and checked resource
   controls. Prepare the exact union of a uniform ACT/365F grid
   to the default horizon and all applicable dated coupon,
   election, economic, default and cash phases within it.
   The resulting intervals use their own actual positive widths.
   Node-only first passage monitors every node of this declared
   profile, including origin and the last test. The actual grid
   determines this discrete law, including inserted dates.
   Admit later cash conditional only on survival at the last
   authorized test when the terms resolve that payment and its
   entitlement then. A claim exposed to new default after the
   last test requires separately qualified coverage. A vested,
   default-independent claim retains its own PV and cash date.
   Mere later cash never silently extends default monitoring.
   Remove half-step coupon placement, inferred steps per year,
   rounded event matching and invented calendar cash dates.
   A structurally resolved origin payoff or extinguished state
   needs no synthetic forward interval. Keep terminal origin
   with zero elapsed horizon distinct from a positive terminal
   horizon. Preserve separate cash and model source coverage.
   [Bond clocks and phases](02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks),
   [checked time grids](../core-numerics/time-grids.md)
   and [Broadie, Glasserman and Kou on discrete monitoring](https://www.columbia.edu/~mnb2/broadie/Assets/bgk_mf.pdf).

4. **Apply inclusive structural default and an explicit recovery
   timing convention.** Terminal default tests only `V(T)≤B`
   at the authorized terminal node, including a qualified zero-time
   test. First passage defaults at origin or a monitored endpoint
   on touch or breach. For bridge monitoring on an interval of
   width `h`, with strictly positive log distances `x_l,x_r`,
   use checked log crossing probability
   `−2*x_l*x_r/(σ²*h)`. Cross when the log of the selected
   interior uniform is strictly below it. Allow scaled
   comparisons when intermediate products would lose range.
   For an endpoint touch or breach, crossing is certain.
   Under ideal GBM and the affine log barrier the bridge gives
   conditional crossing incidence. The detection and recovery
   convention posts at the right endpoint, so the simulated
   detecting time is generally later than the actual first hit.
   Use no sampled hit-time claim. A numeric substep can be a
   detecting knot with a model discount coordinate and no
   invented contractual date. Test default before that node's
   default-contingent coupon, capitalization and repayment.
   Preserve qualified prior or default-independent cash and
   stop the extinguished stream. Select one explicit constant
   recovery or admitted dilution rule on the covered pre-event
   accreted recoverable par. Pay `R(N)*N` once, with no accrued
   or premium. Validate recovery and stock before zero identities.
   This modeled default event does not establish observed
   issuer default. Report monitoring, detecting and settlement
   conventions and qualify incidence and recovery-timing convergence
   separately.
   [Accepted structural touch law](../models-and-valuations-credit/04-structural-default.md),
   [recovery dilution rules](../models-and-valuations-credit/06-recovery-and-lgd.md),
   [Black and Cox on first passage and recovery payoff](https://onlinelibrary.wiley.com/doi/full/10.1111/j.1540-6261.1976.tb01891.x)
   and [Glasserman and Staum on conditional barrier crossings](https://business.columbia.edu/sites/default/files-efs/pubfiles/4311/one-step_survival.pdf).

5. **Replay coupon, election, capitalization and cash once.**
   Use captured exact Money principal and the original parent
   coupon's contractual amount and allocation authority. Freeze
   an authorized election at its actual information phase, with
   no future path observation. Capitalize generated PIK at its
   qualified economic accrual end, adding the exact nonnegative
   principal effect once. Remit eligible signed cash at its
   authorized payment phase once, applying the holder's exact
   cash entitlement. Cash detachment leaves an independently
   authorized principal effect in force. Preserve paid or
   vested claims and reject unresolved dependencies or posting
   order. Select early payment or an earlier election only when
   its required amounts are available at that phase. Determine
   terminal par from the covered accreted balance, preserve
   the actual repayment date, and redeem it once on survival.
   Do not double-pay pending PIK or capitalize it on a defaulting
   branch. Cash and PIK elections do not debit simulated assets
   directly or change the exogenous outer barrier; they update
   bond cash and accreted par under this scope.
   [Coupon cash and PIK](../cashflows/05-coupon-cash-and-pik.md)
   and [dated economic event order](../cashflows/03-principal-and-emission-order.md).

6. **Require an explicit qualified toggle policy and feature basis.**
   An active modeled election uses the accepted threshold,
   logistic or nested EquityComparison rule with the legal
   alternatives, actual election phase and checked complete
   inputs. A known contractual or historical election stays
   fixed. For a model feature, admit explicitly qualified
   accreted debt/asset leverage, a selected leverage-intensity
   map, or a positive-horizon terminal GBM distance on
   the selected state, barrier and risk-neutral clock. Label
   instrument leverage as a proxy where it is only part
   of issuer debt. Remove automatic origin-PD equivalent hazard
   and fabricated distance. Any future equivalent-intensity
   feature needs its own declared conditional basis and
   admission. Hazard-map output affects the toggle only.
   Use strict threshold and stable logistic comparisons under
   the accepted election law. Nested EquityComparison uses
   independently qualified action amounts and explicit checked
   per-election seed/stream assignment and positive horizon,
   counts and work budget. Its hypothetical actions do not
   change the outer structural model. It is a single-election
   rule rather than a full bond policy optimization. Missing
   selected model, state, action or admissible work is an
   error. The contractual cash election and model output
   retain their own provenance.
   [Accepted leverage and toggle elections](../models-and-valuations-credit/05-hazard-and-toggle.md)
   and [qualified distance definition](../models-and-valuations-credit/04-structural-default.md).

7. **Select one checked fixed-budget sampling profile.** Supply
   explicit independent group count in `2..=10,000,000`
   subject to tighter checked resource bounds, antithetic and
   serial/native parallel choice, and exact u64 root seed or
   an expressly selected accepted instrument-ID and scenario-label
   derivation. Use CORE-NUMERICS-17 PCG64 with the root seed and
   the zero-based group ID as stream. Fix group-local draw order
   before running: exact chronological interval normals, then
   chronological bridge uniforms if bridge monitoring is active,
   then logistic uniforms in ordered legal parent/election phase
   order for the applicable modeled rule. Reserve those slots
   independent of realized default, breach or action. In bridge
   and logistic slots, use the top 52 bits of the core uniform
   word's 53-bit code and map rank `b` to `(b+1/2)*2^-52`.
   Reflect that rank for the antithetic leg and negate its
   normal innovations. This profile gives exact interior
   reflected uniforms on the finite grid. The finite RNG grid
   limits rare-event resolution and implies no variance
   improvement guarantee. Nested action comparisons use their
   own identity-bound seed and per-election streams under the
   accepted lower-level law, with paired action draws independent
   of outer draw consumption. Preflight grid, group, path,
   nested work and simultaneous storage, including optional
   path diagnostics. Fail on invalid draw, sizing or numerical
   result with stable group/leg/event context and no partial
   result. Remove embedded budgets and pairing defaults.
   Report actual profile, grid, exact seed and all group and
   trajectory counts. Preserve addressing in qualified paired
   calibration/risk experiments on the same simulation geometry.
   [Checked PCG](../core-numerics/pseudorandom-and-bridge.md),
   [explicit simulation controls](../valuations-framework/model-resolution.md)
   and [fixed-budget estimation](../models-monte-carlo/01-engine-and-estimation.md).

8. **Publish model PV, price uncertainty and conditional path
   statistics on their own bases.** Use one checked time-zero
   currency PV per leg and the equal-weight mean of each
   complete antithetic pair as one estimator. Report mean PV,
   sample SE and 95% Student-t interval on independent group
   observations, with any explicit native-currency half-width
   gate applied after the fixed budget. A structurally exact
   result reports its reason and has no sample uncertainty.
   A sampled constant outcome retains a computed point interval.
   Report physical-leg risk-neutral modeled default frequency,
   average modeled detection time and recovery rate conditional
   on defaults, and mean terminal accreted par conditional on
   survivors. Report each contributing count and use unavailable
   for an empty conditional subset. Report one positive PIK
   incidence statistic, positive capitalizations divided by
   eligible coupon-parent events settled through their admitted
   economic phase across legs, with its contributing count.
   That statistic measures event incidence, including fixed splits.
   Keep per-100 model summaries on an explicitly stated
   positive face basis. A settlement dirty or clean quote
   requires the qualified buyer-value and accrued bridge of
   record 02. Preserve valid raw holder PV when no quote view
   qualifies. No generic valuation-date accrued subtraction.
   Report uncertainty as fixed-model sampling uncertainty
   excluding model, grid and calibration error.
   [Bond clean/dirty basis](02-settlement-and-clocks.md)
   and [fixed-sample interval](https://www.itl.nist.gov/div898/handbook/eda/section3/eda352.htm).

9. **Define benchmark shortfall and empirical loss tail explicitly.**
   Elected loss diagnostics use a positive no-default reference
   PV `P_0` under the same holder cut, dated discount and
   contractual fixed cash/PIK program. For modeled Toggle, require
   an explicitly selected deterministic reference election schedule
   admissible under the terms. Cash can be selected where
   authorized. Zero modeled hazard supplies no implicit election.
   Use the benchmark's literal payment identities and cash.
   Report signed relative PV shortfall `1−mean(PV)/P_0`.
   Rename the old unexpected loss to physical-path PV
   sample dispersion normalized by explicit positive initial
   face `F`. Use sample standard deviation with denominator
   `L−1` on the `L` leg PVs when at least two are present.
   This is a descriptive path statistic. For 95% expected
   shortfall, set each signed loss to `(P_0−PV_leg)/F` and
   average the largest exact 5% of the empirical leg mass.
   With ordered losses take `floor(L/20)` full largest
   observations plus the exact leftover fraction of the next
   and divide by `L/20`. Give equal mass to each leg.
   Preserve signed losses, ties, true zeros and fractional
   tail mass, with checked scaled accumulation. A tail price
   must use a distinct price label if separately displayed.
   Pricing estimator SE and path dispersion are separate.
   Missing qualified benchmark leaves optional benchmark
   diagnostics unavailable and fails an explicitly required
   such calculation.
   [Accepted fractional empirical tail](../analytics/07-historical-tail-risk.md)
   and [Acerbi and Tasche on expected shortfall](https://arxiv.org/abs/cond-mat/0104295).

10. **Solve a signed effective-spread experiment on the frozen strip.**
    If requested, fit a signed annual continuous `s` to
    `Σ_i C_i D(0,T_i) exp(−s τ_i)=mean(PV)` for the
    explicit no-default benchmark, with ACT/365F from the
    model origin. Require nonnegative qualified weights and
    at least one positive later cash amount. The target must
    strictly exceed any fixed origin cash. Use an explicit
    finite ordered bracket and a checked fallible Brent solver
    with declared controls. Admit one exact endpoint or a
    sign-changing bracket. Verify the returned spread, including
    conversion for a requested bp view, against a positive
    explicit native-currency price residual tolerance. A
    price above the unshifted benchmark can justify a negative
    spread. Equality gives zero subject to admitted controls
    and domain. Nonrepresentability, unsupported benchmark,
    absent bracket or failed solve returns the actual reason.
    Remove zero and unconverged-last-iterate fallbacks. The
    fit prices only the frozen comparison cash and leaves the
    structural parameters, recovery and selected bond model
    outside this spread adjustment. Records 09, 12 and 17 own
    structural scenario calibration, OAS and hazard-based
    default metrics, respectively.
    [Checked Brent](../core-numerics/brent-root.md),
    [structural calibration](09-merton-calibration.md),
    [spread ownership](12-spread-quotes.md)
    and [default risk](17-default-risk.md).

11. **Reconcile the actual pricing, host and reporting paths.**
    Apply one prepared dated holder valuation and checked
    model result through direct, native and registered pricing,
    with raw Money for price consumers and consistent price
    normalization only at output. Use exact paid and modeled
    event identities for reporting. Export scheduled cash or
    reconciled selected-model output with its appropriate
    provenance. Refuse unqualified expected cash and contingent
    settlement views. Keep derived grids and private path state
    Rust-owned. Reconcile exposed model terms, results, optional
    diagnostics, exact seeds, schema and atomic validated
    restoration across existing Python and WASM routes. Reject
    conflicting active inputs, unsupported model or calibration
    choices, invalid source or arithmetic and unsupported host
    execution. Qualify structural probability separately from
    coupon/PIK economics, recovery timing, settlement, risk,
    sampling and host parity.
    [Pricing lifecycle](../valuations-framework/pricing-lifecycle.md)
    and [cashflow export](../valuations-framework/cashflow-export.md).

## Observed behavior

### Choice and supported domain

`MertonMc` is explicit and requires `merton_mc_config`. Bond adaptation rejects floating and amortizing specs and all call/put/return-floor rights. Fixed coupons are supported; StepUp is accepted but every coupon uses its initial rate, an explicit approximation. Custom schedule amounts are not replayed: the adapter extracts dates/accrual factors and scalar coupon/notional, so a custom fixed placeholder can lose its actual coupon economics. This is not general custom-schedule valuation.

### Algorithm and controls

Use `ceil(T·steps_per_year)`, at least one step, with `dt=T/n`; embedded defaults are 5,000 independent estimators, antithetic on, 12 steps/year. PCG64 streams are indexed by seed and estimator. Per path draw all Gaussian steps, bridge uniforms and coupon-toggle uniforms. Antithetic legs negate Gaussians and complement bridge uniforms; toggle uniforms are reused.

Evolve `V_next=V exp[(r_model−payout−σ²/2)dt+σ√dt Z]`. The drift uses `config.merton.risk_free_rate`, not the separate `discount_rate` argument (some surrounding docstrings say otherwise). Terminal default occurs only at final step if `V<B`. First-passage uses growing barrier `B exp(g t)`; endpoint crossing defaults, or Brownian-bridge probability `exp(−2 log(V_prev/B_prev) log(V/B)/(σ²dt))` when both log distances are positive. Constructor default is bridge for first-passage and discrete for terminal. Recovery is paid at the detecting step end on then-current notional; dynamic recovery overrides flat recovery. Default is checked before coupon processing, so intrastep coupon/default ordering is an approximation.

Process coupons when step time reaches coupon time minus half a step; discount at the exact coupon time. Coupon is current notional times scalar coupon rate times contractual accrual. Cash pays it, PIK capitalizes it, Split divides it, and Toggle delegates a credit-state decision; absent toggle model falls back to cash. State contains leverage, coupon due, asset value, distance to default and either endogenous leverage hazard or equivalent Merton PD hazard. Endogenous hazard affects this toggle state only; structural barrier default is unchanged. PIK changes recovery/redemption notional but does not itself change the configured debt barrier. Default Uniform(Cash) config is replaced by the bond's cash/PIK type; a nondefault config schedule wins.

Survivors repay accreted principal. DF inputs use log-linear interpolation with endpoint-flat-zero extrapolation; absent/empty knots use `exp(−discount_rate·t)`. Registry pricing derives the fallback discount rate from the curve/horizon and populates dated curve DF knots, without changing structural-model drift.

### Outputs, fallbacks and limitations

Dirty price is mean PV/initial face×100; clean subtracts contractual accrued. Expected loss is `1−meanPV/risk_free_PV` (zero if denominator nonpositive); baseline follows PIK schedule but treats Toggle as Cash. Unexpected loss is physical-path PV sample standard deviation/face. `expected_shortfall_95` is mean of the lowest `ceil(5%·physical_paths)` PVs as percent of face, not a tail loss. Average PIK fraction counts coupon periods with any PIK, not capitalized amount fraction. Terminal notional averages survivors; when none survive it returns initial face. Default time/recovery average defaulters, otherwise zero. Standard error uses antithetic pair means and is in price percentage points.

Effective spread Newton-solves a nonnegative continuous spread on the risk-free baseline, finite-difference derivative at 1 bp, 50 iterations and residual `1e−10·face`. Nonpositive targets, nonpositive maturity or price above baseline return zero; a tiny derivative or iteration exhaustion returns the last iterate without convergence error.

## Evidence and qualification

Owning code: [engine](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/engine.rs), [bond adapter](../../../../finstack-quant/valuations/fixed-income/src/bond/types/traits.rs), [registry adapter](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/pricer.rs), [sampling/result types](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/types.rs), [config/defaults](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/merton_mc.rs), [default data](../../../../finstack-quant/valuations/core/data/defaults/product_mc_defaults.v1.json). [Tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/tests.rs) inspect bridge/terminal branches, seeded determinism, pair-mean stderr and toggle fallback; `endogenous_hazard_lowers_pik_price` only asserts nonincrease and permits equality, so does not prove hazard feedback. References: [Merton 1974](../../../REFERENCES.md#merton-1974), [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo). No independent structural-credit accuracy qualification was executed.

### Independent arithmetic examples

- Endpoint log distances .10 and .20, volatility .40 and
  interval .50 give bridge crossing probability `exp(−.5)`.
  An interior draw .25 triggers crossing and .75 survives.
  Node-only monitoring survives on these endpoints. Paying
  40 at time 1 under flat 5% gives PV 38.04917698,
  versus 38.52777671 at hypothetical actual hit time .75.
- With assets and barrier both 100, zero payout, volatility
  .20 and model rate .02, the terminal log drift is zero.
  The ideal one-year terminal default probability is .50.
  A par-only payout of 100 and default recovery 40 with
  dated discount .95 give model PV 66.50. First passage at
  the same initial touch defaults at origin.
- Issue face 100 earns 1 PIK from 2% over half a year.
  The next parent earns 5.05 at the authored stepped 10%
  on principal 101. A cash share .4 pays 2.02 and capitalizes
  3.03, so surviving principal is 104.03. Default before
  that second posting sees 101 par. Constant recovery .40
  pays 40.40, while inverse dilution from .40 on reference
  par 100 pays 40.00.
- Four pricing legs `(60,100),(80,100)` yield group means
  80 and 90, mean PV 85 and pricing SE 5. The sample
  physical-leg dispersion is about 19.14854 in currency.
- Reference PV and face 100 with 21 path PVs `20,60,100×19`
  give relative PV shortfall `2/35`. The largest 5% loss
  mass takes the loss .80 and 0.05 of loss .40, giving
  expected shortfall `82/105`. The old ceil-two rule would
  average a different tail mass. For a one-year benchmark
  of 100 with unit base discount, target 110 implies signed
  spread `log(100/110)`, about −9.5310% annual.

Qualify terminal and bridge incidence against independently
derived structural probabilities, recovery-grid timing,
fixed and step-up no-default schedules, eligibility and
seasoned/lagged PIK, toggle phase and replay, signed or
unavailable output domains, paired estimator and tail samples,
pricing/calibration bases, and actual Rust and host routes.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
