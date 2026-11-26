# VALUATIONS-OTHER-FIXED-INCOME-17: Convertible Tsiveriotis–Fernandes split-discount lattice

- **Decision status:** Proposed 2026-11-24 — not yet accepted. Record 02 requires a distinct methodology for a split-discount model; this record describes the one that is implemented and the choices an owner must accept or change.
- **Audit date/source:** 2026-11-25; `master` at `9ce9435bba`, which replaced the conversion-probability blend with a separate cash component.
- **Implementation status:** Implemented and registered as an optional model. The default convertible model is unchanged.
- **Validation status:** Limiting-case unit tests, registry tests and three fixtures referenced to an independent two-component lattice run. QuantLib prices a different formulation (see "Departure from QuantLib").
- **Scope inventory:** [README](README.md); [default lattice record](02-convertible-lattice.md).

## Proposed decision

1. **Offer the split-discount lattice as a selectable model, not the default.** `ModelKey::TsiveriotisFernandes` (wire name `tsiveriotis_fernandes`) prices a convertible bond on the same CRR equity lattice, schedule mapping and exercise resolver as the default `tree` model of [record 02](02-convertible-lattice.md). `tree` stays the default.
2. **Discount cash at the credit-risky rate and equity at the risk-free rate.** Each node carries its total value and the part of it that is cash. Over one step the cash part is discounted at `DF × C` and the remainder at `DF`, where `DF` is the risk-free factor and `C` the step's credit factor.
3. **Take the credit spread from the hazard curve as `hazard × (1 − recovery)`.** The step credit factor is `survival^(1 − recovery)`. Equity does not jump on default and the stock drifts at `r − q`: the hazard curve is a spread source only. Recovery is not paid separately, because it is already inside the spread.
4. **A claim redeemed in cash is all cash; a converted claim has none.** An issuer call the holder does not convert into, and a holder put, set the cash part to the redemption price. Conversion sets it to zero.
5. **Use the same model for every price a valuation reports.** The bond floor, Greeks, OAS, implied volatility, conversion and dividend sensitivities, the z-spread fallback of CS01 and the spot ladder all price on the model the valuation selected.

## Observed behavior

### Choice and rationale

[credit_tree.rs](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/credit_tree.rs) holds one lattice for both models; [engine.rs](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/engine.rs) selects it through `ConvertibleModel` and applies the model's credit discount to cash valued outside the lattice (at maturity, after exercise, between valuation and settlement). Without a credit curve the price is bit-identical to `tree`. With one, the two models differ in what the hazard curve means: `tree` treats it as a default intensity with a jump to zero and fractional recovery; this model treats it as the spread on the cash part of the claim.

### Departure from QuantLib

QuantLib's `BinomialConvertibleEngine` discounts each node at one blended rate, `r + (1 − p) × s`, where `p` is the probability of settling in shares. This lattice tracks the cash component itself, as the 1998 paper does, so the two disagree for every bond with a spread. Measured on the at-the-money three-year zero-coupon case with a 2% spread: this lattice converges to about 1,074,060 per 1,000,000 face (17536 steps), and QuantLib to 1,071,424, a difference of 0.25%. A coupon due at a conversion node is also paid here, as record 02 accepts, where QuantLib drops accrued interest on conversion.

### Limitations

- Both convertible lattices correct the exercise kink in the total value; the cash component follows the node-by-node decision. Gamma from a 1% spot bump is therefore positive but not stable across step counts: at the money (three-year zero coupon, 2% spread) it reads 138, 100, 60, 127, 134 and 72 per 1,000,000 face at 100, 200, 400, 800, 1096 and 2192 steps, while the price moves by less than 2e-4 and delta by less than 1%.
- One factor: deterministic rates, spread and volatility; no equity-credit correlation.
- The spread is flat in the node's share price. Models that widen the spread as the share falls are out of scope.

### Evidence

- Unit tests beside the engine, the `tf_` tests in [pricing tests](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/tests.rs): credit-free price equal to `tree` bit for bit for coupon, zero-coupon, mandatory, hard-call, soft-call and dividend-protected bonds; deep out-of-the-money price equal to the spread-discounted bond floor; deep in-the-money price independent of the spread; price falling as the spread widens for every bond with a cash flow, including the coupons of a mandatory convertible; a zero-coupon mandatory convertible independent of the spread.
- Registry tests in [test_tsiveriotis_fernandes.rs](../../../../finstack-quant/valuations/tests/instruments/convertible/test_tsiveriotis_fernandes.rs).
- Three fixtures under `finstack-quant/valuations/tests/golden/data/pricing/regression_goldens/convertible/` (`usd_convertible_tf_*_3y.json`), referenced to a two-component lattice at 35072 steps written in `scripts/golden/quantlib/convertible.py`, independent of this implementation:

  | Fixture | NPV difference | DV01 difference |
  |---|---|---|
  | At the money | 1.2e-5 | 0.18% |
  | Deep out of the money | 1.0e-5 | 0.005% |
  | Deep in the money | 0 | 0 |

  The reference DV01 is a 10bp central difference with a 2% bound.
- Reference: [Tsiveriotis–Fernandes 1998](../../../REFERENCES.md#tsiveriotis-fernandes-1998).

## Open questions for acceptance

1. Keep the coupon on conversion, consistent with `tree`, or drop it under this model?
