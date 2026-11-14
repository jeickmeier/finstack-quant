# VALUATIONS-OTHER-FIXED-INCOME-03: Convertible inverse quotes, sensitivities and spot regimes

- **Decision status:** Accepted 2026-11-14 — full checked convertible quotes and risk package, including separate OAS and implied hazard-spread fits.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Reconciliation pending. The observed entry points and kernels are described below.
- **Validation status:** Live sources and primary references inspected. Independent settlement, risk and inverse arithmetic checked. Runtime, model, market and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked convertible quotes and risk

1. **Bind settlement, cash accrued and quote authority.** Accept an
   eligible actual buyer settlement date and phase, or resolve it from
   explicitly supplied trade date, settlement calendar, lag and
   convention. Retain the valuation information cut. Require a positive
   contractual quote denomination for the named position and claim.
   Convert per-100 points to native whole-position Money by
   `points × denomination/100`. Use one authoritative clean or dirty
   observation, with units, currency, observation cut and provenance.
   A native clean quote is dirty less signed cash accrued. Compute
   accrued on the original cash coupon parents at settlement using
   complete day-count and an explicit first-ex or all-cum rule:
   `C × [f(S)−1_{X≤S}]` before the cash payment date, where `f(S)`
   is the original-parent accrual fraction and the indicator applies
   when a first-ex date `X` exists. Reset that quote adjustment on
   payment date. Actual same-day ownership follows the payment phase.
   PIK and principal can affect the balance and recovery independently.
   A dirty-only request requires only its active inputs. Keep the
   registered clean and accrued amounts in native currency. Preserve
   observed quote, model-derived quote and raw holder PV as distinct
   authorities. A quote target does not pin or overwrite raw pricing.
   [Bond settlement](../valuations-bonds/02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks),
   [original coupon accrued](../cashflows/13-accrued-interest.md),
   [OpenGamma on settlement and issuer discount roles](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf).

2. **Value the settlement buyer's actual claim.** At opening cut `V`
   let `H` be the raw holder PV and `J` the buyer-claim PV on the same
   snapshot and model basis. With explicit qualified relative financing
   factor `F=D_fin(V,S)>0`, use dirty `Q=J/F=(H−Δ)/F` for
   `Δ=H−J`. At equal dates use factor one, with phase-specific rights
   still applied. Admit identical transferable claims and the checked
   bridge for separately valued additive retained rights, including a
   detached coupon payable after settlement. An interval with possible
   intervening exercise, default, contingent delivery or other state
   changes requires a qualified buyer-claim valuation accounting for all
   such possibilities at the original origin. Until that bridge is
   qualified, a model-derived settlement quote on that basis is
   unavailable and raw holder PV remains available. Never restart the
   tree at settlement, substitute the issuer discount for financing,
   infer delivery from maturity alone or drop retained claims. The
   inverse quote objective recomputes the trial-dependent buyer claim
   and uses the authorized financing factor held fixed.
   [Dated holder and buyer claim bridge](../valuations-bonds/02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks).

