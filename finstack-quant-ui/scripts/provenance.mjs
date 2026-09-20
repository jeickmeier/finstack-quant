import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import ts from "typescript";

const base = "https://finstack_quant.dev/schemas/";
const market = `${base}market_data/1/market_context_state.schema.json`;
const result = `${base}results/1/valuation_result.schema.json`;
const calibration = `${base}calibration/1/calibration.schema.json`;
const price = [
  "valuations.instruments.priceInstrument",
  "ValuationInstrumentsNamespace",
  "priceInstrument",
];
const displays = [
  {
    id: "instrument-form",
    schema: "instrument-roots",
    pointer: "#",
    route: "schema",
    convention: "Canonical wire representations; no inferred units.",
  },
  {
    id: "market-context-form",
    schema: market,
    pointer: "#",
    route: "schema",
    convention: "Stored state; no curve reconstruction.",
  },
  {
    id: "calibration-form",
    schema: calibration,
    pointer: "#",
    route: "schema",
    convention:
      "Calibration inputs only; final_market is a returned market state.",
  },
  {
    id: "valuation-summary",
    schema: result,
    pointer: "#/properties/value",
    api: price,
    route: "typed",
    convention: "Exact decimal-string money with its returned currency.",
  },
  {
    id: "stamp-badge",
    schema: result,
    pointer: "#/properties/meta",
    api: price,
    route: "raw",
    convention: "Returned numeric, rounding, FX policy and version stamps.",
  },
  {
    id: "measures-grid",
    schema: result,
    pointer: "#/properties/measures",
    api: price,
    route: "raw",
    convention:
      "Returned values and qualified keys unchanged; unit unavailable.",
    deferred: "machine-readable-units",
  },
  {
    id: "valuation-details",
    schema: result,
    pointer: "#/$defs/ValuationDetails",
    api: price,
    route: "host-declaration",
    convention:
      "Only the published MonteCarloValuationDetails is typed; remaining data is unknown.",
    deferred: "weak-detail-declarations",
  },
  {
    id: "explanation-trace",
    schema: result,
    pointer: "#/properties/explanation",
    api: price,
    route: "raw",
    convention: "Returned trace; no reconciliation or financial arithmetic.",
  },
  {
    id: "covenant-report",
    schema: result,
    pointer: "#/properties/covenants",
    api: price,
    route: "raw",
    convention: "Returned verdicts; no recalculation.",
  },
  {
    id: "cashflow-viewer",
    api: [
      "valuations.instruments.instrumentCashflowsJson",
      "ValuationInstrumentsNamespace",
      "instrumentCashflowsJson",
    ],
    route: "native-json-table",
    convention:
      "CashflowRow presentation adapter from valuations/src/instruments/common_impl/cashflow_export.rs. Lossless tokens, native row currency and reporting-currency PV; Rust supplies total and reconciliation. Original JSON export stays unchanged.",
    deferred: "cashflow-declarations",
  },
  {
    id: "curve-chart",
    schema: market,
    pointer: "#/$defs/CurveState",
    route: "stored-knots-or-fields",
    convention:
      "Original coordinates; base correlation and parametric variants have no synthetic knots.",
    deferred: "full-state-curve-evaluation",
  },
  {
    id: "vol-surface-chart",
    schema: market,
    pointer: "#/properties/surfaces",
    route: "stored-grid",
    convention:
      "Original nodes and quote convention; no off-grid interpolation.",
    deferred: "off-grid-surface-evaluation",
  },
  {
    id: "vol-cube-explorer-black",
    schema: market,
    pointer: "#/properties/vol_cubes",
    api: ["models.volatility.getCubeVol", "VolatilityNamespace", "getCubeVol"],
    route: "typed",
    convention:
      "Annualized Black volatility as decimal; supply stored cube and explicit expiry, tenor, strike.",
  },
  {
    id: "vol-cube-explorer-normal",
    api: [
      "models.volatility.getCubeNormalVol",
      "VolatilityNamespace",
      "getCubeNormalVol",
    ],
    route: "typed",
    convention: "Annualized normal volatility in absolute rate units.",
  },
  {
    id: "fx-delta-surface",
    schema: market,
    pointer: "#/properties/fx_delta_vol_surfaces",
    api: [
      "models.volatility.getFxDeltaVol",
      "VolatilityNamespace",
      "getFxDeltaVol",
    ],
    route: "typed",
    convention:
      "Annualized Black volatility as decimal; forward is explicit caller input.",
  },
  {
    id: "fx-delta-pillars",
    api: [
      "models.volatility.getFxDeltaPillarVols",
      "VolatilityNamespace",
      "getFxDeltaPillarVols",
    ],
    route: "typed-array",
    convention:
      "Float64Array: ATM, 25-delta put, 25-delta call annualized decimal vols.",
  },
  {
    id: "fx-matrix-grid",
    schema: market,
    pointer: "#/$defs/FxMatrixState",
    route: "stored-fields",
    convention: "Stored quotes only; no implied cross or local FX calculation.",
  },
  {
    id: "market-context-browser",
    schema: market,
    pointer: "#",
    route: "stored-fields",
    convention:
      "Schema-owned market sections; raw values where no unit metadata exists.",
  },
  {
    id: "calibration-report",
    api: ["calibration.calibrate", "CalibrationNamespace", "calibrate"],
    route: "returned-report",
    convention:
      "Returned reports and final_market; preserve diagnostics and error metadata.",
  },
  {
    id: "calibration-fit-chart",
    api: ["calibration.calibrate", "CalibrationNamespace", "calibrate"],
    route: "returned-residuals",
    convention:
      "Residuals per step in solver units; no quote-space repricing or cross-step aggregation.",
    deferred: "quote-space-calibration-fits",
  },
  {
    id: "scenario-heatmap",
    api: [
      "valuations.instruments.structuredCreditTrancheScenarioTable",
      "ValuationInstrumentsNamespace",
      "structuredCreditTrancheScenarioTable",
    ],
    route: "returned-prices",
    convention:
      "Rust clean settlement price, percent of current tranche balance; facade original-balance wording is upstream drift.",
    deferred: "scenario-price-docs",
  },
];

