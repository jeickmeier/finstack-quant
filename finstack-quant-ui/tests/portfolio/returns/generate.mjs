import { emit } from "../../../scripts/fixture-output.mjs";
import { native } from "../../../scripts/fixture-inputs.mjs";
// Host data, as a performance system would supply it. Linked: three annual
// sub-period returns over a three-year horizon, and three monthly returns over
// a quarter (0.25 of a 365-day year), which Rust reports cumulatively. Dietz:
// the GIPS Handbook period, a 1,000,000 contribution with 60% of the period
// remaining. XIRR: two contributions and the terminal value, investor signs.
const inputs = {
  linked: { returns: [0.08, -0.03, 0.11], horizonYears: 3 },
  dietz: {
    beginning_market_value: 10_000_000,
    ending_market_value: 10_500_000,
    cashflows: [{ amount: 1_000_000, fraction_of_period_remaining: 0.6 }],
  },
  xirr: [
    { date: "2023-01-03", amount: -1_000_000 },
    { date: "2023-07-03", amount: -250_000 },
    { date: "2024-12-31", amount: 1_400_000 },
  ],
};
const quarter = { returns: [0.012, -0.004, 0.009], horizonYears: 0.25 };
const withdrawal = {
  beginning_market_value: 5_000_000,
  ending_market_value: 4_700_000,
  cashflows: [{ amount: -400_000, fraction_of_period_remaining: 0.25 }],
};
const calls = {
  linked: (request) => native.twrrLinked(request.returns, request.horizonYears),
  dietz: (period) => native.twrrModifiedDietz(period),
  xirr: (cashflows) => native.mwrXirr(cashflows),
};
const success = (form, request) => ({ request, result: calls[form](request) });
function failure(form, request) {
  try {
    calls[form](request);
  } catch (error) {
    return {
      request,
      error: { name: error.name, kind: error.kind, message: error.message },
    };
  }
  throw new Error(`Expected a native ${form} error`);
}
const cases = {
  linked: {
    annual: success("linked", inputs.linked),
    quarter: success("linked", quarter),
  },
  dietz: {
    contribution: success("dietz", inputs.dietz),
    withdrawal: success("dietz", withdrawal),
    noFlows: success("dietz", {
      beginning_market_value: 100,
      ending_market_value: 110,
      cashflows: [],
    }),
  },
  xirr: { conventional: success("xirr", inputs.xirr) },
};
const errors = {
  linked: {
    // A complete loss leaves the linked return undefined.
    completeLoss: failure("linked", { returns: [0.05, -1], horizonYears: 1 }),
  },
  dietz: {
    // A weight outside [0, 1] is never clamped.
    fraction: failure("dietz", {
      ...inputs.dietz,
      cashflows: [{ amount: 1_000_000, fraction_of_period_remaining: 1.2 }],
    }),
    // A redemption of the whole book leaves no positive denominator.
    denominator: failure("dietz", {
      beginning_market_value: 1_000_000,
      ending_market_value: 0,
      cashflows: [{ amount: -1_000_000, fraction_of_period_remaining: 1 }],
    }),
  },
  xirr: {
    // Two net sign changes: Rust does not pick one of several roots.
    twoSignChanges: failure("xirr", [
      { date: "2023-01-03", amount: -1_000_000 },
      { date: "2023-12-29", amount: 2_300_000 },
      { date: "2024-12-31", amount: -1_320_000 },
    ]),
    // A contribution alone has no return.
    oneFlow: failure("xirr", [{ date: "2023-01-03", amount: -1_000_000 }]),
  },
};
// An empty field is submitted as NaN, which the WASM boundary rejects. JSON
// has no NaN, so only the error is stored; the test sends [0.05, NaN] itself.
const emptyField = failure("linked", {
  returns: [0.05, Number.NaN],
  horizonYears: 1,
}).error;
// A blank horizon field is submitted as NaN and one past the float range
// ("1e999") as Infinity; Rust rejects both rather than reporting a cumulative
// or a zero annualised return. Only the errors are stored, as above.
const nonFiniteHorizon = {
  blank: failure("linked", { ...inputs.linked, horizonYears: Number.NaN })
    .error,
  overflow: failure("linked", { ...inputs.linked, horizonYears: Infinity })
    .error,
};
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    description:
      "Native portfolio.twrrLinked, twrrModifiedDietz and mwrXirr results of host-supplied returns, one period's values and flows, and dated investor cash flows, with the native errors of inputs Rust rejects, including a blank (NaN) and an overflowing (Infinity) horizon.",
    inputs,
    cases,
    errors,
    emptyField,
    nonFiniteHorizon,
  }),
);
