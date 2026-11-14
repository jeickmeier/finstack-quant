# VALUATIONS-OTHER-FIXED-INCOME-01: Bond-futures conversion factors, delivery carry and listed marks

- **Decision status:** Accepted 2026-11-14 — full checked bond-futures package, including the fixed-deliverable proxy and separate physical invoice.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Reconciliation pending. The observed entry points and kernels are described below.
- **Validation status:** Source, primary venue references and independent arithmetic reviewed. Contract, delivery, runtime, market and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked bond-futures carry, factors and listed marks

1. **Resolve the listed contract and its actual lifecycle.** Bind the
   venue and contract/month identity, authored or versioned venue
   terms, currency, position side, contract count, face per contract,
   quotation units and applicable point multiplier and ticks. Use
   points per `100` face in this profile. Check cash per point is
   face per contract divided by `100` on that denomination. Retain
   separate economic analytic size and venue clearing lot admission.
   Resolve actual last trading session, mark observations and
   publication, clearing dates, delivery eligibility window and any
   recognized intention, assignment and actual delivery dates/phases.
   Validate the chosen venue's chronology, including delivery before
   the last trade where allowed. Remove the universal strict
   last-trade/delivery/settlement ordering. Where a date alone cannot
   distinguish phases, require the session or authoritative state.
   Keep official rounding at its specified boundary.
   [Listed contracts and marks](../valuations-framework/listed-products.md),
   [CME delivery process and contract examples](https://www.cmegroup.com/content/dam/cmegroup/trading/interest-rates/files/us-treasury-futures-delivery-process.pdf)
   and [CBOT 10-year Treasury note rules](https://www.cmegroup.com/rulebook/CBOT/II/19.pdf).

2. **Keep the model a fixed-deliverable clean-price proxy.** Use
   `BondFutureCleanPriceProxy` only with an expressly selected
   eligible security, one authoritative factor and one explicit
   permitted delivery date and phase within the contract window.
   Require the modeled security and basket binding to agree, with
   positive quote denomination and compatible same-currency fixed
   principal and fixed coupon cash on the admitted deterministic hold
   claim. Reject unresolved financing/delivery rights and require
   a priceable transferable claim. A sole basket member, embedded
   bond, standard contract tenor or presence of a price does not
   elect the proxy inputs. The pricing leg uses the selected
   deliverable, regardless of whether it would be cheapest in the
   market. There is no basket optimization, CTD switching, delivery
   timing, quality or wild-card option value or futures/forward
   convexity adjustment in this model. Keep the proxy label and
   the selected security/date in every model result. Do not replace
   a missing or past analytical delivery date with `as_of`.
   [Bond-futures forward/clean framework](https://quant.opengamma.io/Bond-Futures-OpenGamma.pdf)
   and [CME on the delivery choices](https://www.cmegroup.com/content/dam/cmegroup/trading/interest-rates/files/us-treasury-futures-delivery-process.pdf).

3. **Bind conversion factors to the exact deliverable and edition.**
   Use the authoritative venue factor for the exact contract,
   delivery month and eligible security. For an explicitly authored
   analytical contract, use its explicitly authored factor with that
   provenance. Require finite strictly positive dimensionless factors
   and unambiguous security/contract identity. Retain the independent
   calculator only as an explicitly elected reproduction of the
   qualified CME fixed, level, semiannual noncallable Treasury
   profile. Require independently qualified eligibility and maturity,
   exact delivery month, product and rule edition. The official
   factor remains authoritative for an exchange contract. A computed
   factor is a checked diagnostic and cannot fill or override it
   automatically. No general foreign or callable factor formula.

   Let `n` be the whole years and `z_raw` the remaining complete
   calendar months from the first civil day of delivery month to
   legal maturity. In the qualified CME edition use `z=z_raw` for
   `ZT`, `Z3N`, `ZF`, and `z=3 floor(z_raw/3)` for `ZN`, `TN`,
   `TWE`, `ZB`, `UB`. Use the exact product code and exact calendar
   differences. Round annual decimal coupon `c` to the nearest
   `.00125` with upward ties. Set `g=.06`, `v=z` for `z≤6`
   and `v=z−6` otherwise, and `k=2n` for `z≤6` and `k=2n+1`
   otherwise. Then `a=(1+g/2)^(−v/6)`, `b=(c/2)(6−v)/6`,
   `u=(1+g/2)^(−k)`, `d=(c/g)(1−u)`, and the factor is
   `a(c/2+u+d)−b`. Apply four-decimal rounding once. Use the
   explicit half-up rule for positive exact ties in the standalone
   diagnostic, and report if the numerical result cannot resolve the
   rounding. Do not tie coupon or factor rounding to price ticks.
   Remove the generic standard-yield input, `≥6 years` granularity
   rule, earliest-call heuristic and use of unrounded coupon in this
   helper.
   [CME published factors](https://www.cmegroup.com/trading/interest-rates/us-treasury-futures-conversion-factor-lookup-tables.html),
   [CME formula and product examples](https://www.cmegroup.com/trading/interest-rates/files/Calculating_U.S.Treasury_Futures_Conversion_Factors.pdf).

4. **Carry a matched dirty buyer claim to the delivery phase.**
   Bind the authorized financing start and spot buyer settlement
   `S`, actual selected delivery `D≥S`, and the settlement rights
   at both dates. Use the authorized same-claim spot dirty amount
   `Q_S`: direct dirty, matched clean plus signed original-parent
   accrued, or the qualified BONDS-02 holder/buyer bridge. An
   undifferentiated holder PV is not spot dirty cash. Prepare
   the exact difference in claims between purchase and delivery.
   Value the retained signed cash on its authorized date and role
   as `Δ_ret` at `S`, including any independently owned unpaid
   coupon that remains with the seller. In the simple deterministic
   profile, ordinary interim retained coupons use `Σ C_j D_f(S,T_j)`.
   A later receivable needs a qualified source and coverage for its
   own valuation. Classify same-day cash by both phases and avoid
   the date-only `(S,D]` filter. Require a matched claim or an
   authorized exact retained-claim mapping. Bind exact positive
   finite native financing factor `D_f(S,D)` from an explicit
   same-currency repo/funding source. An approved common source
   may serve another role only with an explicit binding. No
   automatic futures/security-discount fallback and no separate
   repo day-count dependency where the input is dated factors.

   Use `Q_D=(Q_S−Δ_ret)/D_f(S,D)` for delivery dirty, then
   `C_D=Q_D−AI_D` for delivery clean, with `AI_D` the signed
   delivery-buyer accrued under the qualified original-parent law.
   If the quote has face `N>0` on the same denomination, the model
   points are `100 C_D/(N CF)`. Check native amounts and factors,
   positive represented delivery price on the admitted Treasury
   profile, and finite unrounded points. Keep valuation, financing
   and invoice accrued roles separate. The proxy models bond carry,
   not reinvestment or coupon cash of the futures position.
   [Bond settlement and signed accrued](../valuations-bonds/02-settlement-and-clocks.md),
   [repo roles](../valuations-rates/15-repo.md) and
   [forward clean value and conversion factor](https://quant.opengamma.io/Bond-Futures-OpenGamma.pdf).

5. **Use one explicit listed mark and reference; account for VM once.**
   At the live trading cut elect an eligible observed contract mark
   or the specifically requested live proxy. Use neither a trade
   price as a live mark nor a new model value as an observed quote.
   After the applicable trading cutoff, use the authoritative
   final mark where required. An unpublished required final mark
   makes the mark unavailable until eligible. No stale mark or
   reopened model substitute. Resolve lifecycle and mark branch before
   fetching deliverable, repo or factor inputs. `futures_price` and position marking
   share that authority. Use the actual fill for a new trade or
   last applied official clearing mark for a carried position.
   On the raw mark basis use
   `side × contracts × cash_per_point × (mark−reference)`.
   Report cumulative trade P&L separately. Apply venue-defined
   official clearing arithmetic or use authoritative cash for booked
   and pending variation. Keep cash as dated pending claims where
   admitted and exclude applied or settled money from a second
   mark on the same interval. A final mark is a fixed observation.
   Lifecycle closure requires that no relevant VM, delivery or other
   obligation remains. No date-only zero and no CTD dependency for
   a resolved observed or final mark request.
   [Canonical listed VM](../valuations-framework/listed-products.md)
   and [CME clearing amounts and rounding](https://www.cmegroup.com/clearing/files/CME-Money-Calculations-Futures-and-Options.pdf).

6. **Treat actual physical delivery as a separate typed exchange.**
   When delivery is assigned and qualified, identify the exact
   deliverable, face, parties' directions, intention/notice and
   delivery phases, exact contract factor, official invoice mark
   and payable accrued. Use the contract's authorized daily or
   final settlement observation for the actual intention phase.
   In the qualified CME procedure use the intention-day settlement
   for an intention before the last trading day, and the final
   settlement for an intention on or after it.
   For the qualified CME per-100 invoice, use `face/100 ×
   official_points × CF` for the clean component plus contractual
   delivery accrued on the delivered face. Apply the product's
   official per-lot component and total rounding, or carry the
   authoritative issued invoice. Add accrued once. Preserve long
   cash paid and delivered security received, with the opposite
   directions for the short. Generic cashflow export cannot
   represent the security leg. Use a typed exchange result for
   it and report separately admitted pending VM cash on its own
   basis. A clearing mark alone cannot value open physical delivery;
   require an explicit qualified value if the request includes the
   pending exchange. An analytical deliverable or forward price
   cannot supply an actual assignment, invoice or settlement state.
   [CME invoicing](https://www.cmegroup.com/content/dam/cmegroup/trading/interest-rates/files/us-treasury-futures-delivery-process.pdf)
   and [CBOT invoice rule and rounding](https://www.cmegroup.com/rulebook/CBOT/II/19.pdf).

7. **Keep the live point, curve and horizon risks on their own bases.**
   A live mark has signed point delta `side × contracts × cash_per_point`.
   Fixed final mark has no live point delta. For model curve DV01 and
   key-rate DV01, keep elected deliverable, delivery date/phase,
   factor, original mark/reference authority and amount fixed. Apply
   shared direct continuous-zero shocks to the actual active typed
   rate inputs used by the proxy. Reprice modeled spot and the
   financing/retained cash roles where responsive. An observed spot
   amount held fixed as a proxy input stays fixed. A held observed
   futures mark has no live curve response. Keep any admitted pending
   fixed cash discount exposure separately identified. A quoted
   mark does not silently clear itself to obtain model risk.
   For theta use the explicit FRAMEWORK-13 dates and cuts and
   dated conditioning of active held sources. Resolve required
   crossing marks and official or qualified scenario VM, and count
   future holder cash once. The deliverable's interim coupons are
   not futures cash. An active horizon crossing final marking or
   physical delivery needs a qualified scenario covering both sides
   and included obligations. Remove automatic horizon clipping,
   stale mark aging and the blanket rejection based only on the
   first date of the delivery window.
   [Shared rate risk](../valuations-framework/finite-difference-rates.md),
   [dated horizon theta](../valuations-framework/carry-theta-breakeven.md)
   and [listed margining](../valuations-framework/listed-products.md).

8. **Use canonical Rust preparation and disclose the result basis.**
   Preserve exact contract, factor provenance and date/session,
   mark authority, signed size, currency, reference, clearing and
   delivery state, active curves, rounding and risk controls in
   applicable results. A requested `conversion_factor` needs an
   identified security and factor, without a full spot carry
   calculation. Distinguish observed mark, proxy output and issued
   cash. Preserve independently available outputs and propagate
   invalid requests, missing required active data, invalid state
   and failed valuation or numerical steps. Reconcile direct and
   registry pricing, results and risk, table and wire declarations,
   Python, WASM and UI under one authority. Retain rates-futures
   benchmark and convexity contracts with their owners.
   [Metric result basis](../valuations-framework/metric-orchestration.md)
   and [interest-rate futures](../valuations-rates/10-rate-futures.md).

## Observed behavior

### Choice and rationale

The registered `BondFutureCleanPriceProxy` uses a caller-selected cheapest-to-deliver bond and its supplied basket conversion factor. It computes a carry-adjusted CTD forward clean price; it does not optimize the delivery basket. The method name acknowledges this proxy scope. [Types](../../../../finstack-quant/valuations/fixed-income/src/bond_future/types.rs) and [pricer](../../../../finstack-quant/valuations/fixed-income/src/bond_future/pricer.rs) own selection and formulas; use as an exchange-complete futures model is not adopted.

### Algorithm

1. Resolve CTD identifier by explicit `ctd_bond_id`, embedded CTD identifier, or the sole basket member. Multiple unresolved deliverables error. The embedded bond must match that identifier and have a positive finite basket conversion factor.
2. `calculate_conversion_factor` is a separate helper, not an automatic replacement for the supplied basket factor. It obtains fixed coupon `c` and the earliest call start or maturity. Measure whole months from the first day of delivery month; long contracts (`standard_maturity_years≥6`) round residual months down to a quarter, short contracts retain whole months. With notional coupon `g`, use `a=(1+g/2)^(−v/6)`, `b=(c/2)(6−v)/6`, `d=(c/g)(1−C)`, where `C=(1+g/2)^(−2n)` or exponent `−(2n+1)` according to residual-month branch. Factor is `a(c/2+C+d)−b`, rounded half-away to four decimals.
3. Set delivery date to `max(delivery_start,as_of)`. Price the embedded bond for spot dirty value; subtract the financing-curve PV of interim cashflows in `(as_of,delivery]`, divide by financing DF to delivery, and subtract delivery accrued. Divide clean money by bond notional, multiply by 100, and divide by conversion factor.
4. Financing uses `repo_curve_id` if supplied, otherwise the future's discount curve. The CTD's own pricing curves still determine spot value.
5. Listed terms convert model points into signed contracts × multiplier P&L relative to entry price and handle final-settlement lifecycle. `FuturesPrice` instead calls `mark_price`, which can follow a supplied live mark or official final settlement. `ConversionFactor` reports the resolved basket factor. Parallel and bucketed DV01 use shared repricing.

### Scope, controls and conventions

Fixed-rate deliverables are required for the CME-style factor helper. Factors are dimensionless; futures prices are points per 100 face; nominal exposure is contracts × multiplier × 100. Date validation requires last trading before delivery start before settlement. Contract repo day count must be supported even though dated financing DFs determine carry. The family registers this proxy only; no runtime gate selects a delivery-option model.

### Limitations, failure and alternatives

No stochastic CTD switching, quality option, timing option, wild-card option or futures/forward convexity adjustment is computed. Conversion factor conventions cannot be assumed identical for every exchange merely because a contract specification fits this function. The price path resolves the embedded CTD before listed lifecycle pricing, even where final settlement could otherwise avoid model inputs. Physical delivery cannot be represented by the generic cashflow exporter and is rejected. An exchange-specific basket optimization or stochastic delivery model is an unimplemented alternative; quoted marks remain distinct from fair-model price.

### Evidence

[Bond-future tests](../../../../finstack-quant/valuations/fixed-income/tests/bond_future/mod.rs) and colocated tests in [types](../../../../finstack-quant/valuations/fixed-income/src/bond_future/types.rs) cover carry, factors, lifecycle and validation examples; these were not rerun. [Hull](../../../REFERENCES.md#hull-options-futures) and [fixed-income handbook](../../../REFERENCES.md#fabozzi-fixed-income-handbook) are context. No independent delivery-option benchmark was established.

Additional live evidence includes [mark, theta and risk metrics](../../../../finstack-quant/valuations/fixed-income/src/bond_future/metrics.rs),
[Python pricing fixtures](../../../../finstack-quant-py/tests/golden/test_pricing_bond_future.py)
and the [instrument schema consumed in the UI](../../../../finstack-quant-ui/src/generated/schemas/bond_future.json).

## Independent arithmetic and qualification

- For face `100`, spot dirty `102`, retained coupon `3` discounted
  at `.99`, delivery factor `.98`, delivery accrued `1.50` and
  conversion factor `.95`, forward dirty is `9903/98`, clean is
  `4878/49` and proxy points are `97560/931`, about `104.790548`.
  Omitting the retained receipt gives about `107.980666` points.
- The CME formula reproduces eight published examples: `ZT .9835`,
  `Z3N .9637`, `ZF .9135`, `ZN .9003`, `TN .8904`, `TWE .8172`,
  `ZB .8514`, and `UB .5276`. A synthetic quarter-rule case with
  `n=6,z=6` and `.040625` coupon rounds the coupon to `.04125`
  and gives `.9003`. Skipping coupon rounding gives `.8970`.
  [CME worked examples](https://www.cmegroup.com/trading/interest-rates/files/Calculating_U.S.Treasury_Futures_Conversion_Factors.pdf).
- On the CBOT example of one `100,000`-face lot with official
  `100+25.5/32` points and `CF=.9633`, the raw invoice clean
  component is `97,097.6296875`, rounding to `97,097.63` before
  adding contractual accrued. The clean component is distinct
  from the mark change since prior clearing. [CBOT rule example](https://www.cmegroup.com/rulebook/CBOT/II/19.pdf).
- A long three-lot analytical position at `1,000` per point,
  with fill `100`, prior applied clearing `101` and current mark
  `102`, has new raw variation `3,000` and cumulative trade P&L
  `6,000`. Once the current interval is applied, its mark difference
  is zero on that reference. Any unpaid clearing cash remains on
  its admitted cash basis.

Exact rational carry and variation checks and 70-digit factor and
invoice checks reproduce these examples.

Qualification requires contract/edition and actual exchange-session
fixtures, delivery before the trading cutoff, intention/day notice and
final mark authority, official rounding and known physical transfers,
independent repo/price and retained-cash cases, wrong or stale factors,
same-day cash and ex-coupon cuts, selected mark versus proxy risk, and
Rust/host and result-metadata agreement. Arithmetic checks establish
these examples and do not qualify live market values or venue coverage.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
