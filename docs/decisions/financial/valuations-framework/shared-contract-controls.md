# VALUATIONS-FRAMEWORK-19 — Shared exercise, conversion, PIK and stochastic-result controls

- Decision status: **Accepted — complete checked shared contract and stochastic controls package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current shared types and selectors.
- Validation status: Source, selected references and independent arithmetic reviewed. Runtime, model and host qualification pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked shared contract and stochastic controls

1. **Require checked terms and one sign per basis.** The same Rust
   validation governs construction, checked changes, pricing admission and
   restoration. Preserve the canonical wire names and sign mappings
   `receive = +1`, `pay = −1`, `long = +1`, `short = −1`. Require an
   explicit side where it affects cash or value and apply its sign once on
   the stated primary leg or position basis. Remove the implicit long
   default. Preserve fixed coupons as Decimal rates and floating spreads and
   CDS premiums in their declared Decimal basis-point units. Preserve zero
   and signed amounts subject to the product's domain.

2. **Resolve actual leg schedules and market roles.** Supply terms directly
   or through a deliberately selected, qualified template. Require valid
   accrual bounds, explicit grid, frequency, stub and roll, payment and
   fixing calendars where used, day-count context, adjustment and lag.
   Preserve nominal dates, actual accrual dates and payment and fixing dates
   under the accepted operation order. Remove weekends-only substitution,
   inferred end of month and negative lag sentinels. Keep zero additional
   payment lag as identity and distinguish an absent reset from a zero-lag
   reset. Reject unsupported combinations such as SimpleAverage on the
   existing swap route. Preserve the accepted TRS financing method and CDS
   premium/protection date regimes, settlement and provenance. See
   [construction authority](convention-selection.md),
   [schedule grids](../core-conventions/008-schedule-generation.md),
   [cashflow periods](../cashflows/01-schedule-periods.md),
   [TRS legs](trs-legs.md) and [CDS legs](../models-and-valuations-credit/12-cds-legs.md).

3. **Validate issuer-call terms and exact lockout.** Require a nonempty,
   strictly increasing supplied date list and a finite positive call price in
   percent of par, where 100 means par. A lockout through date `L` admits only
   dates strictly after `L`. With no lockout the shared filter admits the
   supplied dates. A valid schedule can have zero eligible dates. Reject
   invalid lists and never sort, deduplicate or snap them. The product
   validates exercise versus notice, valuation cut, lifecycle and horizon,
   determines co-dated event order and payable amounts, and reports
   unsupported callable routes. See
   [callable range accrual](../valuations-fx-and-exotics/callable-range-hw.md)
   and [exercise and observation grids](observation-grids.md).

