# Convertible bond: Tsiveriotis-Fernandes credit model

**Status:** built on 2026-11-24, slices 1 to 3 and 5 to 6 (see "As built" at the end). Slice 4
(mandatory conversion, dividend protection) reuses the shared valuator and is covered only by the
existing tests of the default model. Slice 7 is partly done.

**Written:** 2026-11-24, against `6077e06937` plus the uncommitted golden-fixture work of that day.

**Goal:** add the convertible-bond credit model QuantLib uses (Tsiveriotis-Fernandes on a binomial
equity lattice) as a second, selectable pricing model for `ConvertibleBond`, so that credit-risky
convertibles can be golden-tested against QuantLib and users can choose between the two market
conventions.

## 1. Why

`ConvertibleBond` has one registered model, `tree`: a jump-to-default equity lattice with recovery
of principal (`convertible/pricing/credit_tree.rs`, after Milanov and Kounchev 2012). QuantLib's
`BinomialConvertibleEngine` implements a different model, Tsiveriotis-Fernandes (TF). The two agree
only when there is no credit risk, so today the only QuantLib fixture is the credit-free
zero-coupon case (`pricing/quantlib/convertible/usd_convertible_zero_coupon_3y_quantlib.json`).
With issuer credit there is no external reference at all.

The two models answer the credit question differently:

| | `tree` (existing) | TF (proposed) |
|---|---|---|
| Default | Stock jumps to zero; bond pays recovery of principal | No default event on the lattice |
| Credit input | Hazard curve and recovery rate | Credit spread |
| How credit enters | Survival-weighted discounting and a default drift `+lambda` on the stock | Cash-only part of the bond is discounted at `r + s`; equity part at `r` |
| Stock drift | `r - q + lambda` | `r - q` |

Neither is wrong; they are different market conventions. TF is the convention QuantLib, and a
large share of dealer and vendor convertible models, implement.

## 2. The model

A convertible is split into an equity component, which the issuer can always deliver by issuing
shares, and a cash-only component (coupons, redemption, call and put cash), which carries issuer
credit risk. The cash-only component is discounted at the risky rate `r + s`, the rest at `r`.

QuantLib realises this on a recombining binomial lattice with three numbers per node: the value
`V`, a conversion probability `p`, and a blended discount rate `rho`.

**Terminal nodes** (maturity `T`):

```text
V   = redemption, then the event adjustments of section 2.2 at T
p   = 1 where conversion was taken, else 0
rho = p * r + (1 - p) * (r + s)
```

**Backward step** from the children `d` (down) and `u` (up) at level `i+1` to a node at level `i`:

```text
p_i   = pd * p_d + pu * p_u
rho_i = p_i * r + (1 - p_i) * (r + s)
V_i   = pd * V_d / (1 + rho_d * dt) + pu * V_u / (1 + rho_u * dt)
```

Each branch is discounted at the child's own blended rate, with simple per-step compounding.

### 2.1 What QuantLib's engine fixes

Read from QuantLib `master` on 2026-11-24 (`ql/pricingengines/bond/binomialconvertibleengine.hpp`,
`ql/pricingengines/bond/discretizedconvertible.{hpp,cpp}`, `ql/methods/lattices/tflattice.hpp`,
`ql/instruments/bonds/convertiblebonds.{hpp,cpp}`):

- **Lattice:** uniform `dt = T / steps` from settlement to maturity; constant `pu`, `pd`.
- **Flat inputs:** one volatility (the Black vol at maturity and spot), one risk-free rate (the
  continuous zero rate to maturity), one dividend yield (the same), one credit spread (a scalar
  quote). No term structure of any of them reaches the lattice.
- **Discrete dividends:** their present value is taken off the spot before the lattice is built
  and added back at each node when testing conversion and the soft-call trigger.
- **Event times:** exercise, call/put and coupon times are snapped to the nearest lattice time.

### 2.2 Event order at a lattice time

QuantLib applies, in this order:

1. **Call and put.**
   - Put: `V = max(V, put)`.
   - Hard call: `V = min(V, call)`, or `min(V, max(call, ratio * S))` on a conversion date.
   - Soft call: the same `min(V, max(call, ratio * S))`, on nodes where `S >= trigger` only.
   - A clean call or put price has accrued interest added.
2. **Coupon:** the coupon is added to every node.
3. **Conversion:** where `ratio * S >= V`, `V = ratio * S` and `p = 1`.

Two consequences matter for parity:

