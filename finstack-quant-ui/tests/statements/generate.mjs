// Captures the analyst model's native evaluation and check report for
// presentation-only gallery examples. Rerun after rebuilding the WASM package.
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import prettier from "prettier";
import { serializeHost } from "../../src/codec.mjs";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const fixtures = new URL("../../src/fixtures/statements/", import.meta.url);
const read = (name) => readFile(new URL(name, fixtures), "utf8");
const write = async (name, json) =>
  writeFile(
    new URL(name, fixtures),
    await prettier.format(json, { parser: "json" }),
  );

const modelJson = native.validateFinancialModelJson(
  await read("analyst-model.json"),
);
const evaluator = new native.Evaluator();
const resultJson = serializeHost(evaluator.evaluate(modelJson));
evaluator.free();
const reportJson = serializeHost(
  native.runChecks(
    modelJson,
    native.validateCheckSuiteSpecJson(await read("analyst-checks.json")),
    resultJson,
  ),
);
await write("analyst-result.json", resultJson);
await write("analyst-check-report.json", reportJson);
