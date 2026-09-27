# FX

Foreign-exchange instruments: spot, outright forwards, swaps, NDFs, listed
futures and futures options on the linear/listed side; Garman–Kohlhagen
vanillas plus barriers, digitals, touches, variance swaps and quantos on the
option side. Twelve leaves share one currency-pair convention, one spot-date
rule and one end-of-day lifecycle policy.

This file is an index — which leaf owns which product and which convention it
follows — plus the conventions shared across the family. No leaf here carries
its own README; per-instrument formulas, references and examples are in the
leaf `mod.rs` rustdoc.

## Leaves

| Directory | Prices | Market convention / model |
|-----------|--------|---------------------------|
| `fx_spot/` | Spot FX positions | Value in quote currency = base amount × spot; settlement rolled with the CLS-consistent joint-calendar spot rule (T+2 for majors, T+1 for USD/CAD) |
| `fx_forward/` | Outright forwards | Covered interest parity, `F = S · DF_foreign(T) / DF_domestic(T)`; stable PV `N · (S · DF_foreign − K · DF_domestic)`. Standard constructors take calendar `Tenor` |
| `fx_future/` | Exchange-listed deliverable FX futures | Daily variation-margin P&L. `fair_price` uses the deterministic-rate CIP-forward approximation and does not include stochastic-rate/FX convexity |
| `fx_future_option/` | Options on listed FX futures | Listed-future option lifecycle and settlement terms |
| `fx_swap/` | FX swaps with near and far legs | Near leg at spot, far leg at the CIP forward; standard far maturities use calendar `Tenor`, explicit business-day roll and EOM policy |
| `ndf/` | Non-deliverable forwards on restricted pairs | `NdfQuoteConvention::BasePerSettlement` (default, standard for Asian NDFs) or `SettlementPerBase`; typed `NdfFixingSource` (`Pboc`, `Cnhfix`, `Rbi`, `Kftc`, `Ptax`, `Taifx`, `PhpBval`, `Jisdor`, `Bnm`, `Other`). Standard maturities use calendar `Tenor`; post-fixing valuation requires an observed rate after the fixing date |
| `fx_option/` | Same-day cash-settled European FX calls and puts | Garman–Kohlhagen (1983). The shape is structurally European and has no physical-settlement mode. `FxDeltaConvention` records venue, premium currency and quoted delta kind; metrics expose spot, forward, premium-adjusted spot and premium-adjusted forward delta separately |
| `fx_barrier_option/` | Knock-in / knock-out FX barriers | `Monitoring::Continuous` uses Reiner–Rubinstein (1991); `Monitoring::Discrete { observation_dates }` uses GBM Monte Carlo and observes only contractual dates. MC results carry standard error, path counts, seed, time grid and variance-reduction diagnostics |
| `fx_digital_option/` | FX binaries | `DigitalPayoutType::CashOrNothing` (fixed cash in the payout currency) or `AssetOrNothing` (one unit of the base currency), Garman–Kohlhagen forms. Cash call + cash put = discounted payout |
| `fx_touch_option/` | One-touch and no-touch (American binaries) | `TouchType` = `OneTouch` / `NoTouch`; `BarrierDirection` sets η. One-touch supports `AtHit` or `AtExpiry`; no-touch is valid only with `AtExpiry`. Rubinstein–Reiner (1991) closed form assumes continuous monitoring |
| `fx_variance_swap/` | FX variance swaps | Carr–Madan replication over OTM FX options, accounting for the domestic/foreign rate differential; observation schedule built on joint base/quote calendars |
| `quanto_option/` | Quanto options on a foreign asset settled in domestic currency at a **fixed** FX rate | Drift adjusted by `r_f − q − ρ·σ_asset·σ_FX`, discounted at `r_d` (`ModelKey::QuantoBS`). Negative ρ raises call value, positive ρ lowers it. Analytic only; no public Monte Carlo stub |
| `shared.rs` | Not an instrument | `pub(crate)` CIP inputs, FX option inputs and the common end-of-day event policy |

`quanto_option/` lives here because it is an FX-correlation product and carries
the `"fx"` JSON category, but its metrics and pricer register with the exotics
shards. See Registration below.

## Public surface

Import path: `finstack_quant_valuations::instruments::fx::<leaf>`. All twelve
leaf directories are `pub mod`; `shared` is `pub(crate) mod`. Headline types
are also re-exported flat at `finstack_quant_valuations::instruments`:

`FxSpot`, `FxForward`, `FxFuture`, `FxFutureOption`, `FxSwap`, `Ndf`,
`FxOption`, `FxBarrierOption`, `FxDigitalOption`, `DigitalPayoutType`,
`FxTouchOption`, `TouchType`, `BarrierDirection`, `PayoutTiming`,
`FxVarianceSwap`, `QuantoOption`.

Family-path-only types include `fx_option::{FxDeltaConvention,
FxDeltaConventionKind, FxOptionBuilder}`,
`fx_forward::FxForwardBuilder`, `fx_barrier_option::Monitoring`,
`fx_variance_swap::FxVarianceSwapBuilder`, and
`ndf::{NdfQuoteConvention, NdfFixingSource}`. Monte Carlo payoff kernels and
pricers are crate-private.

Three leaves re-export a shared parameter type under their own path:
`fx_option::FxUnderlyingParams` and `fx_swap::FxUnderlyingParams`, and
`fx_variance_swap::PayReceive`. All three name the same types that already exist
at `instruments::*` — aliases, not FX-specific variants.

