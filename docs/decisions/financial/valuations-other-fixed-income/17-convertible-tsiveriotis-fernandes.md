# VALUATIONS-OTHER-FIXED-INCOME-17: Convertible Tsiveriotis–Fernandes split-discount lattice

- **Decision status:** Proposed 2026-11-24 — not yet accepted. Record 02 requires a distinct methodology for a split-discount model; this record describes the one that is implemented and the choices an owner must accept or change.
- **Audit date/source:** 2026-11-24; working tree on `master` after `7006c3f651`.
- **Implementation status:** Implemented and registered as an optional model. The default convertible model is unchanged.
- **Validation status:** Limiting-case unit tests, registry tests and three QuantLib 1.43 reference fixtures run. Callable and puttable bonds are not referenced externally, for the reason in "Departures from QuantLib".
- **Scope inventory:** [README](README.md); [default lattice record](02-convertible-lattice.md).

## Proposed decision

1. **Offer the split-discount lattice as a selectable model, not the default.** `ModelKey::TsiveriotisFernandes` (wire name `tsiveriotis_fernandes`) prices a convertible bond on the same CRR equity lattice, schedule mapping and exercise resolver as the default `tree` model of [record 02](02-convertible-lattice.md). `tree` stays the default.
2. **Discount cash at the credit-risky rate and equity at the risk-free rate.** Each node carries its value `V` and the probability `p` that the claim is ultimately settled in shares. One step's discount factor on a branch is `DF × C^(1 − p_child)`, where `DF` is the risk-free factor and `C` the step's credit factor.
3. **Take the credit spread from the hazard curve as `hazard × (1 − recovery)`.** The step credit factor is `survival^(1 − recovery)`. Equity does not jump on default and the stock drifts at `r − q`: the hazard curve is a spread source only. Recovery is not paid separately, because it is already inside the spread.
4. **Set `p = 0` when the claim is redeemed in cash.** An issuer call the holder does not convert into, and a holder put, both settle in cash, so the node is discounted as cash from there back. Conversion sets `p` to the share of the node's log-spot cell that converts.
5. **Use the same model for every price a valuation reports.** The bond floor, Greeks, OAS, implied volatility, conversion and dividend sensitivities, the z-spread fallback of CS01 and the spot ladder all price on the model the valuation selected.

## Observed behavior

### Choice and rationale

[tf_tree.rs](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/tf_tree.rs) holds the lattice; [engine.rs](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/engine.rs) selects it through `ConvertibleModel` and applies the model's credit discount to cash valued outside the lattice (at maturity, after exercise, between valuation and settlement). Without a credit curve the price is bit-identical to `tree`. With one, the two models differ in what the hazard curve means: `tree` treats it as a default intensity with a jump to zero and fractional recovery; this model treats it as the spread on the cash part of the claim.

### Departures from QuantLib

QuantLib's `BinomialConvertibleEngine` is the reference implementation of this model. Two choices here differ from it.

- **Cash redemption resets `p`.** QuantLib leaves the conversion probability unchanged at a call or put. Setting it to zero is the reading of the 1998 paper in which a redeemed claim is cash. Measured effect at 1096 steps against QuantLib at 17536: a callable bond prices 0.107% below QuantLib (0.010% when `p` is left unchanged), and a puttable bond sits at 0.010%. No callable or puttable reference fixture is committed for this reason.
- **A coupon due at a conversion node is paid.** The exercise resolver shared with `tree` adds the scheduled coupon after the exercise decision, as record 02 accepts. QuantLib drops accrued interest on conversion.

### Limitations

- Gamma from a 1% spot bump converges from below as the lattice refines: at the money it reaches about 90% of its converged value at 400 steps. Both convertible lattices correct the exercise kink in the value, so it is no longer lattice noise.
- **Coupons of a claim that converts are discounted with only part of the spread.** A node is discounted at `r + (1 − p) × s`, QuantLib's formulation. A mandatory convertible has `p = 1` at every node, so its cash coupons are discounted at `r`: the price does not respond to the spread (it rises by 0.002% at a 10% hazard, from the payment-date correction). A split that tracked the cash component separately, as the 1998 paper does, would take roughly 4% off a five-year 5% mandatory at that hazard. Use `tree` for mandatory convertibles with credit risk.
- One factor: deterministic rates, spread and volatility; no equity-credit correlation.
- The spread is flat in the node's share price. Models that widen the spread as the share falls are out of scope.

### Evidence

- Unit tests beside the engine, the `tf_` tests in [pricing tests](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/tests.rs): credit-free price equal to `tree` bit for bit for coupon, zero-coupon, mandatory, hard-call, soft-call and dividend-protected bonds; deep out-of-the-money price equal to the spread-discounted bond floor; deep in-the-money price independent of the spread; price falling as the spread widens for every bond that can redeem in cash; a zero-coupon mandatory convertible independent of the spread; a coupon-paying mandatory convertible within 0.005% of its credit-free price (the limitation above).
- Registry tests in [test_tsiveriotis_fernandes.rs](../../../../finstack-quant/valuations/tests/instruments/convertible/test_tsiveriotis_fernandes.rs).
- QuantLib fixtures under `finstack-quant/valuations/tests/golden/data/pricing/quantlib/convertible/`, generated by `scripts/golden/quantlib/convertible.py`:

  | Fixture | NPV difference | DV01 difference |
  |---|---|---|
  | At the money | 6e-6 | 0.33% |
  | Deep out of the money | 1.6e-5 | 0.007% |
  | Deep in the money | 0 | 0 |

  The reference DV01 is a 10bp central difference with a 2% bound: QuantLib's own 1bp DV01 moves by 1.6% between 1096 and 17536 steps.
- Reference: [Tsiveriotis–Fernandes 1998](../../../REFERENCES.md#tsiveriotis-fernandes-1998).

## Open questions for acceptance

1. Keep `p = 0` on cash redemption, or follow QuantLib and leave `p` unchanged? The second would allow callable and puttable reference fixtures.
2. Keep the coupon on conversion, consistent with `tree`, or drop it under this model?
3. Replace the conversion-probability blend with a separate cash component, so that coupons always carry the full spread? This departs from QuantLib for every bond, not only mandatory ones, and would retire the three reference fixtures.
