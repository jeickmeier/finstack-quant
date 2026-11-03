// Native statements.normalize results and rejections for normalization-editor,
// over the analyst statement result. Rerun with `mise run ui-fixtures` after
// rebuilding the WASM package.
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { emit } from "../../../scripts/fixture-output.mjs";

const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const resultJson = await readFile(
  new URL(
    "../../../src/fixtures/statements/analyst-result.json",
    import.meta.url,
  ),
  "utf8",
);
const rejection = (run) => {
  try {
    run();
  } catch (error) {
    return error.message;
  }
  throw new Error("Rust accepted a config it should reject");
};
// Adjusted EBITDA: a one-off add-back, run-rate synergies capped at 10% of
// reported EBITDA (capped in some quarters only), management fees under an
// absolute cap and a one-time gain taken out.
const config = {
  target_node: "ebitda",
  adjustments: [
    {
      id: "restructuring",
      name: "Restructuring charges",
      category: "non_recurring",
      value: {
        type: "fixed",
        amounts: { "2024Q1": 4500000, "2024Q3": 2000000 },
      },
    },
    {
      id: "run_rate_synergies",
      name: "Run-rate synergies",
      category: "synergies",
      value: {
        type: "percentage_of_node",
        node_id: "revenue",
        percentage: 0.025,
      },
      cap: { base_node: "ebitda", value: 0.1 },
    },
    {
      id: "management_fees",
      name: "Sponsor management fees",
      value: {
        type: "fixed",
        amounts: {
          "2024Q1": 1200000,
          "2024Q2": 1200000,
          "2024Q3": 1200000,
          "2024Q4": 1200000,
        },
      },
      cap: { base_node: null, value: 1000000 },
    },
    {
      id: "one_time_gain",
      name: "Gain on asset sale",
      value: { type: "fixed", amounts: { "2024Q2": -3000000 } },
    },
  ],
};
const configJson = JSON.stringify(config);
const canonical = native.validateNormalizationConfigJson(configJson);
const results = native.normalize(resultJson, canonical);
const duplicate = JSON.stringify({
  ...config,
  adjustments: [...config.adjustments, config.adjustments[0]],
});
const errors = {
  duplicate: {
    config: JSON.parse(duplicate),
    message: rejection(() => native.validateNormalizationConfigJson(duplicate)),
  },
  missingTarget: {
    config: { ...config, target_node: "adjusted_ebitda" },
    message: rejection(() =>
      native.normalize(resultJson, {
        ...config,
        target_node: "adjusted_ebitda",
      }),
    ),
  },
  // ltm_ebitda has no value (NaN) in its first quarters.
  nonFiniteBase: {
    config: { ...config, target_node: "ltm_ebitda" },
    message: rejection(() =>
      native.normalize(resultJson, { ...config, target_node: "ltm_ebitda" }),
    ),
  },
};
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({ config, canonical, results, errors }),
);
