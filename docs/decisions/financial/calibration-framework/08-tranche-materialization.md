# CALIBRATION-FRAMEWORK-008: Index tranche notional and schedule normalization

- Decision status: **Accepted 2026-11-15 — complete index tranche terms, date roles, quote normalization, shared fit and provenance package**.
- Implementation status: **Observed.** The live builder requires a named standard or bespoke date regime. Reconciliation to the complete accepted package is pending.
- Validation status: **Independent tranche geometry, dated cash and quote-residual checks completed.** Production valuation, calibration, term-sheet repricing, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for index tranche calibration and quote settlement

1. **Bind one exact tranche contract and original notional.** Require the
   index, series, version, currency and qualified dated terms or
   complete bespoke terms. Use the protection-buyer direction. Admit
   exact original-pool fractions `0 ≤ a < d ≤ 1`. With positive
   exact original pool Money `N_pool`, compute original tranche
   notional `N₀ = N_pool × (d − a)` once in checked Money arithmetic.
   Convert strikes to literal percent once for pricing and correlation.
   Admit any positive width the arithmetic can resolve.
   [Accepted tranche loss geometry](../models-and-valuations-credit/17-tranche-loss-distribution.md#accepted-decision-for-checked-finite-pool-tranche-losses) ·
   [Accepted base-correlation sources](../core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks).

2. **Resolve tranche dates under the elected terms.** Preserve trade
   date, valuation close, index effective date, first accrual anchor,
   first covered day, literal contractual maturity, and the dates of
   premium, protection and quote settlement. Use the elected standard
   quarterly-twentieth schedule with ACT/360 and an explicit Following
   calendar, or authored bespoke accrual slices, payment dates and
   day-count context. Both cover maturity `M` through its close and
   end final accrual at `M+1` exclusive. Standard final payment rolls
   Following from `M`. Pay modeled protection on its civil loss day
   or after an explicit positive business-day lag on its own calendar.
   Quote settlement uses its own rule. Maturity and cash dates come
   from these terms in both targets. Remove single-name tenor
   derivation, calendar and start-date defaults, and the use of a
   curve base as valuation date.
   [Accepted tranche cashflows](../models-and-valuations-credit/18-tranche-projection.md#accepted-decision-for-checked-tranche-cashflows-and-settlement).

3. **Reconcile recognized losses and recoveries to live outstanding.**
   Require authoritative historical loss, recovered principal and live
   exposure on the original pool basis, or explicit unseasoned state.
   Loss reduces the tranche from the bottom and alone can generate
   protection cash. Recovery erosion reduces it from the top. Use dated
   opening outstanding for elapsed accrual. Preserve booked upfront,
   earned coupons and confirmed historical claims with their own
   dated entitlements outside the live quote residual.
   [Accepted original-pool state](../models-and-valuations-credit/17-tranche-loss-distribution.md#accepted-decision-for-checked-finite-pool-tranche-losses).

4. **Require the upfront representation and settlement date.** Accept a
   signed clean fraction `u` or actual-settlement fraction `q` of
   original tranche notional. With annual decimal coupon `c`
   admitted from bp and elapsed unit-coupon amount `H` from the
   same live stream, convert clean to signed buyer cash
   `C = N₀u − cH`. Actual-cash quotes give `C = N₀q` directly.
   Positive cash is paid by the buyer and negative is received.
   Preserve zero clean quotes, which can settle an accrued rebate.
   Apply only qualified coupon and quote bounds. External point or
   surviving-notional quotes require their exact basis and a qualified
   conversion to the accepted input.
   [ICE tranche quotation terms](https://www.ice.com/products/34933772/Markit-CDX-North-AmericanInvestment-Grade-Index-Tranche).

5. **Fit one checked live buyer residual.** Both targets use
   `(P − cA_dirty − BC) / N₀`, where `P` is protection PV,
   `A_dirty` is the dirty unit-coupon annuity on the live stream
   and `B` is the factor to quote settlement. Apply the explicit
   same-day cash choice and preserve contractual cash rounding
   before raw PV evaluation. Use the same prepared contract
   in base-correlation and Student-t calibration. Base correlation
   varies eligible correlation parameters for the exact pool,
   maturity and selected copula. Student-t varies only degrees
   of freedom on a qualified fixed correlation basis. Remove the
   reciprocal-width shortcut that manufactures unit tranche notional.
   Require genuine sensitivity to the fitted parameter. The full
   0–100% layer with fixed marginals cannot identify dependence.
   The targets own solver controls and fit acceptance.
   [Accepted base-correlation target](../calibration-targets/15-base-correlation.md#accepted-decision-for-identified-base-correlation-bootstrap-and-atomic-publication) ·
   [Accepted Student-t target](../calibration-targets/16-student-t.md#accepted-decision-for-identified-single-tranche-student-t-calibration).

6. **Freeze active dependencies and exact provenance.** Bind the
   exact discount source and each active hazard role. Use the shared
   hazard for a homogeneous pool or the live member hazards for a
   named pool. Preserve payoff recovery separately from hazard
   construction metadata. Retain the exact quote and terms, resolved
   dates, pool state, model settings and source bindings through trial,
   final repricing and replay. Publish the fitted correlation and
   rebound index atomically. Use Rust admission across Python, WASM
   and schemas.
   [Accepted credit-pool composition](../core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts) ·
   [Accepted calibration provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

## Independent calculation checks

- S&P's Dean Foods example for CDX.NA.HY Series 33 has one 1%-weighted
  default and 9.25% recovery. On 100 million original pool notional,
  equity outstanding falls from 15 million to 14,092,500 through
  loss and super-senior outstanding falls from 65 million to
  64,907,500 through recovery erosion.
  [S&P CDS Indices Primer](https://www.spglobal.com/spdji/en/documents/campaigns/CDS-Indices-Primer---2021.pdf).
- A synthetic 3%–7% layer on 100 million has 4 million original
  notional and 2.5 million outstanding after historical erosion.
  Ten days at the original balance and twenty at the remaining
  balance give elapsed unit-coupon accrual of 250,000 on ACT/360.
  At 500 bp and −2.5% clean upfront, buyer cash is −112,500.
  With settlement factor 0.99 it offsets dirty PV −111,375.
  Using outstanding as the quote denominator introduces a
  −37,125 native PV residual.
- In a synthetic June 18, 2026 trade, separate settlement calendars
  give June 22 payment for a June 20 loss, June 23 for the first
  coupon and June 24 for upfront. December 20 remains the maturity
  and last covered day. Accrual ends December 21 exclusive and the
  final coupon settles December 22.

## Observed choice and rationale at the audit revision

The build context notional is a pool-scale amount. Construction converts it to tranche notional and retains separate quote conventions and schedule overrides. This avoids treating the same upfront fraction as a pool-notional payment.

## Algorithm and conventions

Validate attachment/detachment and quote fields; resolve CDS registry conventions and spot. Set `N_tranche = N_context × (detachment − attachment)`. Inputs attachment/detachment are fractions; `CdsTrancheParams` receives percentages by multiplying both by 100. Upfront, when nonzero, is `N_tranche × upfront_pct` at spot; zero upfront is represented by absence. Coupon is bp, realized loss is initialized to zero, and the side is Pay premium/receive protection.

CdsImm mode anchors effective date at the quarterly CDS accrual start and maturity at the next standard quarterly date (preserving an already-standard date). None mode uses spot effective date and maturity adjusted using the registry calendar/BDC. Other roll rules fail. Schedule frequency/day count/calendar/BDC can override the registry; stub remains registry-supplied, EOM false, payment lag zero and accrual-date adjustment false. Build with explicit discount/credit IDs, then assign effective date and upfront.

`CdsTrancheBuildOverrides::default` selects CdsImm, while envelope base-correlation params use `RollRule::default`, which is **None**; actual caller values therefore select different effective/maturity rules. Target-specific loss-model and base-correlation solving are covered in [calibration targets](../calibration-targets/README.md).

## Current source reconciliation and evidence limits

The live builder requires standard or bespoke after
[O7](07-cds-materialization.md#accepted-decision-named-cds-contractual-date-regimes).
Standard maturity is a literal quarterly roll date. Bespoke start and
rolled maturity still depend on registry dates before schedule overrides.
Base correlation still derives expected maturity from a target date.
Student-t still imposes standard dates and scales notional reciprocally
by tranche width. Same-kernel checks establish consistency. Dated
market terms, source admission and external repricing remain open.

## Source and validation evidence

- Implementation: [build/cds_tranche.rs](../../../../finstack-quant/calibration/src/build/cds_tranche.rs) — width scaling, fraction/percent conversion and schedule override precedence.
- Implementation: [api/schema.rs](../../../../finstack-quant/calibration/src/api/schema.rs) — base-correlation schedule parameters.
- Test evidence: [src/build/cds_tranche.rs](../../../../finstack-quant/calibration/src/build/cds_tranche.rs) — notional normalization, roll and override regressions located; inspected or located, not executed.
- Reference context: [ISDA 2014 credit definitions](../../../REFERENCES.md#isda-2014-credit-definitions) — contract convention context; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