4. **Represent convertible trigger and dividend terms faithfully.** Remove
   implicit 130% and 20-of-30 terms. For the supported shared trigger
   profile, require a finite threshold percentage above 100 and
   `1 ≤ required_days ≤ observation_days`. Compute the checked, positive
   level as `conversion_price × (threshold_pct / 100)`. The inclusive
   profile counts equality on eligible observations. Require the contract's
   qualified comparator, trading-day window and cutoff, observation source,
   and applicable dated conversion price. Other profiles require owner
   support or return unsupported. Require exact historical and modeled
   window state for a path-dependent calculation or a deliberately selected
   and qualified product approximation. The existing single-node test and
   soft-call threshold uplift need separate qualification. Keep
   `None`, `AdjustPrice` and `AdjustRatio` as distinct terms. The product
   owns the corporate actions, exercise and dividend adjustment laws,
   precedence and any disclosed continuous-yield approximation. See
   [convertible lattice](../valuations-other-fixed-income/02-convertible-lattice.md),
   [filed inclusive trigger](https://www.sec.gov/Archives/edgar/data/2070457/000121390026015676/ea0276552-424b3_twentyone.htm),
   [filed strict trigger](https://www.sec.gov/Archives/edgar/data/1831481/000183148126000082/a424b5-ptsxxconvertiblenot.htm)
   and [Broadie, Glasserman and Kou](https://www.columbia.edu/~mnb2/broadie/Assets/bgk_mf.pdf).

5. **Bind quanto and underlying inputs explicitly.** Require distinct
   ordered asset and payoff currencies, the exact foreign financing and
   FX spot/volatility sources and finite correlation in `[−1,1]`. The
   authoritative spot is positive and measured in payoff-currency units per
   asset-currency unit. Require compatible vol conventions, coordinates,
   clocks and discount roles. A qualified reciprocal log-return orientation
   inverts the quote and reverses correlation once. Keep smile-node
   conversion and forward-strike resolution with the product. For FX
   underlyings require distinct ordered base and quote currencies, with
   domestic curve bound to quote and foreign curve to base. Retire generic
   FX-pair presets and identifier-text inference. Validate exact nonempty
   equity/index identities and market source IDs, supplied optional yield
   and duration IDs, and required commodity type, symbol, unit and currency.
   The product binds commodity price or curve sources. See
   [quanto orientation](../models-analytic/quanto-drift.md),
   [FX quote conventions](../core-conventions/011-fx-date-and-quote-conventions.md)
   and [quanto covariance derivation](https://www.columbia.edu/~mh2078/ContinuousFE/FX_Quanto.pdf).

6. **Give recovery, defaults and numerical controls an owner.** Require
   explicit shared CDS-family reference and hazard source identities and
   finite contractual recovery `0 ≤ R < 1` with the accepted product and
   market-source checks. Keep realized settlement recovery and Merton's
   inclusive endpoint on their own bases. Remove the generic 40%/30%
   presets and implicit Modified Following, Short Front, ACT/365F, cash
   settlement, unit multiplier and Black selections. Allow explicit terms
   or a qualified, explicitly selected owner profile. Keep exact unit
   constants. Move numerical convergence tolerances to the algorithms
   owning their units and error tests. Reconcile the shared rate tolerance,
   survival floor, par denominator gate, 252-day default/conversion and
   CDS-option clocks/quadrature values to the accepted local survival,
   positive annuity, explicit annualization, dated clocks and qualified
   quadrature decisions. An inherited numerical constant cannot establish
   default state, market convention or vendor equivalence. See
   [CDS legs](../models-and-valuations-credit/12-cds-legs.md),
   [index valuation](../models-and-valuations-credit/14-index-valuation.md),
   [CDS option pricing](../models-and-valuations-credit/15-cds-option-quadrature.md),
   [CDS option risk](../models-and-valuations-credit/16-cds-option-risk-and-inversion.md)
   and [observation grids](observation-grids.md).

7. **Use one explicit PIK schedule selection.** Choose the bond's
   contractual coupon rule or an explicit model schedule. `Uniform(Cash)`
   means cash. For a stepped schedule require an explicit initial mode at
   zero, finite strictly increasing positive subsequent knots, and a fixed
   origin and clock for the contractual coupon coordinates. On or after a
   knot, use that mode until the next knot. Reject nonfinite or negative
   queries. Preserve the dated meaning when valuation moves. Require one
   exact Decimal cash fraction `c ∈ [0,1]`. For emitted monetary events
   allocate the complete parent coupon once and use its exact residual for
   PIK. Preserve parent identity, payment dates and economic capitalization
   boundaries separately. An active Toggle requires an explicit supported
   policy and required historical or modeled state. Remove the missing-model
   Cash fallback. Contractual elections and simulation arithmetic and
   conservation remain with their owners. See
   [coupon and PIK allocation](../cashflows/05-coupon-cash-and-pik.md),
   [principal and event order](../cashflows/03-principal-and-emission-order.md)
   and [bond simulation](../valuations-bonds/08-merton-simulation.md).

8. **Require explicit Merton run and calibration controls.** Require a
   compatible structural model, positive checked grid control, barrier
   monitoring selection, recovery `0 ≤ R ≤ 1` and PIK source. Also require
   the independent estimator count, antithetic selection and exact seed
   or explicitly selected qualified derivation for the pricing run. Terminal
   monitoring uses the compatible discrete endpoint route. For first
   passage require the chosen supported crossing policy. Check optional
   hazard, dynamic recovery, exercise and discount overrides and reject
   conflicts and unused controls. An elected calibration supplies its
   target, parameter, finite positive cash-PV tolerance, positive solver
   budget, at least two independent estimators and explicit bracket and
   seed policy. Reuse of the run's seed or a bracket derived under an
   owner-qualified policy requires explicit selection. Remove constructor
   grid, barrier and Cash inference, automatic calibration defaults and
   input floors or caps that silently rewrite budgets. Every route either
   applies the selected calibration or rejects it as unsupported. Bond and
   credit owners retain the actual structural dynamics, quote conversion,
   term-structure treatment and calibration residual qualification. See
   [model execution controls](model-resolution.md),
   [bond calibration](../valuations-bonds/09-merton-calibration.md) and
   [dynamic recovery](../models-and-valuations-credit/06-recovery-and-lgd.md).

9. **Use one explicit stochastic run basis.** Structured-credit runs require
   an independent estimator count `N ≥ 2`, explicit antithetic selection
   and exact seed or explicitly selected qualified seed derivation. Share
   draw, work and provenance rules across direct and registered routes.
   Check the product's resource limits, path multiplication and host
   count representation before any draw. Remove automatic 5,000,
   antithetics and the minimum-50 repair. Report the independent estimators
   and physical trajectories distinctly for both structured credit and
   Merton. One paired mean contributes one independent estimate. Product
   owners qualify coupled draws, paths, loss and waterfall estimation. See
   [Monte Carlo estimation](../models-monte-carlo/01-engine-and-estimation.md),
   [streams and seed](../models-monte-carlo/02-random-streams.md) and
   [stochastic waterfall](../valuations-structured-credit/15-stochastic-waterfall.md).

10. **Publish complete results with honest availability.** Finish the
    requested calculation before publishing, and check denominations,
    counts, sample basis, finite ordered uncertainty bounds, probability
    domains, tranche IDs and shape on publication and restore. Require
    one result currency for the related amounts, including zeros. Use the accepted independent
    estimator basis for mean PV uncertainty. Compute dirty price as
    `100 × mean PV / positive current note balance` on the matched deal
    or tranche basis. A zero balance leaves this price unavailable. Publish
    clean only when matched accrued cash permits it. Report conditional WAL
    with its contributing count and leave it unavailable when no principal
    paths contribute. Leave uncomputed duration and optional diagnostics
    unavailable. Preserve computed zeros, signed amounts and complete path
    data when selected, with checked shape and provenance. Keep owner-defined
    loss statistics, confidence choice, cash and valuation reconciliation
    in the product record. Keep loss-tail confidence separate from mean PV
    interval confidence. Reconcile Rust builders, serde, Python, WASM,
    schemas and reporting through existing routes, including dedicated
    pricing and restore entry points. See
    [structured-credit reports](../valuations-structured-credit/22-diagnostics-and-reporting.md)
    and [metric reporting](metric-orchestration.md).

## Observed behavior

## Choice and input-domain algorithms

These contracts are shared across product families. Their small selectors/defaults affect model behavior even though the enclosing files are primarily data definitions.

`PayReceive::sign` maps Receive to +1 and Pay to −1; `Position` defaults Long and maps Long/Short to +1/−1 with exact lowercase parsing. Fixed-leg schedule conversion copies accrual dates, frequency, payment lag and caller-selected accrual adjustment, uses no reset/roll rule, and uses weekends-only when its optional calendar is absent. Float-leg conversion additionally takes the resolved calendar/reset lag from its caller; its own optional-calendar helper also falls back to weekends-only. Both explicit validators require `start<end`; float additionally rejects SimpleAverage. They do not independently validate every lag or convention. Protection-leg construction validates finite recovery in `[0,1)`; direct field/serde construction bypasses that constructor and relies on the product boundary. Premium-leg data retains explicit roll rule and decimal bp coupon; its actual CDS schedule constraints are imposed by the credit product.

- **Issuer call dates:** construction stores dates without validating. Explicit validation requires a nonempty strictly increasing date list and positive percent-of-par call price. The price test is only `price>0`: positive infinity passes this local validator, while NaN fails. Eligibility keeps dates strictly after lockout, so equality is excluded; lockout can leave no eligible dates. Downstream callable-range and other product paths separately decide support and valuation.
- **Convertible trigger:** default threshold is 130 percent of conversion price, with 20 required days in a 30-day observation window. The level helper computes `conversion_price×threshold_pct/100`. Validation requires finite threshold above 100 and `0<required_days≤observation_days`. This shared type stores a window; it does not simulate window memory. `DividendAdjustment::is_protected` treats AdjustPrice and AdjustRatio identically as protected; the product continuous-yield approximation owns how that affects tree value. Conversion ratio/price preference, mandatory delivery, anti-dilution and event ordering belong to the convertible kernel, not this plain container.
- **Quanto contract:** constructor/validator requires finite correlation in `[-1,1]`. Asset currency, asset financing curve, FX vol ID and positive FX spot semantics are declared here; positivity, scalar currency and forward strike lookup are checked by the consuming pricer. The FX quote is payoff-currency units per asset-currency unit, which fixes the sign interpretation of the correlation adjustment.
- **PIK schedule:** uniform mode returns directly. A stepped schedule starts Cash, walks entries in supplied order, uses the last entry with `step_time≤t`, and stops at the first future entry. This method itself does not sort or validate entries/fractions. The consuming bond engine validates them. Equal/unsorted times cannot be assumed harmless merely because the shared type deserializes. Toggle's no-model behavior belongs to the consuming engine; the type documentation describes Cash fallback.

Credit parameter construction is infallible, with corporate/sovereign recovery presets 0.40/0.30; its explicit validator requires finite recovery in `[0,1)`, unlike the Merton configuration's inclusive upper bound. Equity/commodity/index underlying validators require nonempty routing labels, but their constructors mainly store caller data. FX convenience constructors select EUR/USD, GBP/USD, and JPY/USD base/quote order respectively; the helper named `usd_jpy` actually places JPY as base and USD as quote. Caller names must not override the fields' quote direction.

Shared serde helpers explicitly return ModifiedFollowing, ShortFront, ACT/365F, cash settlement and multiplier one. The volatility-model enum defaults to Black. Numerical constants declare rate comparison `1e−12`, credit survival floor `1e−15`, par-spread denominator `1e−12`, 252 trading days, and senior recovery 0.40. The CDS-option constants declare a 365-day pricing clock, 365.25-day theta clock and normal quadrature grid half-width at least six with step 0.05. Actual consumption and solver convergence are documented by the product records; naming these constants after a source is not independent evidence of correct implementation.

## Merton and structured-credit sampling controls

`MertonMcConfig::new` validates recovery in `[0,1]`, reads grid density from embedded defaults, uses BrownianBridge for first-passage barriers and Discrete for terminal barriers, and starts with Cash coupons and no optional hazard/recovery/toggle/calibration/DF override. Setters mostly store data; they do not run a full engine-domain check. Calibration defaults target zero z-spread, solve the debt barrier, use 2,000 independent estimators, at most 40 bisection steps and `1e−4` currency PV residual tolerance, with automatic bracket/seed. The actual bisection objective, recovery dynamics and DF interpolation are in the [bond records](../valuations-bonds/README.md).

`StructuredCreditPricingMode` defaults to 5,000 antithetic estimators (10,000 paths). Its convenience `monte_carlo(n)` silently floors `n` to 50 and enables antithetics. Direct enum construction/serde does not use this floor; schema metadata permits at least two, and the engine must enforce its own domain. Result constructors initialize price/risk/uncertainty fields to zero, ES confidence to 95%, and empty path/tranche vectors; those initial values are placeholders, not computed estimates. Standalone setters do not validate cross-currency or probability consistency. The deal-level clean-price field is documented as unadjusted/equal to dirty price because this result payload lacks per-tranche accrued cashflows; actual population is in structured credit.

## Alternatives, evidence

The accepted decision requires checked construction, explicit estimator admission, and qualified observation state or an explicitly selected approximation. Product records carry their own model and feature qualification.

[parameter defaults](../../../../finstack-quant/valuations/core/src/serde_defaults.rs), [numerical constants](../../../../finstack-quant/valuations/core/src/constants.rs), [market parameters](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/market.rs), [underlying parameters](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/underlying.rs), [bermudan_call.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/bermudan_call.rs), [convertible.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/convertible.rs), [quanto.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/quanto.rs), [merton_mc.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/merton_mc.rs), and [structured_credit.rs](../../../../finstack-quant/valuations/core/src/results/structured_credit.rs) own these contracts and selected inline tests. Test evidence concerns constructors, explicit classification, lockout and valid/invalid inputs; it does not independently validate downstream option/default pricing. No independent source establishes the numerical defaults or their adoption.

## Independent review evidence

- For an explicit weekends-only payment calendar, Saturday 2025-01-11
  rolls to Monday under Following and payment with one further business day
  of lag is Tuesday 2025-01-14. A spread of −25 bp is −0.0025 as a decimal
  rate. For face 1,000 the amount at 102% is 1,020. A call date equal to
  the lockout end is blocked even if every remaining date is blocked too.
- Conversion price 40 and threshold 130% give level 52. Two 30-close
  histories ending at 52 can have 21 and 19 inclusive hits, respectively.
  Only the first qualifies under the 20-of-30 rule. A strict comparison
  counts none if the observations are all 51 or 52.
- For spot 100, foreign log carry 0.02 over one year, asset vol 0.20,
  FX vol 0.10 and oriented correlation 0.5, quanto forward is about
  101.00502. Using the sign from the inverse quote without orientation
  conversion gives about 103.04545.
- For conditional integrated hazard `ln(2)`, survival conditioned on the
  starting cut is 0.5 even if absolute survival at the cut is `1e−20`.
  A protection PV of `1e−16` and a positive matched annuity of `1e−14`
  give 100 bp when admitted on that basis.
- For PIK at zero and Cash from time 2, queries below and at time 2
  choose PIK and Cash, respectively. A coupon of 12.50 with cash fraction
  0.30 produces 3.75 settlement cash and 8.75 economic principal increase.
- Antithetic PV pairs (90, 110) and (110, 130) give two estimator
  observations, 100 and 120, from four trajectories. Mean PV is 110
  and sample standard error is 10. The two-sided 95% Student-t interval
  is approximately [−17.062, 237.062] with one degree of freedom. Mean
  PV 60m on a positive current balance of 60m gives dirty price 100,
  with clean unavailable when matched accrued cash is missing.

These examples check the proposed boundaries and arithmetic. Qualification
needs isolated product fixtures, exact calendar and source authority, dated
exercise and PIK events, matched model and calibration runs, resource and
failure cases and complete host round trips.
[Merton registry pricer](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/pricer.rs),
[structured-credit engine](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/stochastic/pricer/engine.rs).

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
