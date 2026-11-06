# FX, commodity and exotic valuation methodologies

Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

Scope: **113 assigned production-source files**, classified as 84 algorithm-bearing, 28 support, 1 test-only and 0 generated. The manifest includes inline-test-containing implementation files as algorithm-bearing and records actual pricing, fixing, exercise, approximation, numerical control and risk decisions. It is not a list of supported product labels. [coverage.json](coverage.json) maps every assigned source exactly once to the records below.

All **22 records are Draft**: observed implementation, adoption pending. Runtime source/tests/fixtures and financial choices are unchanged. Validation consists of implementation reads, located/inspected test assertions and documentation coverage/link checks; numerical tests, builds and external vendor comparisons were **not executed**. Existing [ADR 0027](../valuations-equity-and-composition/0027-vanilla-option-model-selection.md) covers baseline option model selection, while these records describe additional formulations, parameters, historical state, exercise algorithms and risk behavior. Dependencies delegated to shared models/core are linked and audited in their respective slices; no separate implementation is inferred from a macro or registry key.

In formulas, `DF` is a date-relative discount factor, `N` notional, `Q` quantity, and rates/volatilities are annual decimals unless a record explicitly states otherwise. A model key is not necessarily the kernel name: FX vanilla and commodity Kirk share `Black76` registry keys, and commodity geometric Asians share an `AsianTurnbullWakeman` instrument key.

## Governing and overview ADRs

These ADRs retain their stated scope and decision status. They are separate
from the detailed algorithm records and source counts below.

| ADR | Decision status | Scope |
| --- | --- | --- |
| [0027](../valuations-equity-and-composition/0027-vanilla-option-model-selection.md) | Draft | Cross-family selection overview for vanilla equity and FX options |

## Algorithm inventory