Inside a leaf, `metrics/`, `pricer.rs`, `types.rs` and Monte Carlo payoff
adapters are `pub(crate)` or private; supported items surface through each
leaf's `pub use`. Metric calculator implementations remain internal and are
reached through the standard metric registry.

## Family conventions

- **Pair orientation.** Base/quote throughout: `EUR/USD = 1.10` means one EUR
  buys 1.10 USD. Base is the foreign ("asset") currency, quote is the domestic
  ("numéraire") currency. Option payoffs are quoted in domestic; notionals are
  in base unless a type says otherwise.
- **Spot dates and standard tenors.** Leaves that derive settlement dates roll
  through `instruments::fx_spot_date_for_pair`. Signed lags support bounded T−N
  stepping. Forward, swap and NDF standard-maturity constructors then add a
  calendar `Tenor` with explicit business-day and EOM policy; broken-date
  constructors take explicit contractual dates.
- **Two clocks.** Option pricers combine a vol-surface clock with a
  discount-curve clock; the model rate must satisfy `exp(-r·t_vol) = df`.
  `common_impl::helpers::zero_rate_from_df` derives that rate and
  `instruments::pricing::time` supplies the curve-consistent lookups. Never
  divide `ln(df)` by a year fraction measured on a different day count.
- **Volatility precedence.** Surface-driven pricers resolve σ through
  `common_impl::vol_resolution::resolve_sigma_at`, so
  `MarketQuoteOverrides::implied_volatility` behaves as a flat σ across tenor
  and strike everywhere in the family.
- **FX policy visibility.** `FxForward`, `FxOption` and `QuantoOption` override
  `Instrument::valuation_details` to surface the FX-matrix triangulation flag on
  the `ValuationResult`, satisfying the workspace FX-policy-visibility
  invariant. A new FX instrument that triangulates should do the same.
- **Delta conventions are explicit.** `FxOption::delta_convention` records the
  pair's venue convention and premium currency. Metrics report unadjusted spot
  and forward delta plus distinct premium-adjusted spot and forward delta.
- **Event dates are end-of-day.** Fixing, expiry, delivery and settlement remain
  live on their contractual date and are extinguished on the following
  valuation date.

## Registration

The general checklist is in [`../README.md`](../README.md#adding-an-instrument).
Landing sites for this family:

| Step | Where |
|------|-------|
| Pricer | `src/pricer/fx.rs` for eleven leaves — this shard also registers `rates::xccy_swap`. `QuantoOption` registers in `src/pricer/exotics.rs` |
| Instrument key | `InstrumentType` variant in `src/pricer/keys.rs` |
| JSON tag | `with_instrument_json_registry!` in `../json_loader.rs`, category `"fx"`. Tags: `fx_spot`, `fx_swap`, `fx_forward`, `ndf`, `fx_option`, `fx_digital_option`, `fx_touch_option`, `fx_barrier_option`, `fx_variance_swap`, `fx_future`, `fx_future_option`, `quanto_option` |
| Metrics | `register_<name>_metrics` in the leaf's `metrics/`, called from `register_fx_instrument_metrics` in `src/metrics/core/standard_registry.rs` — except `register_quanto_option_metrics`, which is called from `register_exotic_instrument_metrics` |
| Schemas | `mise run rust-gen-schemas`, verified by `mise run rust-check-schemas` |

`FxOption` is registered under `ModelKey::Black76` even though the pricer is
Garman–Kohlhagen in spot form; the two are equivalent through the CIP forward.
Keep the key and the comment together if either moves.

## Tests and benches

Integration tests live in `../../../tests/instruments/<leaf>/`, compiled into
the single `instruments` target. Dedicated directories exist for `fx_spot`,
`fx_forward`, `fx_swap`, `ndf`, `fx_option`, `fx_barrier_option`,
`fx_variance_swap` and `quanto_option`. `fx_digital_option` and
`fx_touch_option` are covered by colocated `#[cfg(test)]` modules plus the
cross-cutting registry, serde and `fx_dependency_completeness` contract tests.

```bash
# whole integration target
mise run rust-test-integration -- finstack-quant-valuations instruments

# focused leaf tests
mise run rust-test-filter -- finstack-quant-valuations fx_option --integration instruments
mise run rust-test-filter -- finstack-quant-valuations fx_dependency_completeness --integration instruments

# crate unit and integration tests
mise run rust-test-crate -- finstack-quant-valuations

# final workspace gate
mise run rust-test
```

Criterion benches in `../../../benches/`: `fx_pricing` (spot, forward, swap,
NDF, option and metrics) and `fx_exotics_pricing` (barrier, touch, digital,
variance swap, quanto).

```bash
mise run rust-bench-crate -- finstack-quant-valuations fx_exotics_pricing
mise run rust-bench
```

## Related

- [`../README.md`](../README.md) — `Instrument` trait, JSON contract, add-an-instrument checklist
- [`../common_impl/README.md`](../common_impl/README.md) — `fx_dates`, two-clock plumbing, vol resolution, variance replication
- [`../rates/README.md`](../rates/README.md) — cross-currency swaps live there but register in this family's pricer shard
- [`../commodity/README.md`](../commodity/README.md) — the other Black-76-centric family
- [`../../metrics/README.md`](../../metrics/README.md) — greeks and metric registration
- [`../../../tests/instruments/README.md`](../../../tests/instruments/README.md) — test layout and generated fixtures