- The conversion test sees the coupon-inclusive value, and a converting node keeps only the
  conversion value. **The coupon is forfeited on conversion.**
- Call and put do not change `p`. A node called for cash keeps the `p` rolled back from its
  children.

### 2.3 Not verified in the QuantLib source

- The up/down factors and probabilities of the tree class the engine is templated on (CRR,
  Jarrow-Rudd, and so on) were not read.
- `DiscretizedAsset::adjustValues` and `isOnTime`, which decide exactly when the adjustments of
  section 2.2 run, were not read.
- The source files cite no paper. The model reference is Tsiveriotis and Fernandes (1998),
  already listed in `docs/REFERENCES.md` (`#tsiveriotis-fernandes-1998`).

## 3. Design

### 3.1 Selecting the model

Add a model key and register a second pricer; do not add a switch inside the existing pricer.

- `ModelKey::TsiveriotisFernandes`, wire name `tsiveriotis_fernandes`
  (`valuations/core/src/pricer/keys.rs`).
- `ConvertibleTsiveriotisFernandesPricer`, registered beside `ConvertibleTreePricer` in
  `valuations/fixed-income/src/family.rs`.
- `tree` stays the instrument's `default_model()`. Nothing changes for callers that do not ask
  for the new model.

This is how every other instrument offers alternative models, and it is what the golden runner's
`model` field, the Python `model=` argument and the WASM facade already select on.

### 3.2 Credit input

TF needs a credit spread, and the bond carries a hazard curve and a recovery rate. The spread is
derived per lattice step from what the bond already references, so the instrument and market
schemas do not change:

```text
s_i = lambda_i * (1 - R)      lambda_i = -ln(Q(t_i, t_i+1)) / dt
```

with `Q` the conditional survival over the step from `credit_curve_id` and `R` the bond's
`recovery_rate` (zero if unset). A bond without `credit_curve_id` has `s = 0` and must reproduce
the existing credit-free price.

A flat hazard `h` with `R = 0` gives the flat spread `s = h`, which is the QuantLib comparison
case.

### 3.3 Lattice and conventions

Reuse the existing convertible lattice inputs (`ConvertibleBondValuator`: step maps for coupons,
principal, calls, puts, conversion window, soft-call trigger, per-step risk-free discount factors)
and add a TF engine beside `CreditTreeEngine`.

| Choice | QuantLib | This model | Why |
|---|---|---|---|
| Risk-free discounting | Flat rate, `1 / (1 + rho * dt)` | Per-step curve factor, `df_i * exp(-(1 - p) * s_i * dt)` | The library discounts on the curve everywhere; the simple-compounding form differs at `O(dt)` and vanishes with steps |
| Stock drift | `r - q`, flat | `r_i - q` from the per-step curve forward | Same as the existing tree without the default drift |
| Volatility | Flat, at maturity and spot | Flat, as resolved today by `resolve_volatility` | Unchanged |
| Lattice | Tree type of the engine template | `EvolutionParams::equity_crr`, as today | Unchanged |
| Discrete dividends | PV off the spot, added back at nodes | Continuous yield only | The instrument has no discrete-dividend input today |
| Event dates | Snapped to the nearest step | As the existing valuator maps them | Unchanged |

On flat curves and with enough steps the two columns converge to the same price. That is what
the golden test asserts (section 5).

### 3.4 Contract semantics that differ from QuantLib

These are properties of the instrument, not of the credit model, and the existing `tree` pricer
has already decided them. The TF pricer must use the same contract so that switching models
changes only the credit treatment:

| Term | QuantLib | Finstack contract (both models) |
|---|---|---|
| Coupon on a conversion date | Forfeited on conversion | Retained on conversion |
| Order at a node | Call/put, coupon, conversion | Conversion, call, put, then coupon |
| Forced conversion on call | `min(V, max(call, ratio * S))` | Same economics, gated on conversion being permitted |
| Mandatory and variable conversion | Not supported | Supported |
| Dividend protection | Not supported | Conversion ratio accretes at the dividend yield |

So a QuantLib comparison is clean only where these do not bite: zero-coupon bonds, or coupon
bonds on which conversion is never optimal on a coupon date. Section 6 lists this as a decision.

### 3.5 State and event handling

Per node the engine carries `(V, p)`; `rho` is derived, not stored. At each step, in the existing
Finstack order:

1. Roll back `p` and `V` (section 2, with the discounting of section 3.3).
2. Conversion, where permitted: if `conversion > V`, set `V = conversion`, `p = 1`.
3. Call, where triggered: the issuer calls when it lowers the value. If the holder converts in
   response, `V = conversion`, `p = 1`; if the holder takes cash, `V = call`, `p = 0`.