3. **Give valuation diagnostics comparable bases.** For admitted live
   fixed-ratio or single-spot mandatory-collar terms, compute
   contemporaneous indicative `conversion_value` on the dated
   effective terms and eligible par under record 02. Include actual
   exercise eligibility separately. For `parity` divide by the
   stated positive quote denomination of that position. Use
   `conversion_premium = P/X−1` with positive conversion value `X`
   and matched whole-position dirty holder price `P` at the
   valuation cut. Preserve negative premiums. A separately requested
   settlement premium needs dirty buyer price and a conversion
   reference on that same eligible settlement and claim basis.
   Closed and resolved positions have no live conversion diagnostic.
   An unknown required right remains unavailable or errors according
   to the source and result contract.

   Define `bond_floor` as the value of the qualified live standalone
   nonoption debt counterfactual. Use the authoritative fixed or
   qualified projected coupon/principal cash and eligible recovery
   basis, with exact dated discounting and record 02 recovery.
   Remove embedded conversion, call and put rights from this
   counterfactual. Require explicit cash redemption terms for
   a mandatory structure that lacks an independently admitted cash
   strip. The standalone result has no equity, vol or tree-grid
   prerequisite. Its value can exceed the live convertible, and
   pending fixed delivery keeps its own claim PV.
   [ICMA, conversion value and premium on gross/dirty basis](https://www.icmagroup.org/assets/documents/Media/Bondmarketsbook/Bond%20markets_structures%20and%20yield%20calculations.pdf),
   [dated debt cash and recovery](../valuations-bonds/03-pv-models.md).

4. **Fit the selected constant tree volatility.** Use the live
   OFI-02 prepared route and compatible dirty quote, freezing the
   dated grid, state, spot, discount, hazard, dividend yield and
   other inputs. The trial is ACT/365F annual lognormal
   `σ`; the inverse supplies it directly without a market
   volatility quote. For positive `σ` require the entire explicit
   finite bracket strictly above `max_i |b_i|/√h_i` under the
   accepted `b_i=−log D_i+H_i−q h_i`, and check each represented
   trial. Exact zero remains a valid forward model branch, outside
   the positive-volatility fit. Remove the hidden doubling,
   bisection of pricing failures and fixed upper-volatility cap.
   Flat/insensitive or expired objectives do not supply an implied
   volatility. Collars may have multiple matches, and an explicit
   trigger can jump across the target. Use the fit acceptance rule
   below and identify the selected bracket and effective grid.
   [Accepted convertible lattice](02-convertible-lattice.md#accepted-decision-for-checked-convertible-lattice-and-exercise).

5. **Use distinct OAS and implied hazard-spread experiments.**
   Define `oas` as one signed annual continuous ACT/365F spread
   `s` in the live valuation rollback. Multiply the base relative
   discount by `exp(−s h_i)` for each step and include the same
   clock once in the prompt-recovery integrals and attached
   post-decision claim valuation. Keep the base equity transition
   probabilities, survival and equity carry fixed. Reapply the
   same exercise resolver at each trial, with reference make-whole
   and settlement financing inputs held fixed. Separately vested
   independent receivables keep their own valuation law. The
   overlay fits the admitted claim while preserving the original issuer
   financing and pricing-measure curve. Require an admitted live
   issuer debt obligation with an embedded right and identifiable
   spread exposure. A pure share or resolved delivery claim is
   outside this fit.

   Carry forward the existing intensity-fit capability under the
   explicit name `implied_hazard_spread`. Require the active issuer
   hazard curve, keep its native clock, and add the signed annual
   intensity shift to its canonical parameters. Check all transformed
   intensities are finite and nonnegative. Recompute survival,
   prompt recovery, and conditional stock drift from that curve.
   Require the full bracket to respect the intensity and applicable
   CRR support domains. Report both fitted coordinates in decimal
   annual units on their own clocks. Neither metric changes meaning
   based on market-data presence. No fitted spread is a par-CDS quote
   or an independently inferred default probability.
   [Bond option-adjusted fit](../valuations-bonds/12-spread-quotes.md),
   [FHFA on OAS as a model-based relative valuation measure](https://www.fhfa.gov/media/33831).

6. **Require a bounded price fit and verify the returned quote.**
   Supply a finite ordered bracket, explicit work limits and positive
   native-currency residual tolerance. For an objective established
   continuous and strictly monotone on that interval, use checked
   fallible Brent with explicit positive coordinate tolerance.
   Otherwise require an expressly selected local fit, with bounded
   sign-preserving midpoint trials. Evaluate both endpoints in order.
   Admit an exact matching endpoint only when local isolation is
   qualified. Otherwise require opposite signs. Two matching
   endpoints or a structurally insensitive objective are
   unidentifiable. A price plateau supplies no identifiable
   parameter. Reprice the actual returned parameter before display
   rounding and gate `|J(x)/F−Q_target|≤tolerance`. Report achieved
   residual, fit basis and status. Local matches are bracket-selected,
   with no global uniqueness guarantee. Missing inputs, unsupported
   prices, discontinuities without a qualifying residual, exhausted
   budgets and nonadvancing work can return distinct errors.
   Remove fallback brackets, invalid-price sentinels, and
   expiry-based zero fills.
   [Checked Brent](../core-numerics/brent-root.md),
   [Brent's original bracketed method](https://maths-people.anu.edu.au/brent/pd/rpb005.pdf).

7. **Hold a comparable model basis across each Greek.** Compute
   requested risks independently on the raw or explicitly prepared
   fitted model, with actual state, claim basis, grid, source
   definitions, recognized observations and any elected fit held
   fixed except for the declared shock. Never refit to the old
   market quote during the ordinary risk reprice.
   - **Delta and gamma.** Use an explicit relative spot step
     `0<h<1` and genuine ordered positive represented endpoints
     around the spot. Use the shared endpoint secant and unequal-step
     gamma. Report delta in shares and gamma in shares per currency
     unit of spot. Hold the sampled strike and volatility basis fixed.
     The original eligible spot trigger responses remain active.
   - **Vega.** Shock the expressly selected active scalar or the
     complete designated native Black grid, keeping the surface
     sample coordinates fixed and reapplying the variance-clock
     bridge each time. Report currency per absolute `.01` of
     annual vol in that source's clock. Permit an opt-in full-up
     one-sided Vega only when the down side crosses a prechecked
     domain boundary and the base and up side lie in the same
     connected admissible domain. A zero-volatility point separated
     from the positive CRR region does not admit that secant.
     Missing data, collapsed moves and failed reprices do not select
     a different stencil.
   - **Rho, DV01 and theta.** Rho moves the designated discount
     source in continuous zeros, affecting every admitted valuation
     role of that source, including stock drift, with issuer hazard
     held fixed. Use explicit active typed discount, projection and
     reference scopes for combined or bucketed DV01. Report signed
     currency per positive rate bp using the shared checked
     stencils. Theta requires explicit dated cuts, qualified market
     conditioning and a consistent event/settlement scenario. Use
     `entitled cash + closing PV − opening PV` with each cash or
     delivery outcome counted once and surviving claims retained. A delivered share
     position needs an explicit close-value basis for the holder
     experiment to include it.
     Remove the implicit one-day roll and unrelated Greek failures.
   [Rate finite differences](../valuations-framework/finite-difference-rates.md),
   [option risk](../valuations-framework/option-sensitivities.md),
   [dated horizon theta](../valuations-framework/carry-theta-breakeven.md).

8. **Separate dividend, conversion, credit and mixed risk.**
   - **Dividend01** reports signed currency per +1 bp of continuous
     yield on an explicit step. Use the checked central stencil,
     or an elected full-up one-sided stencil at the prechecked
     nonnegative boundary. Explicit zero yield can have exposure.
   - **Conversion01** reports signed currency per +1% relative
     increase in the effective live fixed share ratio. Use explicit
     symmetric fractional moves on that ratio and derive the
     reciprocal conversion price from each trial on the same
     denomination. Carry through authorized conversion-price-linked
     triggers and hold independent thresholds, realized history and
     quote coordinates fixed. Variable collars and resolved share
     counts have no eligible live fixed-ratio experiment.
   - **CS01** shifts exact selected par-CDS quotes and recalibrates
     their qualified hazard source before repricing the full
     convertible. Hold recovery fixed, use signed holder currency
     per +1 bp, and preserve exact source/quote bucket identity.
     **Hazard01** uses a direct intensity move under the accepted
     shared policy and has its own keys and units. Do not manufacture
     a hazard curve for the no-default tree. A plain separately
     qualified debt strip retains its owner's z-spread experiment.
   - **Mixed risks** retain spot×vol, spot×credit, rates×credit and
     credit×vol on explicitly admitted independent axes. Use the
     full two-sided four-corner stencil and normalize by the actual
     spans in percent spot, rate bp, par quote bp or vol points as
     applicable. Stage the noncredit shock before replaying shocked
     quotes from the fixed credit center. Bound all work and propagate
     failed corners. Structural zero requires a demonstrated absence
     of exposure on the selected live or resolved claim basis.
   [Credit sensitivities](../valuations-framework/credit-sensitivities.md),
   [mixed risk and calibration order](../valuations-framework/cross-factor-risk.md),
   [White on CDS quote calibration and sensitivity](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf).

9. **Make the spot ladder an explicit scenario grid.** Require
   a nonempty bounded list of positive finite native spot values.
   Sort by value, reject exact duplicates and mark market spot by
   exact represented equality where present. Keep the recognized
   contractual state and held market inputs including the volatility
   strike fixed, and reuse the raw price, delta and diagnostic
   contracts for every row. Preserve independently available
   columns. For a qualified live fixed-share profile divide delta
   by effective deliverable shares on the same eligible balance.
   Keep a finite ratio as calculated. If regime classification is
   elected, classify a ratio in `[0,1]` as Busted below `.40`,
   Balanced from `.40` through `.80`, and EquitySensitive above
   `.80`. Variable-share, resolved and out-of-range cases have no
   qualified classification. These are presentation bands. Remove
   implicit scenario multiples, clipping and null-on-error.
   [Putnam illustration of convertible delta bands](https://www.putnam.com/static/pdf/improving-diversification-with-convertible-bonds.pdf),
   [current spot-ladder route](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/spot_ladder.rs).

10. **Publish the same checked result in Rust and its consumers.**
    Carry model/source identity, valuation and settlement phases,
    monetary amount basis, fit and bump controls, availability
    and numerical basis through direct and registered metrics.
    Bring typed methods, spot-ladder rows/table, serialization,
    schemas, attribution, Python and WASM facades and UI onto
    the same canonical valuation. Dated parity and premium
    need the qualifying cut. Unsupported columns may be absent
    on valid price rows. Distinguish them from missing required
    data, a failed price or sensitivity and proved or computed zero.
    Qualification must establish grid and bump sensitivity before
    interpreting finite differences as smooth-limit Greeks.
    [Canonical metric reporting](../valuations-framework/metric-orchestration.md).

## Observed behavior

### Choice and rationale

[Engine](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/engine.rs) and [metric registration](../../../../finstack-quant/valuations/fixed-income/src/convertible/metrics/mod.rs) compute finite differences and inverse prices on the lattice in [record 02](02-convertible-lattice.md). Quoted clean percent is converted to settlement dirty money; diagnostics preserve distinct price, conversion parity and bond-floor concepts. Method adoption and bump accuracy are open.

### Algorithm and controls

- Settlement advances optional business days on the coupon calendar and applies its business-day convention. Accrued uses linear cash coupon allocation with PIK excluded and without an ex-coupon rule. Dirty settlement value subtracts survival-weighted interim cash from as-of PV, then divides by the risk-free settlement DF. CleanPrice reports dirty minus accrued in money, not percent.
- Delta/Gamma use centered spot differences; Vega uses absolute vol bumps and is scaled to one volatility percentage point. If the down-vol repricing returns any Validation error, it falls back to the base price and the actual one-sided width. Rho is currency per basis point of risk-free curve bump. Theta is one calendar-day roll-forward PV minus current PV when time remaining exceeds `1/365.25`; direct Greek result carries theta, while registry installation lists Delta/Gamma/Vega/Rho.
- Dividend01 uses ±1 bp dividend yield with a zero floor on the down shock and rescales by actual width; no dividend identifier returns zero. Conversion01 bumps explicit ratio ±1%, or price ∓1% when only price exists, returning half the PV difference. If both ratio and price are populated, it changes ratio alone and can violate the validated ratio/price consistency contract.
- With a credit curve, CS01 delegates to shared hazard repricing. Without one, [CS01](../../../../finstack-quant/valuations/fixed-income/src/convertible/metrics/cs01.rs) creates a synthetic zero-hazard, zero-recovery curve and takes a one-sided +1 bp hazard change, parallel or triangular key-rate buckets. This changes conditional equity drift and survival on the whole defaultable-equity value, not only a separately discounted cash component, despite the fallback log description.
- [Implied volatility](../../../../finstack-quant/valuations/fixed-income/src/convertible/metrics/implied_vol.rs) searches the lattice-admissible lower bound starting at `1e-8`, doubling then bisecting validity for 32 iterations, with upper vol `3.0`. Brent uses `1e-12`, 100 iterations; repricing residual per notional must be ≤`1e-10`. Same-sign endpoint residuals error.
- [OAS](../../../../finstack-quant/valuations/fixed-income/src/convertible/metrics/oas.rs) solves a parallel **hazard-rate** shift when credit is present, otherwise a discount-curve shift. It reprices settlement dirty value on every trial. Lower bounds respect nonnegative hazard/forward-curve floors, upper bound `.50`, tolerance `1e-8`, 100 iterations. This output has different economic meanings in the two branches; it is not a cash-component z-spread.
- For a live convertible, the floor removes conversion/call/put exercise and rolls contractual payments with survival and recovery. Live preparation also resolves equity inputs. The resolved-settlement branch can return the pending cash/share PV as the floor, and the maturity branch discounts remaining cash without the live recovery treatment. Conversion value is instantaneous payoff; parity divides by face; premium compares price to conversion value. [Spot ladder](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/spot_ladder.rs) defaults to spot multiples `.25,.5,.75,1,1.25,1.5,2,3`; sorts/deduplicates positive supplied spots; computes delta/effective-ratio hedge ratio; labels below `.40` Busted, above `.80` EquitySensitive, otherwise Balanced. Mandatory-variable ratio regimes are absent rather than fabricated.

### Scope, limitations and alternatives

All monetary risks inherit lattice discontinuities and chosen bumps. Greek calculations use the native engine, while inverse-vol metric reprices through the metric context; arbitrary model overrides are not a separate accepted calibration contract. OAS/hazard and synthetic-CS01 terminology differs from the actual whole-value credit behavior described above. The one-sided Vega fallback catches validation more broadly than solely an invalid CRR probability. Spot-regime thresholds are presentation heuristics, not ratings or investment advice.

Analytic derivatives, smoothed Greeks, a finer adaptive lattice, or explicitly named hazard/discount spread metrics are alternatives needing separate approval. A generic cross-factor metric is also registered for spot-vol, spot-credit, rates-credit and credit-vol; the shared metric kernel controls mixed-bump arithmetic.

### Evidence

[Pricing tests](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/tests.rs) and colocated inverse-quote/spot-ladder tests provide finite-difference, round-trip and limiting-case evidence. [Convertible delta bands](../../../REFERENCES.md#convertible-delta-bands) supplies context for labels; [Brent](../../../REFERENCES.md#brent-1973) for inverse solves. No independent entire-surface Greek certificate was established.

## Independent arithmetic and qualification

- A holder PV of `107`, a retained coupon worth `5` at valuation
  and financing factor `.98` give buyer dirty `5100/49`,
  about `104.08163265`. Equal dates with the same retained claim
  give buyer dirty `102`. A cash coupon `6` with accrued
  fraction `5/6` on its first-ex date contributes `−1`.
- Twenty shares at spot `40` imply conversion value `800` and
  parity `.80` on `1000` denomination. Matched dirty PV
  `960` gives `.20` premium. An admitted terminal cash call
  of `95` can cap a par-`100` live claim below its stripped
  bond comparison of `100`.
- In the record 02 one-step example with `σ=.20`, the base PV is
  `106.70612169373337`. An OAS overlay of `.02` gives
  `104.60862573893942` with unchanged up probability
  `.5774931963561241`. An active issuer intensity move changes
  that transition and can produce positive signed holder risk.
- In a one-step, zero-rate/no-default/no-yield mandatory collar
  with spot `115`, par `100` and bounds `100,120`, target
  `102` has IV matches near `.08345265437` and `.74579091405`
  in disjoint brackets. In a separate one-step case, par and spot
  `100`, one share convertible only at stock `≥120` and zero
  carry, the PV jumps from `100` to about `109.09090909`
  at the trigger. A bracket can straddle target `105` without a fit.
- At the terminal conversion kink `max(100,S)` and spot `100`,
  symmetric spot steps `1` and `2` both give delta `.5`.
  Gamma changes from `1` to `.5`. A held-rate coupon crossing
  with opening `105`, closing `100` and cash `5` gives total
  theta zero. A mixed rectangle on `V=Sσ` at spot `100` gives
  `.01` in currency per 1% spot × one vol point.

Standalone exact and high-precision examples validate these
arithmetic cases. Independent price and fit qualification still
requires legal buyer/holder entitlement fixtures, surface-clock and
credit-replay evidence, convergence across actual exercise grids and
steps, market quote comparison, and native/registry/host agreement.
Repository pricing and host test suites were not run for this review.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
