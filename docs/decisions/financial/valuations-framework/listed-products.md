# VALUATIONS-FRAMEWORK-03 — Listed futures and options: pricing, exercise, and settlement

- Decision status: **Accepted — complete checked listed futures and options package, 2026-11-13**.
- Implementation status: **Observed; reconciliation pending** across shared terms, instrument owners, clearing inputs and hosts.
- Validation status: Source and clearing references inspected. Independent arithmetic and lattice examples checked. Production pricing, venue settlement and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked listed futures and options

1. **Bind the actual contract and quotation units.** Require exact
   contract and underlying identities, settlement currency, positive
   finite quantity and point multiplier, explicit position, contract
   dates and settlement form. Prices, strikes and marks use the same
   declared point denomination. Use signed prices when the selected
   contract admits them. Distinguish fractional analytical exposure
   from clearing quantities, which must obey the applicable lot rules.
   Resolve explicit authored terms or an exact qualified venue
   specification with source, version and applicable effective
   date retained. The listed catalogue describes coverage. It
   supplies no active contract, quotes or market data. Remove
   implicit Cash, long-position and UST 10-year selections where
   they currently fill missing economics. Remove the panic-based
   specification lookup. If a contract specifies a constant tick
   size and cash per tick, check the multiplier against their
   ratio on the same denomination. Preserve unrounded model
   marks. Apply tick or clearing cash rounding only at its
   explicitly owned stage. A tick does not select a generic
   Money precision or payoff rounding.
   [CME clearing calculations](https://www.cmegroup.com/clearing/files/CME-Money-Calculations-Futures-and-Options.pdf).

2. **Define futures marks by lifecycle and reference.** At the
   effective valuation close, select an exact observed trading mark
   or an explicit live product model mark. Preserve that selection
   and reject a failed required lookup or model calculation. Resolve
   the actual contract phase at the last trading cutoff. If a civil
   date cannot distinguish the pre-cutoff and post-cutoff states,
   require the applicable session or authoritative phase. After
   trading, require the official contract final mark when it is
   needed, with its observation and availability qualified to the
   valuation information set. A stale live quote or modeled mark
   cannot stand in for final settlement. Report the undiscounted
   mark or variation `side × contracts × multiplier × (mark−reference)`.
   Use trade fill for the new-trade reference, or the last applied
   official clearing mark for a carried position. Label any
   cumulative trade P&L separately. Reconcile pending variation
   and exclude booked or settled cash from a second mark on the
   same interval. Live point delta on this mark basis is
   `side × contracts × multiplier`; a fixed final mark has
   no live-price delta. This is shared VM arithmetic. Invoice,
   deliverable basket and physical delivery economics belong to
   the product owner.

3. **Separate modeled option quotes, position marks and booked cash.**
   Require explicit European or American style, Black-76 or
   Bachelier, premium-paid or futures-style margining, and cash or
   future settlement. Require an exact matched underlying mark,
   a complete option model clock and, for modeled delivery, a
   settlement currency and point scale consistent with the
   deliverable as described below. Select an explicit native
   scalar volatility for the live model under
   [record 02](model-resolution.md#accepted-decision-for-checked-valuation-model-inputs-and-controls).
   Black uses positive finite futures price and strike and
   relative decimal sigma per square root model year. Normal
   accepts finite signed futures price and strike and uses
   price points per square root model year. Retain exact zero
   sigma. Require positive modeled time on a live date-based
   price. Use the accepted
   [unit forward option kernels](../models-analytic/forward-normal-shifted.md)
   for European-equivalent prices. Compute the unsigned holder
   quote first, then apply position, size and multiplier once.
   A premium-paid cash option uses relative discount to its fixed
   payment date. For the admitted premium-paid futures-delivery
   profile, exercise-time VM is represented at exercise, so the
   European payoff uses relative discount to expiry. The admitted
   futures-style live quote uses factor one. Report its
   reference-based position mark separately from the gross unit
   quote. Require the labelled trade or prior official option
   reference for the live mark. Premium, margin and settlement
   obligations follow their actual cash ownership, dates and
   status. A recorded reference is neither an unpaid premium
   nor a delivered future's reference.
   [Black 1976](https://doi.org/10.1016/0304-405X(76)90024-6)
   and [CME margining styles](https://www.cmegroup.com/education/articles-and-reports/a-primer-on-margining-styles-for-options).

4. **Use the European expectation for qualified fixed-payment
   American cash and futures-style quotes.** In the selected
   deterministic-rate model, the futures price is an integrable
   martingale and intrinsic is convex. If exercise leaves payment
   on the same fixed date and introduces no other rights, funding
   or margin effects, conditional expected terminal intrinsic
   is at least intrinsic at the earlier exercise time. Use the
   European expectation in this admitted case. The same convex
   comparison applies to the admitted undiscounted futures-style
   profile when the exercise reward has the same unit quote
   basis and there is no exercise-dependent carry. Apply the
   position sign after valuing the holder's exercise right.
   A contract with an exercise-dependent cash date or different
   closing-margin economics needs an explicit separate model.
   Stochastic rates, delivery choice and Bermudan schedules are
   outside this shared profile.

5. **Use one term-structured lattice for premium-paid American
   futures delivery.** Select checked zero-carry CRR for Black,
   with reciprocal multipliers `u=exp(σ sqrt(h))`, `1/u` and
   up probability `1/(1+u)`. For Normal use symmetric additive
   moves `±σ sqrt(h)` and probability one half. Both model a
   martingale futures price. Use an explicit positive number
   of subdivisions per civil day on the ACT/365F model clock
   and the exact authored eligible exercise closes, including
   expiry. A permitted immediate exercise is an explicit
   origin opportunity. Bind each eligible civil close to its
   indexed day boundary. Intermediate subdivision nodes have
   no invented exercise right. Use the checked mapped discount
   source at each model node under the accepted fractional-day
   rate-clock mapping, and discount continuation by adjacent
   factor ratios. Use the greater of continuation and intrinsic
   only at eligible exercise nodes. Check the complete planned
   work for both grids before allocation. With zero volatility, use
   the single constant futures path and maximize discounted
   intrinsic over eligible exercise nodes. Require positive
   checked grid and aggregate node-work limits. Use checked
   geometry, weights and accumulation. Remove implicit 401/201
   grids and constant-rate inference from the expiry factor.
   Compare the explicit base and doubled subdivisions at
   identical exercise dates and require a caller-selected
   positive per-unit premium tolerance. Return the finer
   qualified value. This is a finite-grid stability check.
   It gives no general error bound against continuous exercise
   or an exchange's intraday exercise windows.
   [Checked CRR and LR](../models-numerical/equity-binomial-construction.md),
   [exact grids](../core-numerics/time-grids.md),
   [mapped discount clock](../models-analytic/rate-clock-and-projection.md).

6. **Use authoritative exercise and settlement state.** Share one
   admitted live, closed-unexercised or exercised/assigned state
   across PV, quotes and risk. At and after expiry close require
   the resolved outcome. European exercise belongs to expiry.
   American exercise must be within its eligible window. Require
   exact contract, lot, date, action and fixing provenance as
   applicable. Split partially resolved lots into separately
   identified positions. An observed price does not decide whether
   exercise or assignment occurred. Respect a confirmed contrary
   action where the actual rules allow it. A cash-settled option
   uses the authoritative signed payoff and due date. If deriving
   a confirmed amount, use the applicable contractual rule and
   official observation for that resolved action. Future delivery
   creates the exact underlying position at strike, with direction
   `option position sign × (+1 call or −1 put)` and the
   contract's actual delivery ratio. The simple intrinsic model
   requires the same settlement currency and an option point
   multiplier equal to the delivery ratio times the futures
   point multiplier after explicit point-unit conversion.
   Delegate that future to its own mark, reference and settlement
   state. If the option temporarily owns an untransferred
   position, retain one explicit
   owner and stop including the future at transfer. Clear the
   option reference from the delivered-future calculation.
   [CME exercise and assignment rules](https://www.cmegroup.com/clearing/contrary-option-exercise-instructions.html).

7. **Value only eligible unpaid obligations after resolution.**
   Preserve identified premium, final option variation, exercise
   cash and any initial futures variation as separate applicable
   clearing obligations, or use an authoritative net obligation
   reconciled to its components. A live modeled quote is not
   evidence of clearing cash. Use explicit same-day eligibility
   and factor one for included same-day cash. Discount a required
   future fixed claim on its admitted dated settlement basis under
   [CORE-CONVENTIONS-016](../core-conventions/016-discounting.md),
   including unpaid finalized futures variation and fixed cash
   left after removal of a futures-style option.
   Use settled and transferred history exactly once. Resolve
   overdue unpaid claims explicitly. Validate terms and state
   before an empty residual. A date past settlement is not by
   itself evidence of closure, and a confirmed closed lot need
   not obtain unused volatility, marks or reference quotes.
   [CME clearing calculations](https://www.cmegroup.com/clearing/files/CME-Money-Calculations-Futures-and-Options.pdf).

8. **Give live sensitivities explicit units and a common price basis.**
   For a positive-time, positive-volatility European-equivalent
   price use the accepted analytic futures-price Delta and Gamma
   and native model Vega. Hold the quote volatility, clock and
   payoff discount basis fixed for price sensitivity. Scale once
   in holder sign. Report Delta per futures-price point, Gamma
   per point squared and Vega per +.01 of the selected native
   volatility. The latter means one percentage point of Black
   volatility or .01 futures-price points per square root year
   of Normal volatility. Respect the analytic bundle's absence
   at its deterministic boundary. For American delivery use
   checked central price stencils with explicit point step `h_F`
   on the same price kernel. Use `(V_+−V_-)/(2h_F)` and
   `[(V_+−V_0)+(V_-−V_0)]/h_F²`. Use an explicit native vol step
   `h_σ` and central Vega `.01 × (V_+−V_-)/(2h_σ)`; use
   `.01 × (V_+−V_0)/h_σ` only when the lower step is outside
   the admitted volatility domain. Reject unresolved moves,
   failed reprices and invalid Black price endpoints. Use the
   same grid pair throughout the price and risk experiment. Require the same
   measure on base and finer grids to pass an explicit stability
   tolerance in that measure's units, and report actual stencils.
   A finite lattice can have price kinks; a computed zero stencil
   is not a proof of structural zero or derivative convergence.

9. **Resolve rate, time and lifecycle risk without fabricating marks.**
   An explicit signed futures-price move per +1bp is required
   only for a requested mapped futures-price rate sensitivity on
   an active branch. Multiply it by the qualified futures-point
   Delta, and label the source and units. A discount-rate DV01
   reprices the actual discount leg, including any eligible fixed
   future cash, using its curve clock. Remove the maturity-times-
   total-PV shortcut. A combined rate scenario requires its own
   matched target and repricing basis. For a live next-day metric
   select a one-civil-day fixed-market reprice with futures quote
   and annual model sigma held fixed, using the admitted dated
   relative factors at both closes. Require both closes to be
   live and compatible with the same contract and timing rules.
   A crossing into expiry, exercise or cash settlement requires
   the authorized horizon and lifecycle calculation under
   [record 13](carry-theta-breakeven.md). Do not synthesize an
   expiry fixing. Apply the same state admission to every metric.
   A fixed cash claim has no live futures or vol sensitivity,
   but can retain discount exposure. A delivered position carries
   its own futures-point risk. The scalar volatility here is a
   supplied pricing input. This package adds no listed IV inverse.

10. **Use one checked Rust path and report its actual basis.**
    Keep raw pricing and risk arithmetic checked in native
    currency and apply Money finalization once under
    [CORE-CONVENTIONS-012](../core-conventions/012-money-decimal-precision.md).
    Report selected contract specification, market mark and
    reference, cutoff and status, model clock, grid, requested
    and represented shocks, dates, and clearing versus modeled
    cash provenance. Pricing, risk and export share the same
    resolved state. A known cash claim may export its dated
    cash under the product owner. An unresolved contingent
    decomposition is unavailable. A qualified closed empty
    residual uses the accepted schedule semantics. Reconcile
    asset-owned wrappers, native, registry, risk, portfolio,
    clearing-report inputs, Python/WASM, strict restore, wire
    schemas, declarations, parity and fixtures. The shared
    model does not infer a venue's current calendar, exercise
    automation or settlement rounding from a contract symbol.

## Observed behavior

### Choice, domain, and rationale

`ListedFutureTerms` values a signed contract position by variation-margin mark to market, `s × contracts × multiplier × (mark − entry_price)`. `FutureOptionTerms` separately makes price model, exercise, premium margining, and cash versus future delivery explicit. The models are Black-76 with positive futures price/strike and decimal lognormal volatility, or Bachelier with unrestricted finite futures price/strike and normal volatility in price points per square-root year. Contract count and multiplier must be finite and positive; direction is `Position`.

For an American option whose exercise cash is paid on a fixed contractual date, the code deliberately uses the European expectation. Its documented rationale is the convex payoff of a martingale futures price with deterministic rates: waiting dominates exercise when the payment date does not change. This is an economic assumption needing adoption, not a general claim about all American cash options.

### Algorithm and lifecycle

1. A live future resolves an explicit `quoted_price` before a supplied model-price closure. Strictly after last trading, it requires `settlement_price`. Strictly after settlement it returns zero; settlement-date MTM remains included. No discounting is applied. Physical asset/quantity is validated metadata; this helper does not value a delivery basket.
2. A live option requires an explicit implied-volatility quote in instrument overrides, including a supplied zero. Time uses the terms' day count. Premium-paid options discount to cash payment, or to expiry for future delivery; futures-style options use discount factor one.
3. European and fixed-date cash American calls use `DF[F N(d1) − K N(d2)]` under Black-76, or `DF[(F−K)N(d)+σ√T φ(d)]` under Bachelier; puts use the corresponding opposite payoff. At zero time or volatility the branch returns discounted intrinsic.
4. American future delivery with positive volatility uses a martingale futures lattice. Black-76 delegates to Leisen–Reimer with dividend yield equal to the effective rate `−ln(DF)/T`. Normal uses additive moves `±σ√Δt`, probability one half, and backward induction `max(intrinsic, exp(−rΔt)(Vdown+Vup)/2)`. Futures-style effective rate is zero. The deterministic delivery branch returns `intrinsic × max(DF,1)` to choose immediate versus terminal exercise.
5. Position value is the signed scale times the unit quote; futures-style additionally subtracts `option_reference_price`, which is required and nonnegative. This is MTM from the reference quote, not an undiscounted premium balance.
6. From expiry onward, an exercise/expiry observation is mandatory unless the terminal settlement date has passed. European observations must be on expiry; American observations may be earlier. A recorded exercise delivers a future only if the observed price was strictly in the money. Cash uses the observed intrinsic paid on the contractual date. Delivered futures use `call/put sign × (current mark−strike)`; after underlying last trading they require official settlement price. All branches return zero strictly after terminal settlement.

### Risk selection and controls

Official American PV defaults to 401 tree steps; finite-difference risk defaults to 201. `tree_steps` overrides both. Delta uses `(V(F+h)−V(F−h))/(2h)` with `h=max(|F|×1e−4,1e−4)`. Gamma uses the centered second difference with `h=max(|F|×1e−3,1e−3)`. Black bumps are capped at half the positive futures price. Vega is the cash change from `σ+0.01`, without division; this is one absolute volatility point in the selected model's units. Theta is next-calendar-day value less current value, capped at expiry, on the same market curves. All are scaled by signed contracts/multiplier.

After exercise, gamma/vega/theta are zero. Delta on a delivered in-the-money future is signed contracts/multiplier until last trading, then zero after verifying settlement price. At expiry without an observation the Greek functions return zero whereas PV errors; callers must not use zero Greeks as evidence that the lifecycle observation exists. The interest-rate option macro additionally requires `underlying_price_change_per_bp` for rate-risk mapping.

### Limitations, alternatives

- Bermudan exercise is rejected because no exercise schedule is supplied. Stochastic rates, exercise-dependent cash-payment dates, and delivery choice/basis are not modeled by these terms.
- American risk is computed on a different default grid from official PV.
- The additive normal tree and effective constant rate approximate the supplied discount term structure. A term-structured lattice or PDE is an unimplemented alternative here.
- A future's post-settlement zero branch precedes terms validation; settled malformed terms need not produce an error through that path.

## Evidence

- [FutureOptionTerms](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/types.rs): owning formulas, 401/201 defaults, exercise and settlement branches, finite differences.
- [Instrument macro](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/instrument_impl.rs): cross-asset delegation, overrides and rate-risk requirement.
- [ListedFutureTerms](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/terms.rs): explicit mark priority, MTM formula and date boundaries.
- [Future-option implied-volatility tests](../../../../finstack-quant/valuations/tests/instruments/future_option/implied_volatility.rs): located cross-asset behavior/roundtrip evidence; a roundtrip is not an independent price benchmark.
- [Black (1976)](../../../REFERENCES.md#black-1976), [Bachelier (1900)](../../../REFERENCES.md#bachelier-1900), and [Hull](../../../REFERENCES.md#hull-options-futures) provide canonical model context. These citations do not establish approval of the lifecycle rules or tree controls.

### Independent review evidence

- Two long futures, tick size `.0025` worth `6.25`, have
  a point multiplier of `2,500`. With fill `95.00`, prior
  settled mark `95.10` and new mark `95.125`, current
  variation is `125` and cumulative trade P&L is `625`.
  A finalized unpaid claim of `125` with factor `.99`
  has PV `123.75`. Separately, rounding per-contract money
  endpoints `1.006` and `1.004` to cents gives a difference
  of `.01`, whereas rounding their difference gives `.00`.
- For a futures-style long call, strike `100`, previous
  option reference `7`, final option quote `8`, first
  delivered futures mark `108` and multiplier `1,000`,
  separate clearing components are `+1,000` final option
  variation, `−8,000` premium and `+8,000` first futures
  variation. Their sum is `+1,000`. A subsequent futures
  mark of `110` gives `+2,000` using the new reference
  `108`. The components demonstrate the ownership boundary.
- Black inputs `F=104,K=100,σ=.2,T=.5` give gross unit
  call and put `7.969257478694` and `3.969257478694`.
  A fixed payment factor of `.96` gives premiums about
  `7.650487179546` and `3.810487179546`. With factor
  one, signed scale `+10` and reference `2`, the call
  margin mark is about `59.692574786939`. Normal inputs
  `F=−4,K=−5,σ=2,T=1` give a positive gross unit call
  of about `1.395593114803`.
- For a Normal two-step delivery call, `F=100,K=99`,
  moves of one point, eligible exercise at both receiving
  nodes and relative factors `1.05,.98`, the undiscounted
  terminal intrinsic values are `0,1,3`. Discounting
  continuation by the two actual step factors gives
  `1.295`; using the constant step factor `sqrt(.98)`
  gives about `1.234949493661`. At zero volatility, a
  constant intrinsic of `10` and factors `1,1.05,.98`
  on eligible exercise nodes gives `10.50` at the
  interior node, while using only the endpoints gives `10`.
- At-the-money Normal `F=K=98.5,σ=1.2,T=.5`, factor
  `.98`, and two long contracts at `2,500` per point,
  yield PV about `1,658.7174`, Delta `2,450` per point,
  Gamma `2,303.7741` per squared point and Vega
  `13.8226` per +.01 native sigma. A mapped futures move
  of `−.01` points per +1bp gives `−24.50` per bp.
  Separately, in a unit-factor ATM Normal tree with
  `σ=1.2,T=1`, odd grids of 201 and 401 steps both
  give zero Gamma in exact arithmetic for a central
  point step of `.001`. The strike stays between the
  same terminal nodes. Analytic unit Gamma is about
  `.3324519`. Grid agreement alone cannot establish
  sensitivity accuracy.

Production qualification still needs dated venue examples for
marks and clearing cash, payoff timing and model admission,
price and Greek convergence over representative grids and steps,
and shared native, registry and host lifecycle checks.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