4. Put: if `put > V`, `V = put`, `p = 0`.
5. Add the coupon and any scheduled principal.

Steps 3 and 4 set `p` on a cash exercise. QuantLib leaves `p` untouched there (section 2.2). A
cash redemption is by definition a cash-only claim, so `p = 0` is the model-consistent choice; it
is recorded as a deliberate difference and is one of the things the golden test will measure.

The exercise decisions are kinks in `V` across the level. Apply them with
`trees::kink_smoothing` (`smoothed_max` / `smoothed_min`) so the price does not ripple with the
spot level, as the bond lattices now do. `p` at a corrected node takes the fraction of the node's
cell on the converting side.

### 3.6 Metrics

The convertible registers 34 metrics. Most reprice through the registry and so follow the selected
model unchanged: `delta`, `gamma`, `vega`, `theta`, `rho`, `dv01`, `bucketed_dv01`, `dividend01`,
`conversion01`, `parity`, `conversion_value`, `conversion_premium` and the carry family. The ones
that carry their own credit treatment need a decision in the implementation slice:

- `cs01` and `bucketed_cs01` bump the hazard curve. Under TF that moves the derived spread;
  confirm the bump reaches `s_i` and document the unit.
- `bond_floor` is priced by `price_bond_floor` with the existing tree's credit treatment. Under
  TF the floor is the straight bond discounted at `r + s`.
- `oas` and `implied_vol` are solvers around the pricer (`metrics/oas.rs`,
  `metrics/implied_vol.rs`); confirm each reprices with the selected model, not with `tree`.
- The credit cross-gammas (`cross_gamma_spot_credit`, `cross_gamma_rates_credit`,
  `cross_gamma_credit_vol`) follow from `cs01` once that is settled.

## 4. Slices

Each slice ends with the product working and its scoped checks green.

1. **Model key and pricer shell.** Add `ModelKey::TsiveriotisFernandes`, the pricer and its
   registration. The pricer calls a TF engine that supports the credit-free case only and must
   reproduce the `tree` price bit for bit when `credit_curve_id` is absent. Regenerate the
   registry snapshot and schemas (`mise run gen-write`).
2. **Credit spread and blended discounting.** Derive `s_i`, carry `p`, implement sections 2 and
   3.3 for a bullet zero-coupon bond with voluntary conversion. Unit tests: `s = 0` reproduces
   slice 1; the bond floor of a deep out-of-the-money convertible equals the straight bond at
   `r + s`; a deep in-the-money convertible is insensitive to `s`.
3. **Calls, puts, coupons, soft call.** Section 3.5 in full, with kink correction. Unit tests for
   each exercise right against the limiting cases above.
4. **Mandatory conversion and dividend protection.** Reuse the valuator's existing handling.
5. **Metrics.** The credit-dependent metrics of section 3.6 under TF.
6. **Golden fixtures.** Section 5.
7. **Bindings and docs.** Python stub and WASM declarations for the new model name, the parity
   ledger, `docs/REFERENCES.md`, and the convertible README.

## 5. Golden tests against QuantLib

Add builders to `scripts/golden/quantlib/convertible.py` using
`ql.BinomialCRRConvertibleEngine` with a non-zero credit spread quote, on flat curves.

| Fixture | Purpose |
|---|---|
| Zero-coupon, at the money, flat spread 2% | The credit model itself, free of the coupon-on-conversion difference |
| Zero-coupon, deep out of the money | Bond floor at `r + s` |
| Zero-coupon, deep in the money | Insensitivity to `s` |
| Zero-coupon, hard call | Call handling and the `p` choice of section 3.5 |
| Zero-coupon, put | Put handling |

Tolerances are set before the first comparison, the way the existing zero-coupon fixture did it:
price QuantLib on several lattice types at the comparison step count and at a much finer
reference, and take about three times the spread between them. Expect roughly 1e-4 relative on
NPV and 1e-3 on DV01. The simple-versus-continuous compounding difference of section 3.3 is
`O(dt)` and must fall inside that bound; if it does not, that is a finding to report, not a
tolerance to widen.

A coupon-bearing fixture is not planned unless the decision in section 6 allows one.

## 6. Decisions needed before building

1. **Coupon on conversion.** QuantLib forfeits it; the Finstack contract retains it. Keeping the
   contract (recommended) means coupon bonds cannot be compared with QuantLib on conversion
   dates. The alternative is an explicit contract term on `ConversionSpec`, which is a schema
   change affecting both models.
