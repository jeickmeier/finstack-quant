import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import ts from "typescript";
import { schemaAt } from "../src/schema.mjs";
import { byCodeUnit } from "./order.mjs";

const base = "https://finstack_quant.dev/schemas/";
const market = `${base}market_data/1/market_context_state.schema.json`;
const result = `${base}results/1/valuation_result.schema.json`;
const calibration = `${base}calibration/1/calibration.schema.json`;
const price = [
  "valuations.instruments.priceInstrument",
  "ValuationInstrumentsNamespace",
  "priceInstrument",
];
const metricMetadata = [
  "valuations.instruments.metricMetadata",
  "ValuationInstrumentsNamespace",
  "metricMetadata",
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
    api: metricMetadata,
    route: "native-metadata",
    convention:
      "Returned values and original keys unchanged; units, decoded coordinates, and groups come from Rust.",
  },
  {
    id: "valuation-details",
    schema: result,
    pointer: "#/$defs/ValuationDetails",
    api: price,
    route: "host-declaration",
    convention:
      "All detail variants use Rust-derived WASM host declarations and validation metadata.",
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
    route: "typed",
    convention:
      "Returned CovenantReport fields keyed by covenant instance key, in the order supplied: test description, actual, threshold and headroom in the metric's own units, the passed verdict and details; unset fields shown as a dash, complete reports kept as JSON. No verdict, headroom or threshold is computed.",
  },
  {
    id: "covenant-compliance",
    schema: `${base}covenants/1/dated_covenant_reports.schema.json`,
    pointer: "#",
    api: [
      "covenants.CovenantEngine#evaluateSeries",
      "CovenantEngine",
      "evaluateSeries",
    ],
    route: "typed",
    convention:
      "Returned DatedCovenantReports rows in input order and their verdicts; a covenant without a report on a date shows a dash. Statement rows come from statements.nodeToDatedSchedule per metric (period end), zipped by period from a chosen first period, with a non-finite value dropping that metric from its row; covenant windows decide which metrics a date requires.",
  },
  {
    id: "cashflow-viewer",
    schema: `${base}results/1/instrument_cashflow.schema.json`,
    pointer: "#",
    api: [
      "valuations.instruments.instrumentCashflows",
      "ValuationInstrumentsNamespace",
      "instrumentCashflows",
    ],
    route: "native-envelope-table",
    convention:
      "The returned envelope object, validated against the Rust-generated cashflow schema. Native row currency and reporting-currency PV; Rust supplies total and reconciliation. Its JSON view is a host serialization, not native text. The optional annual ladder is cashflows.calendarYearLadder per row currency (worker partition, first-appearance order) over each flow's native_pv: row-currency PV before FX conversion and any scenario price shock.",
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
      "Annualized Black volatility as decimal; supply stored cube and explicit expiry, tenor, strike. Fixed-tenor and fixed-expiry slices are Rust-materialized (vol-cube-tenor-slice, vol-cube-expiry-slice).",
  },
  {
    id: "vol-cube-explorer-normal",
    api: [
      "models.volatility.getCubeNormalVol",
      "VolatilityNamespace",
      "getCubeNormalVol",
    ],
    route: "typed",
    convention:
      "Annualized normal volatility in absolute rate units. Normal slices are Rust-materialized (vol-cube-tenor-slice-normal, vol-cube-expiry-slice-normal).",
  },
  {
    id: "vol-cube-tenor-slice",
    api: [
      "models.volatility.materializeCubeTenorSlice",
      "VolatilityNamespace",
      "materializeCubeTenorSlice",
    ],
    route: "returned-surface",
    convention:
      "Returned VolSurface JSON (expiry by strike, Black or shifted-Black quote type with displacements as Rust returns them); fixed tenor in years and absolute strikes in forward-rate units are explicit caller input. Off-node tenors are Rust-interpolated.",
  },
  {
    id: "vol-cube-tenor-slice-normal",
    api: [
      "models.volatility.materializeCubeTenorSliceNormal",
      "VolatilityNamespace",
      "materializeCubeTenorSliceNormal",
    ],
    route: "returned-surface",
    convention:
      "Returned VolSurface JSON of normal volatilities (expiry by strike); fixed tenor and absolute strikes are explicit caller input. Off-node tenors are Rust-interpolated.",
  },
  {
    id: "vol-cube-expiry-slice",
    api: [
      "models.volatility.materializeCubeExpirySlice",
      "VolatilityNamespace",
      "materializeCubeExpirySlice",
    ],
    route: "returned-slice",
    convention:
      "Returned VolCubeExpirySlice JSON (tenor by strike, tenor-major, Black or shifted-Black quote type with displacements as Rust returns them); fixed expiry in years and absolute strikes are explicit caller input. Off-node expiries are Rust-interpolated.",
  },
  {
    id: "vol-cube-expiry-slice-normal",
    api: [
      "models.volatility.materializeCubeExpirySliceNormal",
      "VolatilityNamespace",
      "materializeCubeExpirySliceNormal",
    ],
    route: "returned-slice",
    convention:
      "Returned VolCubeExpirySlice JSON of normal volatilities (tenor by strike); fixed expiry and absolute strikes are explicit caller input. Off-node expiries are Rust-interpolated.",
  },
  {
    id: "vol-cube-node-smile",
    api: ["models.volatility.SabrSmile", "SabrSmile", "generateSmile"],
    route: "returned-smile",
    convention:
      "SabrSmile of one stored cube node (paramsAt, forwardAt, the node expiry): normal volatility when beta < 1e-4, Black otherwise, as returned. validateNoArbitrage takes an explicit continuously compounded decimal rate; repairArbitrage runs only when the caller supplies its passes. Never routed through a surface check.",
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
      "Annualized Black volatility as decimal; forward is explicit caller input for evaluated samples. The strike grid is Rust-materialized (fx-strike-grid).",
  },
  {
    id: "fx-strike-grid",
    api: [
      "models.volatility.materializeFxDeltaSurface",
      "VolatilityNamespace",
      "materializeFxDeltaSurface",
    ],
    route: "returned-surface",
    convention:
      "Returned VolSurface JSON (expiry by strike, Rust quote type and interpolation metadata); spot domestic per unit of foreign and continuously compounded decimal rates are explicit caller input, never defaulted or looked up.",
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
    id: "market-scenario-report",
    api: ["scenarios.applyScenario", "ScenariosNamespace", "applyScenario"],
    route: "returned-report",
    convention:
      "Returned ApplicationReport counters, applied shocks, time roll with carry and warnings; nothing summed or converted. The workbench prices the shocked market at the returned valuation date with the base model and metrics and shows both valuations; no P&L is computed.",
  },
  {
    id: "scenario-heatmap",
    api: [
      "valuations.instruments.trancheScenarioTable",
      "ValuationInstrumentsNamespace",
      "trancheScenarioTable",
    ],
    route: "returned-prices",
    convention:
      "Rust clean settlement price, percent of current tranche balance.",
  },
  {
    id: "tranche-analytics",
    api: [
      "valuations.instruments.trancheMetrics",
      "ValuationInstrumentsNamespace",
      "trancheMetrics",
    ],
    route: "returned-metrics",
    convention:
      "Returned TrancheMetrics fields in the units each Rust field documents; break-even CDR, discount margin and OAS as returned decimals; prices percent of current tranche balance; no price default.",
  },
  {
    id: "performance-report",
    api: ["analytics.Performance#summary", "Performance", "summary"],
    route: "returned-analytics",
    deferred: "machine-readable-units",
    convention:
      "Returned analytics.Performance results of a host-supplied panel: the summary TableEnvelope, per-ticker series against each ticker's own active dates, rolling series, monthly and annual periodic returns, drawdown episodes and lookback returns (FYTD from a core.FiscalConfig only when a fiscal-year start is supplied). Decimal fractions and ratios as returned, no unit inferred; omitted options are left to Rust; non-finite values are left out of figures. The monthly heatmap colour domain is the caller's or the returned values' extent (presentation only).",
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
  // Published owners: namespace interfaces and declared handle classes (such
  // as analytics.Performance, merged with a same-named interface).
  const owners = Map.groupBy(
    ast.statements.filter(
      (node) =>
        (ts.isInterfaceDeclaration(node) || ts.isClassDeclaration(node)) &&
        node.name,
    ),
    (node) => node.name.text,
  );
  const digest = (value) => createHash("sha256").update(value).digest("hex");
  function apiSource([name, owner, method]) {
    const member = owners
      .get(owner)
      ?.flatMap((node) => [...node.members])
      .find((node) => node.name?.getText(ast) === method);
    if (
      !member ||
      !(ts.isMethodSignature(member) || ts.isMethodDeclaration(member))
    )
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
    const node = schemaAt(contract.schema, pointer);
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
    .sort(byCodeUnit);
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
    "VolCubeConstructor",
    "FxDeltaVolSurfaceConstructor",
    "SabrSmileConstructor",
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
  // Retain the native scenario authority alongside the matching facade declaration.
  const scenarioPath =
    "finstack-quant/valuations/structured-credit/src/structured_credit/metrics/scenario.rs";
  const scenarioSource = await readFile(resolve(repo, scenarioPath), "utf8");
  return {
    inventory: {
      version: 1,
      fixtureManifest: "generated/fixtures.json",
      fixtureManifestSha256: digest(fixtureManifest),
      entries,
      hostDeclarations,
      hostContract: {
        source:
          "finstack-quant-wasm/schemas/host/1/valuation_result.schema.json",
        sha256: digest(
          await readFile(
            resolve(
              repo,
              "finstack-quant-wasm/schemas/host/1/valuation_result.schema.json",
            ),
          ),
        ),
      },
      details,
      scenarioAuthority: {
        source: scenarioPath,
        sha256: digest(scenarioSource),
      },
      deferred: {
        "machine-readable-units":
          "Generic values stay raw; do not infer units from names or decimal refs.",
        "full-state-curve-evaluation":
          "Use stored nodes and parameters; no constructor-based curve recreation.",
        "off-grid-surface-evaluation":
          "Stored nodes only; off-grid values come only from native evaluators or a Rust-materialized grid (fx-strike-grid, vol-cube-tenor-slice, vol-cube-expiry-slice).",
        "quote-space-calibration-fits":
          "Returned residuals only until quote repricing is published.",
        "missing-financial-aggregates":
          "Unavailable unless an existing canonical result supplies the value.",
      },
    },
    curves,
  };
}