export async function generateProvenance(repo, contracts, fixtureManifest) {
  const declarationPath = "finstack-quant-wasm/index.d.ts";
  const text = await readFile(resolve(repo, declarationPath), "utf8");
  const ast = ts.createSourceFile(
    declarationPath,
    text,
    ts.ScriptTarget.Latest,
    true,
  );
  const interfaces = new Map(
    ast.statements
      .filter(ts.isInterfaceDeclaration)
      .map((node) => [node.name.text, node]),
  );
  const digest = (value) => createHash("sha256").update(value).digest("hex");
  function apiSource([name, owner, method]) {
    const member = interfaces
      .get(owner)
      ?.members.find((node) => node.name?.getText(ast) === method);
    if (!member || !ts.isMethodSignature(member))
      throw new Error(`Missing published API: ${owner}.${method}`);
    const declaration = member.getFullText(ast).trim();
    return {
      name,
      source: declarationPath,
      owner,
      member: method,
      inputs: member.parameters.map((parameter) => ({
        name: parameter.name.getText(ast),
        type: parameter.type.getText(ast),
        optional: Boolean(parameter.questionToken),
      })),
      returns: member.type.getText(ast),
      declaration,
      sha256: digest(declaration),
    };
  }
  function schemaSource(uri, pointer) {
    const contract = contracts.get(uri);
    if (!contract) throw new Error(`Missing schema: ${uri}`);
    const node =
      pointer === "#"
        ? contract.schema
        : pointer
            .slice(2)
            .split("/")
            .reduce(
              (value, key) =>
                value?.[key.replaceAll("~1", "/").replaceAll("~0", "~")],
              contract.schema,
            );
    if (node === undefined)
      throw new Error(`Missing schema pointer: ${uri}${pointer}`);
    return {
      uri,
      pointer,
      source: contract.source,
      sha256: digest(JSON.stringify(node)),
      description: node.description ?? null,
    };
  }
  const instrumentRoots = [...contracts.keys()]
    .filter((uri) => /\/instrument\/1\/[^/]+\/[^/]+\.schema\.json$/.test(uri))
    .sort();
  const entries = displays.map(({ api, schema, pointer, ...entry }) => ({
    ...entry,
    ...(api ? { api: apiSource(api) } : {}),
    ...(schema
      ? {
          schemas: (schema === "instrument-roots"
            ? instrumentRoots
            : [schema]
          ).map((uri) => schemaSource(uri, pointer)),
        }
      : {}),
  }));
  const curves = contracts
    .get(market)
    .schema.$defs.CurveState.oneOf.map((variant, index) => ({
      type: variant.properties.type.const,
      route: Object.hasOwn(variant.properties, "knot_points")
        ? "stored-knots"
        : "fields",
      fields: Object.keys(variant.properties),
      source: schemaSource(market, `#/$defs/CurveState/oneOf/${index}`),
    }));
  const details = contracts
    .get(result)
    .schema.$defs.ValuationDetails.oneOf.map((variant) => ({
      type: variant.properties.type.const,
      route: variant.properties.type.const === "monte_carlo" ? "typed" : "raw",
    }));
  const hostTypes = [
    "ValuationResult",
    "ValuationDetails",
    "MonteCarloValuationDetails",
    "MoneyValue",
    "TrancheScenarioCell",
    "VolCubeConstructor",
    "FxDeltaVolSurfaceConstructor",
  ];
  const hostDeclarations = hostTypes.map((name) => {
    const node = ast.statements.find(
      (node) => node.name?.getText(ast) === name,
    );
    if (!node) throw new Error(`Missing host type: ${name}`);
    const declaration = node.getFullText(ast).trim();
    return {
      name,
      source: declarationPath,
      declaration,
      sha256: digest(declaration),
    };
  });
  // The scenario field's published TS comment is known to disagree with Rust.
  const scenarioPath =
    "finstack-quant/valuations/src/instruments/fixed_income/structured_credit/metrics/scenario.rs";
  const scenarioSource = await readFile(resolve(repo, scenarioPath), "utf8");
  return {
    inventory: {
      version: 1,
      fixtureManifest: "generated/fixtures.json",
      fixtureManifestSha256: digest(fixtureManifest),
      entries,
      hostDeclarations,
      details,
      scenarioAuthority: {
        source: scenarioPath,
        sha256: digest(scenarioSource),
      },
      deferred: {
        "machine-readable-units":
          "Generic values stay raw; do not infer units from names or decimal refs.",
        "cashflow-declarations":
          "No public typed WASM cashflow return. The viewer validates a presentation-only adapter against canonical Rust CashflowRow and native fixtures; it does not add a native API.",
        "weak-detail-declarations":
          "Unknown host details remain raw. Actual structured-credit num_paths is bigint; the typed Monte Carlo path counts are number. Never infer unknown host shapes from wire integer formats.",
        "full-state-curve-evaluation":
          "Use stored nodes and parameters; no constructor-based curve recreation.",
        "off-grid-surface-evaluation": "Stored nodes only.",
        "quote-space-calibration-fits":
          "Returned residuals only until quote repricing is published.",
        "scenario-price-docs":
          "Rust defines clean settlement percent of current balance.",
        "missing-financial-aggregates":
          "Unavailable unless an existing canonical result supplies the value.",
      },
    },
    curves,
  };
}