2. **Credit spread source.** Section 3.2 derives it from the hazard curve and recovery. The
   alternative is a quoted spread on the bond. Deriving it (recommended) needs no schema change
   and keeps `cs01` meaningful.
3. **`p` on a cash call or put.** Section 3.5 sets `p = 0`; QuantLib leaves it. Matching QuantLib
   exactly would make the fixtures tighter but treats a cash redemption as partly equity.
4. **Discrete dividends.** Out of scope here because the instrument has no input for them.

## 7. Out of scope

- Replacing or changing the existing `tree` model.
- A stochastic interest-rate or stochastic-credit convertible model.
- The Ayache-Forsyth-Vetzal extension of TF that models the stock's behaviour on default.

## 8. As built

Decisions 1 to 3 of section 6 were taken as recommended; decision 4 stays out of scope.

**What exists**

- `ModelKey::TsiveriotisFernandes` (`tsiveriotis_fernandes`) and
  `ConvertibleTsiveriotisFernandesPricer`, registered beside the `tree` pricer.
- `convertible/pricing/tf_tree.rs`: the lattice of sections 2 and 3.3 and the spread-discounted
  bond floor.
- The convertible's own metric calculators now price on the model the valuation selected
  (`delta`, `gamma`, `vega`, `rho`, `oas`, `bond_floor`, `conversion01`, `dividend01`, the
  z-spread fallback of `cs01`). Before this they always priced with `tree`.
- The shared lattice setup keeps a payment's value when it is moved onto the grid using the
  selected model's credit discount (survival for `tree`, `survival^(1 - recovery)` for TF).
- Every cash payment valued outside the lattice (at maturity, after exercise, between valuation
  and settlement) uses the same model-specific credit discount.
- Tests: limiting cases beside the engine (`convertible/pricing/tests.rs`, the `tf_` tests) and
  registry-level tests that price, bond floor, delta, vega, DV01 and CS01 all follow the selected
  model (`valuations/tests/instruments/convertible/test_tsiveriotis_fernandes.rs`).

**Differences from this spec**

- **Kink correction (section 3.5) is not applied to the value.** Only the conversion probability
  is smoothed: on conversion it takes the share of the node's cell that converts. The value takes
  the node's own decision, exactly as the `tree` lattice does. This keeps the credit-free price
  bit-identical to `tree`, which slice 1 required and which a corrected value would break.
- **Fixtures.** Three, not five: at the money, deep out of the money, deep in the money. The call
  and put fixtures were built and dropped. With `p = 0` on a cash redemption (decision 3) the
  callable price is 0.107% below QuantLib; leaving `p` unchanged, as QuantLib does, brought it
  within 0.010%. The puttable price sat at 0.010%, on the bound, for the same reason.
- **DV01 reference.** QuantLib's lattice price is not smooth in the rate at 1bp with a credit
  spread: its own 1bp DV01 moves by 1.6% between 1096, 4384 and 17536 steps. The fixtures take
  the reference DV01 from a 10bp central difference and allow 2%.

**Measured against QuantLib** (Finstack at 1096 steps, QuantLib at 17536):

| Fixture | NPV difference | DV01 difference |
|---|---|---|
| At the money | 6e-6 | 0.33% |
| Deep out of the money | 1.6e-5 | 0.007% |
| Deep in the money | 0 | 0 |

**Known limitation.** Gamma from the standard 1% spot bump is lattice noise at the money for both
models, and can be negative under TF (about -66 at 1096 steps against roughly +75 from a 4% bump;
`tree` gives +155 against roughly +86). Curing it needs the exercise kink in the value corrected
in both lattices, which changes `tree` prices and is a separate decision.

**Follow-ups closed 2026-11-24**

- The exercise kink in the value is now corrected in both lattices, which supersedes the "Kink
  correction" difference and the "Known limitation" above: gamma is positive and converges with
  the step count, and the credit-free price still equals `tree` bit for bit.

- Methodology record written as a proposal:
  `docs/decisions/financial/valuations-other-fixed-income/17-convertible-tsiveriotis-fernandes.md`.
- `convertible_spot_ladder` takes the model (Rust, Python `model=`, WASM `model`).
- Tests now cover mandatory conversion, dividend protection, hard and soft calls and zero coupons
  under TF. They exposed one limitation, recorded in the methodology record: a mandatory
  convertible's coupons are discounted without the spread.
