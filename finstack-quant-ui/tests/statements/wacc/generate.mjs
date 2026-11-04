// Native statements_analytics.wacc rates and rejections for wacc-calculator.
// Rerun with `mise run ui-fixtures` after rebuilding the WASM package.
import { emit } from "../../../scripts/fixture-output.mjs";
import { native } from "../../../scripts/fixture-inputs.mjs";

const rate = (inputs) =>
  native.wacc(
    inputs.equityWeight,
    inputs.costOfEquity,
    inputs.debtWeight,
    inputs.costOfDebt,
    inputs.taxRate,
  );
const rejection = (inputs) => {
  try {
    rate(inputs);
  } catch (error) {
    return error.message;
  }
  throw new Error(`wacc accepted ${JSON.stringify(inputs)}`);
};
const blended = {
  equityWeight: 0.6,
  costOfEquity: 0.115,
  debtWeight: 0.4,
  costOfDebt: 0.06,
  taxRate: 0.25,
};
// A rate of zero is a returned rate, not a missing one.
const allEquityAtZero = {
  equityWeight: 1,
  costOfEquity: 0,
  debtWeight: 0,
  costOfDebt: 0,
  taxRate: 0,
};
const cases = {
  blended: { inputs: blended, rate: rate(blended) },
  zero: { inputs: allEquityAtZero, rate: rate(allEquityAtZero) },
};
const weights = { ...blended, debtWeight: 0.5 };
const tax = { ...blended, taxRate: 1.5 };
const errors = {
  weights: { inputs: weights, message: rejection(weights) },
  tax: { inputs: tax, message: rejection(tax) },
  // What an empty equity weight field submits; JSON has no NaN to store as an input.
  empty: { message: rejection({ ...blended, equityWeight: Number.NaN }) },
};
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({ cases, errors }),
);
