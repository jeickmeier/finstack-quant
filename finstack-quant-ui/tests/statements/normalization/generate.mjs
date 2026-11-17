// Native statements.normalize results and rejections for normalization-editor,
// over the analyst statement result. Rerun with `mise run ui-fixtures` after
// rebuilding the WASM package.
import { emit } from "../../../scripts/fixture-output.mjs";
import { native, readRepoText } from "../../../scripts/fixture-inputs.mjs";

const resultJson = await readRepoText(
  "finstack-quant-ui/src/fixtures/statements/analyst-result.json",
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
const usd = (amount) => ({ amount: String(amount), currency: "USD" });
const config = {
  target_node: "ebitda",
  output_node: "adjusted_ebitda",
  adjustments: [
    {
      id: "restructuring",
      name: "Restructuring charges",
      category: "non_recurring",
      value: {
        type: "fixed",
        amounts: { "2024Q1": usd(4500000), "2024Q3": usd(2000000) },
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
      cap: { type: "reference", base_node: "ebitda", fraction: 0.1 },
    },
    {
      id: "management_fees",
      name: "Sponsor management fees",
      value: {
        type: "fixed",
        amounts: {
          "2024Q1": usd(1200000),
          "2024Q2": usd(1200000),
          "2024Q3": usd(1200000),
          "2024Q4": usd(1200000),
        },
      },
      cap: { type: "absolute", amount: usd(1000000) },
    },
    {
      id: "one_time_gain",
      name: "Gain on asset sale",
      value: { type: "fixed", amounts: { "2024Q2": usd(-3000000) } },
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
    config: { ...config, target_node: "reported_ebitda" },
    message: rejection(() =>
      native.normalize(resultJson, {
        ...config,
        target_node: "reported_ebitda",
      }),
    ),
  },
  // A fixed amount in the wrong units for the monetary target.
  wrongUnits: {
    config: {
      ...config,
      adjustments: [
        {
          id: "scalar",
          name: "Scalar amount",
          value: { type: "fixed", amounts: { "2024Q1": 1 } },
        },
      ],
    },
    message: rejection(() =>
      native.normalize(resultJson, {
        ...config,
        adjustments: [
          {
            id: "scalar",
            name: "Scalar amount",
            value: { type: "fixed", amounts: { "2024Q1": 1 } },
          },
        ],
      }),
    ),
  },
};
// ltm_ebitda has no value in its first quarters: those periods come back
// unavailable with a reason, the later ones with a bridge.
const unavailable = native.normalize(resultJson, {
  target_node: "ltm_ebitda",
  output_node: "adjusted_ltm_ebitda",
  adjustments: [config.adjustments[0]],
});
await emit(
  new URL("cases.json", import.meta.url),
  JSON.stringify({ config, canonical, results, unavailable, errors }),
);
