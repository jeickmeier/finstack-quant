import { emit } from "../../../scripts/fixture-output.mjs";
import { native } from "../../../scripts/fixture-inputs.mjs";
// The worker's ModelsApi.optionCalculator over the raw package (the facade's
// models namespace re-exports optionCalculation unchanged).
function calculate(request) {
  try {
    return { ok: true, value: native.optionCalculation(request) };
  } catch (error) {
    return {
      ok: false,
      error: { name: error.name, message: error.message, kind: error.kind },
    };
  }
}
// Illustrative terms: at-the-money equity, out-of-the-money put on a forward,
// and a normal-model rate call. Each implied-volatility request inverts the
// price Rust returned for the same terms.
const models = {
  black_scholes: {
    kind: "black_scholes",
    spot: 100,
    strike: 100,
    rate: 0.05,
    div_yield: 0.01,
    expiry: 1,
    is_call: true,
  },
  black76: {
    kind: "black76",
    forward: 100,
    strike: 95,
    df: 0.97,
    expiry: 0.5,
    is_call: false,
  },
  bachelier: {
    kind: "bachelier",
    forward: 0.03,
    strike: 0.0325,
    expiry: 2,
    is_call: true,
  },
};
const vols = { black_scholes: 0.2, black76: 0.25, bachelier: 0.0075 };
const cases = {};
for (const [name, model] of Object.entries(models)) {
  const priced = { model, solve: { kind: "price", vol: vols[name] } };
  const priceResult = calculate(priced);
  const solved = {
    model,
    solve: { kind: "implied_vol", price: priceResult.value.price },
  };
  cases[`${name}_price`] = { request: priced, envelope: priceResult };
  cases[`${name}_implied_vol`] = {
    request: solved,
    envelope: calculate(solved),
  };
}
const add = (name, request) => {
  cases[name] = { request, envelope: calculate(request) };
};
add("theta_trading_days", {
  model: { ...models.black_scholes, theta_days_per_year: 252 },
  solve: { kind: "price", vol: vols.black_scholes },
});
// Rust rejects Greeks at a zero volatility, so the calculation fails whole.
add("zero_vol_rejects_greeks", {
  model: models.black_scholes,
  solve: { kind: "price", vol: 0 },
});
// Rust rejects a price below intrinsic value for every model it inverts.
add("below_intrinsic", {
  model: { ...models.black_scholes, spot: 120 },
  solve: { kind: "implied_vol", price: 1 },
});
add("bachelier_below_intrinsic", {
  model: { ...models.bachelier, forward: 0.05 },
  solve: { kind: "implied_vol", price: 0.001 },
});
// An empty field reaches Rust as NaN and is rejected there.
cases.empty_field = {
  request: {
    model: { ...models.black76, df: "NaN" },
    solve: { kind: "price", vol: vols.black76 },
  },
  envelope: calculate({
    model: { ...models.black76, df: Number.NaN },
    solve: { kind: "price", vol: vols.black76 },
  }),
};
await emit(new URL("./cases.json", import.meta.url), JSON.stringify(cases));
