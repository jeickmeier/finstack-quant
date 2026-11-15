# CALIBRATION-TARGETS-009 — Equity spot and carry resolution for smile calibration

- **Decision status:** Accepted 2026-11-15 — complete equity quote, spot, carry and dated forward package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent fixing-phase, delayed-payment, carry-clock and matched-quote checks verified. Production calibration, market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for explicit equity spot carry and dated smile forwards

1. **Select exact live European option quotes.** Validate every
   supplied quote. Consume a nonempty explicit set of quote IDs
   with compatible underlying, share or index-point basis, currency,
   exercise and settlement terms, volatility convention and clock.
   Reject selected expired, mismatched or unsupported rows.
   Preserve call or put side separately from bid, mid or ask.
   Parity-compatible calls and puts at the same strike share the
   model prediction and count as one strike. Each keeps its own
   fit row.
   [Accepted quote admission](01-quote-contracts.md#accepted-decision-for-exact-typed-calibration-quotes-and-native-units).

2. **Bind one dated spot authority.** Elect a qualified authored
   price or exact market quote binding. Require finite positive
   cash-equivalent spot on the matching security, share or index-point
   basis, native currency, settlement basis and information cutoff.
   Retire override precedence, ticker-derived lookups and currency
   stripping. Currency conversion and nonstandard share or settlement
   bases retain separate qualification.
   [Accepted equity spot and carry](../valuations-equity-and-composition/02-spot-and-carry.md#accepted-decision-for-checked-cash-equity-valuation-and-dividend-carry).

3. **Elect one carry policy and one dated funding source.** Admit
   explicit no distributions, finite signed continuous annual yield,
   or an exact qualified cash dividend schedule. Prepaid spot is
   `S` for no distributions and `S exp(−qτ)` for continuous
   yield on the ACT/365F carry clock. Zero yield is a valid literal
   election. Combined cash and residual yield and directly quoted
   forwards require separately qualified models. Require an explicit
   funding source for SABR and SVI. An elected unit-factor source
   can represent zero funding. Retire the implicit SVI zero rate.
   Compute `F = prepaid / D(V,E)` at actual underlying fixing `E`.
   Keep carry, funding and volatility clocks distinct from the option
   payment date.
   [Accepted European fixing and payment rules](../valuations-equity-and-composition/07-black-tree-and-direct-greeks.md#accepted-decision-for-checked-live-equity-option-pricing-and-greeks).

4. **Use ex-date entitlement and actual cash payment dates.** Require
   complete selected schedule coverage and applicable vintage,
   exact event IDs, gross per-share Money, and matching share basis
   and currency. Include rights acquired with spot and retained
   at the actual fixing phase. Compute prepaid spot as
   `S − Σ dᵢ D(V,Pᵢ)` using each event's actual payment date
   `Pᵢ`, including payment after fixing. Keep distinct same-date
   events, reject unsupported entitlement regimes, and require
   strictly positive prepaid spot and forward for smile fitting.
   [Accepted dividend events](../core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates) ·
   [FINRA ex-dividend timing rules](https://www.finra.org/rules-guidance/rulebooks/finra-rules/11140).

5. **Freeze dated forward inputs through fit and publication.** Resolve
   actual fixing dates and phases for admitted quotes and requested
   output and validation slices. Reject incompatible slices that map
   to one represented coordinate. Preserve quote IDs and authored
   order, spot and carry authority, fixing and settlement terms,
   funding binding, used dividend events and resolved forwards
   through fitting, reporting and restoration. Use the same Rust
   resolution across existing interfaces. Records 010–012 own
   materialization, model-specific volatility domains, minimum
   distinct strikes and optimizer choices.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Surface materialization](10-total-variance-materialization.md) ·
   [Equity SABR](11-equity-sabr.md) · [Equity SVI](12-equity-svi.md).

## Independent calculation checks

- With spot `100`, a fixing factor of `.98` and a retained
  dividend of `4` paid later at factor `.95`, the forward is
  `4810/49`, about `98.1633`. Discounting on the ex-date at
  `.98` instead gives `4804/49`, about `98.0408`. Fixing
  before the dividend detaches gives `5000/49`, about `102.0408`.
- With spot `100`, fixing factor `.98` and annual continuous
  yield `.04` over 182 actual days, the ACT/365F carry forward
  is about `100.02575350`. Substituting ACT/360 for the carry clock
  gives about `99.99804853`.
- Parity-compatible quotes of 20% and 21% at one strike have a
  minimum worst annual volatility residual of `.005`. A
  tolerance of `.002` on each quote fails.

## Observed choice and rationale at the audit revision

Equity SABR/SVI calibration uses forwards derived from explicit spot and carry. Missing dividends are not silently assumed zero. A continuous yield takes precedence over a matching cash-dividend schedule; the latter subtracts present values from prepaid spot. Forward positivity is required by the lognormal surface models.

## Observed inputs and algorithm

1. Select only `OptionVol` quotes whose underlying exactly equals the requested ticker; other underlying/class/option kinds are excluded. Require at least one matching quote before expiry filtering. Both call and put quoted vols fit the same smile; `option_type` is not a separate model input.
2. Spot override takes precedence; otherwise read the scalar under ticker. A Money price contributes its amount, and Unitless is also accepted. Downstream forward construction requires finite positive spot.
3. Dividend-yield override takes precedence over unitless `<ticker>-DIVYIELD`. A monetary scalar at that key errors; lookup absence yields no continuous yield. Any supplied yield must be finite but may be negative. With a yield, ignore all dividend schedules and use `prepaid(T)=S*exp(-qT)`.
4. Otherwise find a unique schedule whose underlying is ticker, or whose ID is ticker or `<ticker>-DIVS`. None or multiple matches error. Ignore events on/before surface base date; reject future stock/yield events. For cash dividends, retain ACT/365F ex-date time and `cash_amount*D(base,ex_date)`; sum only events with `t_event<=T`. Then `prepaid(T)=S-sum(PVcash)`.
5. Convert ACT/365F model time to the discount curve's own base/day-count through `model_time_on_curve`, and use relative `D(base,T)`. Return `F(T)=prepaid(T)/D(base,T)`, rejecting nonfinite/nonpositive discount factors or forwards. Cash is discounted on the event/ex date; there is no separate dividend payment date in this source representation.
6. Equity SABR requires a discount curve. SVI optionally omits it, but then still requires an explicit/market continuous yield and uses `F(T)=S*exp(-qT)`, i.e. zero discount rate. Cash schedules are not supported in that no-discount branch.

## Observed limits, alternatives and evidence

Currency is discarded when extracting Money spot/cash amounts; this helper does not prove that spot, dividends and discounting use one currency. An explicit yield even of zero suppresses the cash schedule. Ex-date inclusion at expiry is an input convention, not a dividend-pay-date model. Time conversion and relative discounting avoid blindly applying a curve's numeric t to a different base.

Alternatives are explicit forward quotes/price curves, foreign-currency cash conversion, dividend payment dates, and separate stock/yield event models. These need distinct contracts rather than silently mixing carry inputs.

- Owning [util.rs](../../../../finstack-quant/calibration/src/targets/util.rs), with calls in [SABR](../../../../finstack-quant/calibration/src/targets/vol.rs) and [SVI](../../../../finstack-quant/calibration/src/targets/svi.rs), is inspected.
- Inspected inline tests use hand-computed continuous-yield/cash formulas and change discount base/ACT360 versus ACT365F clocks; a missing-carry rejection is also inspected. These establish intended algebra and invariance as test evidence, without a run.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
