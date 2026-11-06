# CALIBRATION-TARGETS-009 — Equity spot and carry resolution for smile calibration

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and rationale

Equity SABR/SVI calibration uses forwards derived from explicit spot and carry. Missing dividends are not silently assumed zero. A continuous yield takes precedence over a matching cash-dividend schedule; the latter subtracts present values from prepaid spot. Forward positivity is required by the lognormal surface models.

## Inputs and algorithm

1. Select only `OptionVol` quotes whose underlying exactly equals the requested ticker; other underlying/class/option kinds are excluded. Require at least one matching quote before expiry filtering. Both call and put quoted vols fit the same smile; `option_type` is not a separate model input.
2. Spot override takes precedence; otherwise read the scalar under ticker. A Money price contributes its amount, and Unitless is also accepted. Downstream forward construction requires finite positive spot.
3. Dividend-yield override takes precedence over unitless `<ticker>-DIVYIELD`. A monetary scalar at that key errors; lookup absence yields no continuous yield. Any supplied yield must be finite but may be negative. With a yield, ignore all dividend schedules and use `prepaid(T)=S*exp(-qT)`.
4. Otherwise find a unique schedule whose underlying is ticker, or whose ID is ticker or `<ticker>-DIVS`. None or multiple matches error. Ignore events on/before surface base date; reject future stock/yield events. For cash dividends, retain ACT/365F ex-date time and `cash_amount*D(base,ex_date)`; sum only events with `t_event<=T`. Then `prepaid(T)=S-sum(PVcash)`.
5. Convert ACT/365F model time to the discount curve's own base/day-count through `model_time_on_curve`, and use relative `D(base,T)`. Return `F(T)=prepaid(T)/D(base,T)`, rejecting nonfinite/nonpositive discount factors or forwards. Cash is discounted on the event/ex date; there is no separate dividend payment date in this source representation.
6. Equity SABR requires a discount curve. SVI optionally omits it, but then still requires an explicit/market continuous yield and uses `F(T)=S*exp(-qT)`, i.e. zero discount rate. Cash schedules are not supported in that no-discount branch.

## Limits, alternatives and evidence

Currency is discarded when extracting Money spot/cash amounts; this helper does not prove that spot, dividends and discounting use one currency. An explicit yield even of zero suppresses the cash schedule. Ex-date inclusion at expiry is an input convention, not a dividend-pay-date model. Time conversion and relative discounting avoid blindly applying a curve's numeric t to a different base.

Alternatives are explicit forward quotes/price curves, foreign-currency cash conversion, dividend payment dates, and separate stock/yield event models. These need distinct contracts rather than silently mixing carry inputs.

- Owning [util.rs](../../../../finstack-quant/calibration/src/targets/util.rs), with calls in [SABR](../../../../finstack-quant/calibration/src/targets/vol.rs) and [SVI](../../../../finstack-quant/calibration/src/targets/svi.rs), is inspected.
- Inspected inline tests use hand-computed continuous-yield/cash formulas and change discount base/ACT360 versus ACT365F clocks; a missing-carry rejection is also inspected. These establish intended algebra and invariance as test evidence, without a run.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
