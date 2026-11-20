// Native statements_analytics.wacc rates and rejections for wacc-calculator.
// Rerun with `mise run ui-fixtures` after rebuilding the WASM package.
import { emit } from "../../../scripts/fixture-output.mjs";
import { native } from "../../../scripts/fixture-inputs.mjs";

const rate = (inputs) => native.wacc(inputs);
const rejection = (inputs) => {
  try {
    rate(inputs);
  } catch (error) {
    return error.message;
  }
  throw new Error(`wacc accepted ${JSON.stringify(inputs)}`);
};
const blended = {
  equity_weight: "0.6",
  cost_of_equity: 0.115,
  debt_weight: "0.4",
  cost_of_debt: 0.06,
  tax_rate: 0.25,
};
// A rate of zero is a returned rate, not a missing one.
const allEquityAtZero = {
  equity_weight: "1",
  cost_of_equity: 0,
  debt_weight: "0",
  cost_of_debt: 0,
  tax_rate: 0,
};
const exactComplement = {
  ...blended,
  equity_weight: "0.6000000000000000000000000001",
  debt_weight: "0.3999999999999999999999999999",
};
const cases = {
  exactComplement: { inputs: exactComplement, rate: rate(exactComplement) },
  blended: { inputs: blended, rate: rate(blended) },
  zero: { inputs: allEquityAtZero, rate: rate(allEquityAtZero) },
};
const weights = { ...blended, debt_weight: "0.5" };
const tax = { ...blended, tax_rate: 1.5 };
const inexact = { ...blended, equity_weight: exactComplement.equity_weight };
const errors = {
  inexact: { inputs: inexact, message: rejection(inexact) },
  weights: { inputs: weights, message: rejection(weights) },
  tax: { inputs: tax, message: rejection(tax) },
  // An empty weight is forwarded as empty decimal text for native validation.
  empty: { message: rejection({ ...blended, equity_weight: "" }) },
};
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({ cases, errors }),
);
