import { createRequire } from "node:module";
import { emit } from "../../../scripts/fixture-output.mjs";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
// The worker's ModelsApi.optionCalculator over the raw package (the facade's
// models namespace re-exports these functions unchanged).
function price(r, vol) {
  if (r.model === "black_scholes")
    return native.bsPrice(
      r.spot,
      r.strike,
      r.rate,
      r.divYield,
      vol,
      r.expiry,
      r.isCall,
    );
  if (r.model === "black76")
    return native.black76Price(
      r.forward,
      r.strike,
      r.df,
      vol,
      r.expiry,
      r.isCall,
    );
  return native.bachelierPrice(r.forward, r.strike, vol, r.expiry, r.isCall);
}
function impliedVol(r) {
  if (r.model === "black_scholes")
    return native.bsImpliedVol(
      r.spot,
      r.strike,
      r.rate,
      r.divYield,
      r.expiry,
      r.price,
      r.isCall,
    );
  if (r.model === "black76")
    return native.black76ImpliedVol(
      r.forward,
      r.strike,
      r.df,
      r.expiry,
      r.price,
      r.isCall,
    );
  return native.impliedVolBachelier(
    r.price,
    r.forward,
    r.strike,
    r.expiry,
    r.isCall,
  );
}
function greeks(r, vol) {
  if (r.model === "black_scholes")
    return native.bsGreeks(
      r.spot,
      r.strike,
      r.rate,
      r.divYield,
      vol,
      r.expiry,
      r.isCall,
      r.thetaDaysPerYear ?? undefined,
    );
  if (r.model === "black76")
    return native.black76Greeks(r.forward, r.strike, vol, r.expiry, r.isCall);
  return native.bachelierGreeks(r.forward, r.strike, vol, r.expiry, r.isCall);
}
const failure = (error) => ({
  ok: false,
  error: { name: error.name, message: error.message, kind: error.kind },
});
function attempt(call) {
  try {
    return { ok: true, value: call() };
  } catch (error) {
    return failure(error);
  }
}
function calculate(request) {
  try {
    const { model, mode } = request;
    if (mode === "price") {
      const value = price(request, request.vol);
      return {
        ok: true,
        value: {
          model,
          mode,
          price: value,
          greeks: attempt(() => greeks(request, request.vol)),
        },
      };
    }
    const vol = impliedVol(request);
    return {
      ok: true,
      value: {
        model,
        mode,
        impliedVol: vol,
        greeks: attempt(() => greeks(request, vol)),
      },
    };
  } catch (error) {
    return failure(error);
  }
}
// Illustrative terms: at-the-money equity, out-of-the-money put on a forward,
// and a normal-model rate call. Each implied-volatility request inverts the
// price Rust returned for the same terms.
const terms = {
  black_scholes: {
    model: "black_scholes",
    spot: 100,
    strike: 100,
    rate: 0.05,
    divYield: 0.01,
    expiry: 1,
    isCall: true,
  },
  black76: {
    model: "black76",
    forward: 100,
    strike: 95,
    df: 0.97,
    expiry: 0.5,
    isCall: false,
  },
  bachelier: {
    model: "bachelier",
    forward: 0.03,
    strike: 0.0325,
    expiry: 2,
    isCall: true,
  },
};
const vols = { black_scholes: 0.2, black76: 0.25, bachelier: 0.0075 };
const cases = {};
for (const [model, term] of Object.entries(terms)) {
  const priced = { ...term, mode: "price", vol: vols[model] };
  const priceResult = calculate(priced);
  const solved = {
    ...term,
    mode: "implied_vol",
    price: priceResult.value.price,
  };
  cases[`${model}_price`] = { request: priced, envelope: priceResult };
  cases[`${model}_implied_vol`] = {
    request: solved,
    envelope: calculate(solved),
  };
}
const add = (name, request) => {
  cases[name] = { request, envelope: calculate(request) };
};
add("theta_trading_days", {
  ...terms.black_scholes,
  thetaDaysPerYear: 252,
  mode: "price",
  vol: vols.black_scholes,
});
// Rust prices a zero volatility and rejects its Greeks.
add("zero_vol_rejects_greeks", {
  ...terms.black_scholes,
  mode: "price",
  vol: 0,
});
// Rust rejects a price below intrinsic value for every model it inverts.
add("below_intrinsic", {
  ...terms.black_scholes,
  spot: 120,
  mode: "implied_vol",
  price: 1,
});
add("bachelier_below_intrinsic", {
  ...terms.bachelier,
  forward: 0.05,
  mode: "implied_vol",
  price: 0.001,
});
// An empty field reaches Rust as NaN and is rejected there.
cases.empty_field = {
  request: { ...terms.black76, df: "NaN", mode: "price", vol: vols.black76 },
  envelope: calculate({
    ...terms.black76,
    df: Number.NaN,
    mode: "price",
    vol: vols.black76,
  }),
};
await emit(new URL("./cases.json", import.meta.url), JSON.stringify(cases));
