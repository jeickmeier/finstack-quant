// Native StatementResult tables for statement-export: the analyst result in both
// layouts, and a model whose node id needs CSV quoting. Rerun with
// `mise run ui-fixtures` after rebuilding the WASM package.
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { emit } from "../../../scripts/fixture-output.mjs";

const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
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
const quotedId = 'adjusted, "core" revenue';
const quoted = evaluate(
  native.validateFinancialModelJson(
    JSON.stringify({
      ...model,
      nodes: {
        ...model.nodes,
        [quotedId]: { ...model.nodes.revenue, node_id: quotedId },
      },
    }),
  ),
);
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({
    analyst: tables(analyst),
    quotedId,
    quoted: tables(quoted),
  }),
);
