// Native StatementResult tables for statement-export: the analyst result in both
// layouts, and a model whose node id needs CSV quoting. Rerun with
// `mise run ui-fixtures` after rebuilding the WASM package.
import { readFile } from "node:fs/promises";
import { emit } from "../../../scripts/fixture-output.mjs";
import { native } from "../../../scripts/fixture-inputs.mjs";

const fixtures = new URL("../../../src/fixtures/statements/", import.meta.url);
const read = (name) => readFile(new URL(name, fixtures), "utf8");
const evaluate = (modelJson) => {
  const evaluator = new native.Evaluator();
  try {
    return JSON.stringify(evaluator.evaluate(modelJson));
  } finally {
    evaluator.free();
  }
};
// JSON has no NaN, so a cell Rust holds as NaN is stored here as null.
const tables = (resultJson) => ({
  wide: native.statementResultToTableWide(resultJson),
  long: native.statementResultToTableLong(resultJson),
});
const analyst = await read("analyst-result.json");
const model = JSON.parse(await read("analyst-model.json"));
// A model node id is a formula identifier, so it never needs CSV quoting.
// The export still has to quote arbitrary column names: evaluate a valid
// node, then rename it in the native tables.
const quotedId = 'adjusted, "core" revenue';
const validId = "adjusted_core_revenue";
const quoted = evaluate(
  native.validateFinancialModelJson(
    JSON.stringify({
      ...model,
      nodes: {
        ...model.nodes,
        [validId]: { ...model.nodes.revenue, node_id: validId },
      },
    }),
  ),
);
const renamed = (value) =>
  JSON.parse(
    JSON.stringify(value).replaceAll(
      JSON.stringify(validId),
      JSON.stringify(quotedId),
    ),
  );
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    analyst: tables(analyst),
    quotedId,
    quoted: renamed(tables(quoted)),
  }),
);
