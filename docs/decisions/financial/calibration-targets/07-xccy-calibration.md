# CALIBRATION-TARGETS-007 — Collateral-qualified foreign discount calibration

- **Decision status:** Accepted 2026-11-15 — complete cross-currency calibration package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent FX bridge, reset, identification, positive factor and dated diagnostic calculations verified. Production calibration, market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for collateral-qualified foreign discount calibration

1. **Fit one foreign discount curve on an explicit collateral basis.**
   Bind the ordered pair with foreign as base and domestic as quote
   and reporting currency. Require at least one eligible cross-currency
   basis swap. Admit cross-currency-only and mixed sets of eligible
   foreign rate anchors when they identify the curve. Route rates-only
   fits to the discount target. Require each rate anchor to qualify
   on the elected collateral basis. Hold domestic discount and
   independent index projections fixed. Use the candidate for overnight
   projection only through a qualified explicit link.
   [Accepted discount calibration](03-discount-calibration.md#accepted-decision-for-explicit-discount-calibration-and-delivered-curve-fit) ·
   [Accepted linked OIS roles](../calibration-framework/20-linked-ois-replay.md#accepted-decision-for-linked-single-curve-ois-replay-and-publication).

2. **Separate booked principal from valuation FX.** Freeze the exact
   quoted leg, signed Decimal bp, companion spread, principal exchanges,
   reset elections, dates and eligible fixings. Size and book initial
   principal with its authorized dated sizing spot. Elect valuation
   cash FX separately. A direct cash quote stays fixed across trials.
   With a later-value spot, hold its market quote fixed and recompute
   `X(V) = S(P) × D_dom(V,P) / D_for(V,P)` and eligible pending resets
   at their actual value dates. Preserve published fixings and booked
   amounts. Retire implicit FX precedence and cash-to-sizing substitution.
   [Accepted XCCY preparation](../calibration-framework/09-xccy-materialization.md#accepted-decision-for-cross-currency-swap-calibration-and-fx-authority) ·
   [ARRC interdealer conventions](https://www.newyorkfed.org/medialibrary/Microsites/arrc/files/2020/Recommendations_for_Interdealer_Cross-Currency_Swap_Market_Conventions.pdf).

3. **Fit the dated cash ledger and locate actual foreign support.**
   Discount eligible cash in native currency, translate foreign PV at
   valuation cash FX and divide signed raw domestic PV by the exact
   positive domestic normalization notional. Preserve native rate-anchor
   residuals, including elected futures points or cash scale.
   Set each cross-currency quote's pillar at the latest date the fitted
   foreign evaluator serves, including foreign cash, FX bridges, pending
   reset value dates and explicitly linked benchmark endpoints. Keep
   contractual far date and actual payment dates separate. A later
   domestic-only payment leaves the foreign pillar unchanged.
   [Accepted rate preparation and residuals](02-rate-preparation-and-residuals.md#accepted-decision-for-explicit-rate-preparation-and-native-fit-residuals).

4. **Require actual identification and elect the fit method.** Use one
   identifying quote per new knot for standalone Linear or LogLinear
   bootstrap. Prove that the necessary foreign queries are supported
   on the current prefix and invariant to later knots. Require explicit
   factor bounds, seeds, scans and work limits with qualified Brent or
   computed-zero evidence. Elect simultaneous fitting for coupled cases
   or shared-coordinate quotes, using a declared grid, explicit weights
   and bounds, and the accepted checked global solver. Require actual
   identification of free parameters even when quote count is sufficient.
   [Accepted bootstrap](../calibration-framework/10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing) ·
   [Accepted scalar search](../calibration-framework/11-scalar-root-search.md#accepted-decision-for-bounded-scalar-search-and-candidate-evidence) ·
   [Accepted weighted solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates).

5. **Use the same positive-factor contract in trials and delivery.**
   Require an explicit base date, source clock and context, interpolation
   and literal tails. Keep the exact `(0, 1)` anchor. Admit finite
   positive factors above one. Remove the hard `1.5` cap and the
   trial-versus-final shape discrepancy. Use explicit annual continuous
   zero-rate bounds, mapped to factor bounds for bootstrap or through
   checked solver coordinates for simultaneous fitting. Keep valuation
   cut separate from curve base. Reprice each consumed quote on the
   exact delivered market against its native tolerance, including
   zero-weight rows. Apply elected economic checks separately.
   [Accepted final curve validation](../calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication).

6. **Publish the foreign curve and preserve exact construction evidence.**
   Publish the evaluator, typed bindings, construction record and fit
   report atomically. Preserve original basis observations and actual
   collateral, FX, fixing and solver inputs. Automatic XCCY quote
   replay requires an explicitly qualified contract. Direct curve
   parameter risk remains a separate operation.
   [Accepted construction provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

7. **Remove the standalone basis-spread output.** Retire
   `BasisSpreadCurve` and its optional output ID. On explicit request,
   compare the delivered sources' dated log discount changes as
   `(ℓ_dom − ℓ_for)/α` over actual dates `T > V` and one explicit
   reporting clock. Each source evaluates the dates on its own
   clock. Report a signed annual continuous foreign-minus-domestic
   zero-rate difference. The original contractual basis quote retains
   its signed Decimal bp and specified leg.
   [Accepted dated zero-rate diagnostic](../core-market/basis-spreads.md#accepted-decision-source-derived-dated-zero-rate-differences).

## Independent calculation checks

- Receiving USD 130 and paying EUR 104 sized at 1.25 USD/EUR
  later-value spot gives zero initial exchange PV when the FX bridge
  is applied correctly. A domestic spot-date factor of `.96` and
  foreign factor of one give cash FX `1.20`. Using the sizing
  quote as cash FX instead gives `−USD 5.20`.
- With domestic spot/reset factors `.96,.90` and foreign
  spot/reset/payment factors `1,.975,.95`, pending reset FX is
  `1.30` and a foreign 100 payment has domestic PV `114`.
  Scaling all three foreign factors by `.90` changes cash FX
  from `1.20` to `4/3` while preserving the reset FX, initial
  exchange PV and translated payment PV.
- A simple one-period rate of `−3/8` and accrual one imply
  the exact positive factor `8/5`, above the observed hard cap.
- With cash FX `1.20` and one-year domestic and foreign factors
  `.98` and `.97`, the covered-interest-parity forward is
  `291/245`. The continuous foreign-minus-domestic zero-rate
  difference is about `102.565002` bp over one reporting year.

## Observed choice and inputs at the audit revision

Bootstrap a foreign discount curve from rate quotes and/or convention-built XCCY basis swaps while holding an existing domestic discount curve fixed. The optional BasisSpreadCurve is a derived **continuous foreign-minus-domestic zero spread**, not the dealer's coupon basis quotes. Only Bootstrap is implemented; GlobalSolve errors.

## Observed algorithm and selection

1. Require domestic discount and positive schema FX spot. Extract both rate and XCCY quote classes; either nonempty class suffices, despite a source comment suggesting rate anchors are mandatory. Curve clock defaults ACT/365F, interpolation LogLinear and extrapolation FlatForward. Schema FX input is T+0 cash FX in domestic units per foreign unit; the target does not convert a later-settlement screen spot to cash FX.
2. Rate quotes use candidate foreign curve as both discount and projection at `N=1,000,000`, with no step OIS override. XCCY builders receive domestic/foreign discount roles; missing per-quote spot is filled from schema spot, while an explicit positive quote spot takes precedence. Pair conventions select fixed-notional versus MtM resetting legs and schedules; [accepted XCCY construction](../calibration-framework/09-xccy-materialization.md#accepted-decision-for-cross-currency-swap-calibration-and-fx-authority) owns the replacement contract.
3. Merge prepared instruments by pillar time and bootstrap from `(0,1)`. Rate residual uses raw prepared calibration value; XCCY uses instrument raw PV, each divided by 1,000,000. Initial guess extrapolates the previous zero rate `exp(log(DFprev)*t/tprev)` when previous positive time exists; with the supplied origin seed the initial guess is 1. The no-previous-knot fallback queries domestic DF at numeric t. The target supplies no custom scan, so framework geometric scan and exhaustive bracketing apply.
4. Trial curve uses Raw validation allowing nonmonotone DFs without forward floor; accepted knots need `0<DF<=1.5` and finite values. Final build uses the **default DiscountCurve validation**, without the discount target's currency/nonmonotonicity override. The discount-specific success tolerance applies (default `1e-8` PV/N).
5. Optional spread output samples each quote's actual pillar date. Compute both relative DFs from calibration base date to that date, validate finite positivity, and set `b(t)=(-ln DF_foreign + ln DF_domestic)/t` on the foreign curve clock. Prepend `(0,0)`, use Linear interpolation and the requested extrapolation. Domestic curve may have its own base/day-count because date-relative discounting aligns the dates before dividing by the common t.

## Observed limits and alternatives

The trial/final shape policies differ: a negative-rate foreign curve can be numerically fitted yet fail final default validation. No currency-based relaxation or target rate bounds are used here, and the fixed 1.5 DF cap is separate from discount calibration bounds. Nonlocal requested interpolation receives a single bootstrap sweep, without discount target global polishing; final repricing can expose earlier-quote drift. The result carries no rate replay sidecar from this target.

Different quote spots can imply different notionals while market FX remains a separate context dependency. Curve zero-spread output combines all differences between calibrated foreign and domestic rates; it should not be interpreted as a pure funding basis without a separately agreed decomposition. Alternatives include direct zero-spread parameterization against a fixed foreign reference curve, coupled multi-currency solve, or benchmark-quote replay.

## Evidence

- Owning [xccy_basis.rs](../../../../finstack-quant/calibration/src/targets/xccy_basis.rs) and its quote-builder dependency were inspected.
- [XCCY integration tests](../../../../finstack-quant/calibration/tests/calibration/xccy_basis.rs) locate signed output, clock/base invariance, ordinary swap quotes and GlobalSolve rejection; inline quote acceptance uses the same production builder/pricer. These do not independently establish collateral/FX-reset market conventions.

Neither comments nor output type prove short-end anchors were supplied.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