Open adoption, implementation and validation work is owned by the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation |
|---|---|---|---|---|
| FX cash, forward and swap value | Quote receipt; CIP DF ratios; signed near/far exchanges | Discounting; market/quoted spot; missing contract rates re-mark at par | [VALUATIONS-FX-AND-EXOTICS-FX-DELIVERY-CIP](fx-delivery-cip.md) | Observed source; evidence located/inspected, not executed |
| NDF settlement and fixing transition | Reciprocal quote normalization; maturity-forward projection; observed-fixing cashflow | Observed fixing → quoted forward → two-curve CIP | [VALUATIONS-FX-AND-EXOTICS-NDF-FIXING-SETTLEMENT](ndf-fixing-settlement.md) | Observed source; evidence located/inspected, not executed |
| FX listed futures/options | CIP model mark; listed MTM; shared Black/normal futures-option kernel | Common listed terms and macro, not FX vanilla spot payoff | [VALUATIONS-FX-AND-EXOTICS-FX-LISTED-CONTRACTS](fx-listed-contracts.md) | Observed source; evidence located/inspected, not executed |
| FX vanilla price and quote Greeks | GK/Black equivalent; inverse vol solver; four delta conventions | Black76 key; active strike vol; analytic Greeks | [VALUATIONS-FX-AND-EXOTICS-FX-VANILLA-DELTA](fx-vanilla-delta.md) | Observed source; evidence located/inspected, not executed |
| FX digitals | Cash/asset digital CDF kernels and analytic derivatives | Black76 key; strict expiry ITM rule | [VALUATIONS-FX-AND-EXOTICS-FX-DIGITAL](fx-digital.md) | Observed source; evidence located/inspected, not executed |
| FX barriers | Continuous analytic plus explicit GBM monitoring grids | Continuous → analytic; discrete → MC; explicit overrides | [VALUATIONS-FX-AND-EXOTICS-FX-BARRIERS](fx-barriers.md) | Observed source; evidence located/inspected, not executed |
| FX touches | Discounted hit-time transform and expiry hit/no-hit probability | Analytic, started continuous window only | [VALUATIONS-FX-AND-EXOTICS-FX-TOUCH](fx-touch.md) | Observed source; evidence located/inspected, not executed |
| FX variance swaps | Exact realized estimator plus smile-replicated interval squared-return forecast | Close-to-close forecasting; OHLC only fully observed | [VALUATIONS-FX-AND-EXOTICS-FX-VARIANCE](fx-variance.md) | Observed source; evidence located/inspected, not executed |
| Quanto options | Domestic-measure adjusted asset drift; fixed FX conversion | QuantoBS; ATM-forward FX vol | [VALUATIONS-FX-AND-EXOTICS-QUANTO-DRIFT](quanto-drift.md) | Observed source; evidence located/inspected, not executed |
| Commodity delivery and listed contracts | Discounted marked delivery; listed single/average fixings; futures-option macro | Quoted forward/curve/spot carry; listed mark terms | [VALUATIONS-FX-AND-EXOTICS-COMMODITY-DELIVERY-LISTED](commodity-delivery-listed.md) | Observed source; evidence located/inspected, not executed |
| Commodity spot options | Black-76; Leisen–Reimer exercise; forward-pinned Schwartz–Smith MC | European Black; American/Bermudan tree; explicit two-factor MC | [VALUATIONS-FX-AND-EXOTICS-COMMODITY-SPOT-OPTION](commodity-spot-option.md) | Observed source; evidence located/inspected, not executed |
| Commodity averaging swap | Equal business-day average; exact past fixings; fixed quantity per period | Discounting; optional calendar/lag; midpoint empty-window fallback | [VALUATIONS-FX-AND-EXOTICS-COMMODITY-AVERAGE-SWAP](commodity-average-swap.md) | Observed source; evidence located/inspected, not executed |
| Commodity swaptions | DF-weighted average swap forward and annuity Black formula | Black76 with one swap-rate vol | [VALUATIONS-FX-AND-EXOTICS-COMMODITY-SWAPTION](commodity-swaption.md) | Observed source; evidence located/inspected, not executed |
| Commodity Asians | Exact geometric lognormal; arithmetic two-moment matching; seasoned effective strike | Averaging-method branch under AsianTurnbullWakeman key | [VALUATIONS-FX-AND-EXOTICS-COMMODITY-ASIAN](commodity-asian.md) | Observed source; evidence located/inspected, not executed |
| Commodity spread options | Kirk effective weight/variance; leg delta/vega | Black76 key invoking Kirk; invalid domain errors | [VALUATIONS-FX-AND-EXOTICS-COMMODITY-SPREAD](commodity-spread.md) | Observed source; evidence located/inspected, not executed |
| Asset Asians | Exact geometric; TW; paired geometric control-variate GBM; Heston QE | Geometric/TW defaults; explicit GBM/Heston | [VALUATIONS-FX-AND-EXOTICS-ASSET-ASIAN](asset-asian.md) | Observed source; evidence located/inspected, not executed |
| Asset barriers | Continuous formula; datewise GBM/Heston; log-space Rannacher/CN PDE | Continuous analytic; discrete GBM; explicit PDE/Heston | [VALUATIONS-FX-AND-EXOTICS-ASSET-BARRIER](asset-barrier.md) | Observed source; evidence located/inspected, not executed |
| Lookbacks | Continuous extrema formulas; discrete/bridge-corrected GBM extrema | Continuous analytic; discrete MC | [VALUATIONS-FX-AND-EXOTICS-ASSET-LOOKBACK](asset-lookback.md) | Observed source; evidence located/inspected, not executed |
| Asset range accrual | Smile call-spread digitals; observed counts; GBM/quanto drift | StaticReplication default; explicit asset GBM | [VALUATIONS-FX-AND-EXOTICS-ASSET-RANGE-ACCRUAL](asset-range-accrual.md) | Observed source; evidence located/inspected, not executed |
| Callable rate range accrual | HW affine term forwards; bank-account discount; issuer LSMC | Strict scalar HW; single curve; plain MC without calls | [VALUATIONS-FX-AND-EXOTICS-CALLABLE-RANGE-HW](callable-range-hw.md) | Observed source; evidence located/inspected, not executed |
| Snowball/inverse floater | Recursive coupon/floor/cap; deterministic basis-adjusted HW term rates | HW MC default; explicit noncallable inverse-floater discounting | [VALUATIONS-FX-AND-EXOTICS-SNOWBALL-HW](snowball-hw.md) | Observed source; evidence located/inspected, not executed |
| TARN | Target-truncated accrual; early principal redemption; HW term rates | HW MC with deterministic no-stochastic-reset branch | [VALUATIONS-FX-AND-EXOTICS-TARN-TARGET](tarn-target.md) | Observed source; evidence located/inspected, not executed |
| FX spot rate/base amount diagnostics | SpotRate=PV/base amount; InverseRate its reciprocal; BaseAmount contractual | Registered calculators | [VALUATIONS-FX-AND-EXOTICS-FX-DELIVERY-CIP](fx-delivery-cip.md) | Ordered implementation inspected; not executed |
| FX swap currency IR01 and carry | DF exponential ±1bp; contract-rate defaults recomputed; included near/far carry | Domestic/foreign bespoke IR01; generic combined DV01 also available | [VALUATIONS-FX-AND-EXOTICS-FX-DELIVERY-CIP](fx-delivery-cip.md) | Ordered implementation inspected; not executed |
| FX ATM strike and higher-order risk | DNS strikes F exp(±σ²t/2); vanna/volga from symmetric vol scenarios | Convention-selected sign; bump capped at half resolved vol | [VALUATIONS-FX-AND-EXOTICS-FX-VANILLA-DELTA](fx-vanilla-delta.md) | Ordered implementation inspected; not executed |
| FX forecast interval second moment | ΔW + (partial log return + carry - ΔW/2)² | Every unresolved observation interval | [VALUATIONS-FX-AND-EXOTICS-FX-VARIANCE](fx-variance.md) | Ordered implementation inspected; not executed |
| FX variance risk and history | Sample-weighted variance vega versus smile FD vega; exact OHLC histories | Estimator-specific realized domain; no ATM fallback | [VALUATIONS-FX-AND-EXOTICS-FX-VARIANCE](fx-variance.md) | Ordered implementation inspected; not executed |
| Commodity exercise/settlement claims | Recorded cash intrinsic versus physical delivery forward; LR exercise tree | Recorded claim first; early exercise requires spot | [VALUATIONS-FX-AND-EXOTICS-COMMODITY-SPOT-OPTION](commodity-spot-option.md) | Ordered implementation inspected; not executed |
| Schwartz–Smith terminal forward pin | Shift long drift by ln(F_market/F_model)/t; exact two-factor steps | 100k paths/252 steps/seed42 default; configurable | [VALUATIONS-FX-AND-EXOTICS-COMMODITY-SPOT-OPTION](commodity-spot-option.md) | Ordered implementation inspected; not executed |
| Asian arithmetic control variate | Paired arithmetic/geometric paths; online covariance; exact seasoned E[G] | Only arithmetic GBM; captures full path results | [VALUATIONS-FX-AND-EXOTICS-ASSET-ASIAN](asset-asian.md) | Ordered implementation inspected; not executed |
| Barrier PDE event treatment | Barrier Dirichlet rebate; discrete node reset; restart Rannacher; KI parity | 200 space/100 time default; two implicit steps | [VALUATIONS-FX-AND-EXOTICS-ASSET-BARRIER](asset-barrier.md) | Ordered implementation inspected; not executed |
| HW parameter and simulation controls | Complete override → pricer pair → fitted CapFloor market scalars | 10k estimators; antithetic; four inter-event steps; OOS optional | [VALUATIONS-FX-AND-EXOTICS-CALLABLE-RANGE-HW](callable-range-hw.md) | Ordered implementation inspected; not executed |
| Callable range continuation state | Short-rate polynomial basis; adjusted accrued-coupon exercise cash | Issuer LSMC with degree2 row [1,r,r²,tr] | [VALUATIONS-FX-AND-EXOTICS-CALLABLE-RANGE-HW](callable-range-hw.md) | Ordered implementation inspected; not executed |
| Snowball/TARN reset-payment ordering | Reset computes pending amount; next payment flushes using bank factor | HW event grid includes maturity payment | [VALUATIONS-FX-AND-EXOTICS-SNOWBALL-HW](snowball-hw.md) | Ordered implementation inspected; not executed |

## Shared selection and evidence boundaries

The [FX family registration](../../../../finstack-quant/valuations/fx/src/family.rs) and [exotics family registration](../../../../finstack-quant/valuations/exotics/src/family.rs) select the reachable engines and local metric sets. The latter installs explicit Schwartz–Smith coefficients and path settings; it does not calibrate them. Common MC defaults come from [pricer_defaults.v1.json](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json). Shared listed-contract formulas, generic risk bump kernels, volatility resolution and process/payoff integrators are named dependencies, not duplicated mathematical implementations in these family files.

Tests cited by a record are classified by evidence role: deterministic formula identity, invariant, lifecycle regression, or alternate production-engine comparison. Such tests do not prove vendor agreement, market calibration, estimator convergence or authorization to adopt a convention. Missing independent benchmarks and concrete inconsistencies are collected in the [FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
