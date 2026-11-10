// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRequire } from "node:module";
import model from "../../src/fixtures/statements/analyst-model.json";
import rolledQ2 from "../../src/fixtures/statements/roll-forward-2025Q2.json";
import reportedQ4 from "../../src/fixtures/statements/analyst-reported-2025Q4.json";
import covenantEngine from "../../src/fixtures/covenants/analyst-covenants.json";
import covenantSeries from "../../src/fixtures/covenants/analyst-covenant-series.json";
import { StatementsWorkbench } from "@/components/finstack/statements/blocks/statements-workbench/statements-workbench";
import { reportStatementCell } from "@/components/finstack/statements/components/statement-grid/projection";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import waccCases from "./wacc/cases.json";
import capitalStructure from "./capital-structure/cases.json";
import { budget } from "../shared/budget";
import { formatRawMoney } from "../../src/format/format";
import { createClient } from "../shared/worker/client";
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../shared/worker/client"),
);
vi.mock(
  "@/components/finstack/statements/components/statement-diagnostics/statement-explanation",
  () => ({ StatementExplanation: () => null }),
);
vi.mock(
  "@/components/finstack/statements/components/statement-chart/statement-chart",
  () => ({ StatementChart: () => null }),
);
// Figures are drawn by the shared chart in the browser gates; here each one
// is a labelled placeholder.
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
let call: ReturnType<typeof vi.fn>;
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  call = vi.fn(async (method: string, arg: any) => {
    switch (method) {
      case "initialize":
        return { state: "ready", worker: true };
      case "validateStatementModel":
        return native.validateFinancialModelJson(arg);
      case "validateNormalizationConfig":
        return native.validateNormalizationConfigJson(arg);
      case "normalize":
        return native.normalize(arg.resultJson, arg.configJson);
      case "statementResultToTableWide":
        return native.statementResultToTableWide(arg);
      case "statementResultToTableLong":
        return native.statementResultToTableLong(arg);
      case "wacc":
        return native.wacc(arg);
      case "evaluateStatement":
        return new native.Evaluator().evaluate(arg.modelJson);
      case "validateStatementFormula":
        native.parseAndCompile(arg);
        return native.parseFormula(arg);
      case "defaultRollupSpec":
        return native.rollupSpecDeclaredMethods(arg.modelJson, arg.frequency);
      case "rollupStatement":
        return native.statementResultRollup(
          arg.resultsJson,
          arg.modelJson,
          arg.specJson,
        );
      case "evaluateStatementScenarios":
        return native.evaluateScenarioSet(arg.modelJson, arg.scenarioSetJson);
      case "traceStatementScenario":
        return native.scenarioSetTrace(arg.scenarioSetJson, arg.scenario);
      case "diffStatementScenarios":
        return arg.pairs.map((pair: { baseline: string; comparison: string }) =>
          native.scenarioDiff(
            arg.scenarioSetJson,
            arg.resultsJson,
            pair.baseline,
            pair.comparison,
            arg.metrics,
            arg.periods,
          ),
        );
      case "creditAssessment":
        return native.creditAssessment(arg.resultsJson, arg.period);
      case "plSummaryReport":
        return native.plSummaryReport(
          arg.resultsJson,
          arg.lineItems,
          arg.periods,
        );
      case "evaluateDcf":
        return native.evaluateDcf(
          arg.modelJson,
          {
            wacc: arg.wacc,
            terminal_value: arg.terminalValue,
            ufcf_node: arg.ufcfNode,
            net_debt_override: arg.netDebtOverride,
          },
          arg.optionsJson,
          arg.marketJson,
          arg.asOf,
        );
      case "dcfSensitivity":
        return native.dcfSensitivity(
          arg.modelJson,
          arg.wacc,
          arg.terminalValue,
          arg.ufcfNode,
          arg.netDebtOverride,
          arg.optionsJson,
        );
      case "evaluateLbo":
        return native.evaluateLbo(arg.modelJson, arg.configJson);
      case "capitalStructureSummary":
        return native.capitalStructureSummary(
          arg.resultJson,
          arg.modelJson,
          arg.configJson,
        );
      case "runSensitivity": {
        const result = native.runSensitivity(arg.modelJson, arg.config);
        return {
          result,
          tornado: native.generateTornadoEntries(
            result,
            arg.targetMetric,
            arg.period,
          ),
        };
      }
      case "runVariance":
        return native.runVariance(
          arg.baseJson,
          arg.comparisonJson,
          arg.configJson,
        );
      case "varianceBridge":
        return native.varianceBridge(
          arg.baseJson,
          arg.comparisonJson,
          arg.targetMetric,
          arg.period,
          arg.drivers,
          arg.baselineLabel,
          arg.comparisonLabel,
        );
      case "backtestForecast":
        return native.backtestForecast(arg.actual, arg.forecast);
      case "evaluateMonteCarlo":
        return new native.Evaluator().evaluateMonteCarlo(
          arg.modelJson,
          arg.configJson,
        );
      case "monteCarloBreachProbability":
        return (
          native.monteCarloBreachProbability(
            arg.resultsJson,
            arg.metric,
            arg.threshold,
          ) ?? null
        );
      case "covenantTemplate":
        return native.covenantTemplate(arg);
      case "validateCovenantEngine":
        return native.validateCovenantEngineJson(arg);
      case "evaluateCovenants": {
        const engine = native.CovenantEngine.fromJson(arg.engineJson);
        try {
          return native.evaluateCovenants(arg.resultsJson, engine);
        } finally {
          engine.free();
        }
      }
      case "forecastBreaches": {
        const engine = native.CovenantEngine.fromJson(arg.engineJson);
        try {
          return native.statementsAnalyticsForecastBreaches(
            arg.resultsJson,
            engine,
            arg.configJson,
          );
        } finally {
          engine.free();
        }
      }
      case "goalSeek":
        return native.goalSeek(arg.modelJson, {
          target_node: arg.targetNode,
          target_period: arg.targetPeriod,
          target_value: arg.targetValue,
          driver_node: arg.driverNode,
          driver_period: arg.driverPeriod,
          update_model: arg.updateModel,
          bounds: arg.bounds,
        });
      default:
        throw new Error(`Unexpected worker call: ${method}`);
    }
  });
  createClient.mockReset().mockReturnValue({ call, close: vi.fn() });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it(
  "opens complete statements, applies an explicit edit, and rolls forward from a supplied model",
  async () => {
    const accepted = vi.fn();
    const rollForward = vi.fn(async () => JSON.stringify(rolledQ2));
    render(
      <FinstackQueryProvider>
        <StatementsWorkbench
          defaultModelJson={JSON.stringify(model)}
          onModelAccepted={accepted}
          onRollForward={rollForward}
        />
      </FinstackQueryProvider>,
    );
    const income = await screen.findByRole("region", {
      name: "Income statement",
    });
    expect(within(income).getByRole("row", { name: /Revenue/ })).toBeTruthy();
    expect(within(income).getByText("USD in millions")).toBeTruthy();
    expect(within(income).getByText("Cell source:")).toBeTruthy();
    expect(screen.getByText("Explain selected value")).toBeTruthy();
    expect(
      within(income).getByRole("columnheader", { name: "2024Q1 Historical" }),
    ).toBeTruthy();
    expect(
      within(income).getByRole("columnheader", { name: "2025Q2 Forecast" }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "2024Q1 Historical" }));
    expect(
      screen
        .getByRole("button", { name: "2024Q1 Historical" })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      screen.getAllByText("2024Q1 · display in millions").length,
    ).toBeGreaterThan(0);
    fireEvent.click(
      within(income).getByRole("button", {
        name: "Revenue 2024Q1, value source, edit value",
      }),
    );
    const exactValue = screen.getByRole("textbox", {
      name: "Revenue 2024Q1 exact value",
    });
    expect(document.activeElement).toBe(exactValue);
    fireEvent.change(exactValue, {
      target: { value: "190000000" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "Save Revenue 2024Q1" }),
    );
    await waitFor(() => expect(accepted).toHaveBeenCalledTimes(2));
    expect(
      JSON.parse(accepted.mock.lastCall![0]).nodes.revenue.values["2024Q1"]
        .amount,
    ).toBe("190000000");
    await screen.findByRole("button", {
      name: "Revenue 2024Q1, value source, edit value",
    });
    expect(
      screen.getByRole("button", {
        name: "Revenue 2024Q1, value source, edit value",
      }).textContent,
    ).toBe("■190.0");
    expect(
      call.mock.calls.filter(([method]) => method === "evaluateStatement"),
    ).toHaveLength(accepted.mock.calls.length);
    fireEvent.click(
      screen.getByRole("tab", { name: "Forecast & roll forward" }),
    );
    await screen.findByRole("button", { name: "Roll forward to 2025Q2" });
    fireEvent.click(
      screen.getByRole("button", { name: "Roll forward to 2025Q2" }),
    );
    await waitFor(() => expect(rollForward).toHaveBeenCalledTimes(1));
    await screen.findByRole("button", { name: "Roll forward to 2025Q3" });
    expect(
      screen.getByRole("button", { name: "2025Q2 Historical" }),
    ).toBeTruthy();
  },
  budget(30000),
);

it(
  "changes a native forecast method and calculates a newly added metric",
  async () => {
    const accepted = vi.fn();
    render(
      <FinstackQueryProvider>
        <StatementsWorkbench
          defaultModelJson={JSON.stringify(model)}
          onModelAccepted={accepted}
        />
      </FinstackQueryProvider>,
    );
    await screen.findByRole("region", { name: "Income statement" });
    fireEvent.click(
      screen.getByRole("tab", { name: "Forecast & roll forward" }),
    );
    await userEvent.click(
      screen.getByRole("combobox", { name: "Revenue forecast method" }),
    );
    await userEvent.click(
      await screen.findByRole("option", { name: "Fixed growth" }),
    );
    const revenueEditor = screen.getByRole("region", {
      name: "Revenue forecast",
    });
    fireEvent.change(
      within(revenueEditor).getByRole("textbox", { name: /^Rate/ }),
      { target: { value: "0.1" } },
    );
    const applyForecast = within(revenueEditor).getByRole<HTMLButtonElement>(
      "button",
      { name: "Apply forecast" },
    );
    await waitFor(() => expect(applyForecast.disabled).toBe(false));
    fireEvent.click(applyForecast);
    await waitFor(() => expect(accepted).toHaveBeenCalledTimes(2));
    const forecastModel = JSON.parse(accepted.mock.lastCall![0]);
    expect(forecastModel.nodes.revenue.forecast).toEqual({
      method: "growth_pct",
      params: { rate: 0.1 },
    });
    fireEvent.click(screen.getByRole("tab", { name: "Metrics & ratios" }));
    fireEvent.click(screen.getByRole("button", { name: "Add metric" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Metric name" }), {
      target: { value: "Revenue to debt" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Metric ID" }), {
      target: { value: "revenue_to_debt" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: "Formula" }), {
      target: { value: "revenue / total_debt" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add and calculate" }));
    await waitFor(() => expect(accepted).toHaveBeenCalledTimes(3));
    const metricModel = JSON.parse(accepted.mock.lastCall![0]);
    expect(metricModel.nodes.revenue_to_debt).toMatchObject({
      node_type: "calculated",
      formula_text: "revenue / total_debt",
      value_type: { type: "scalar" },
      rollup: "formula",
    });
    const nativeResult = new native.Evaluator().evaluate(
      accepted.mock.lastCall![0],
    );
    expect(nativeResult.nodes.revenue_to_debt["2024Q1"]).toBeCloseTo(
      182400000 / 410000000,
    );
    const ratios = await screen.findByRole("region", {
      name: "Metrics, margins & covenant ratios",
    });
    expect(
      within(ratios).getByText("USD in millions · ratios in native units"),
    ).toBeTruthy();
    expect(
      within(ratios).getByRole("row", { name: /Revenue to debt/ }),
    ).toBeTruthy();
    expect(
      within(ratios).getByRole("button", {
        name: "Revenue to debt 2024Q1, formula source, inspect value",
      }).textContent,
    ).toBe("ƒ0.445");
    fireEvent.click(screen.getByRole("tab", { name: "Statements & editing" }));
    fireEvent.click(
      screen.getByRole("button", {
        name: "Revenue 2024Q1, value source, edit value",
      }),
    );
    fireEvent.change(
      screen.getByRole("textbox", { name: "Revenue 2024Q1 exact value" }),
      { target: { value: "190000000" } },
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Save Revenue 2024Q1" }),
    );
    await waitFor(() => expect(accepted).toHaveBeenCalledTimes(4));
    fireEvent.click(screen.getByRole("tab", { name: "Metrics & ratios" }));
    const refreshedRatios = await screen.findByRole("region", {
      name: "Metrics, margins & covenant ratios",
    });
    await waitFor(() =>
      expect(
        within(refreshedRatios).getByRole("button", {
          name: "Revenue to debt 2024Q1, formula source, inspect value",
        }).textContent,
      ).toBe("ƒ0.463"),
    );
  },
  budget(30000),
);

const canonicalModel = () =>
  native.validateFinancialModelJson(JSON.stringify(model));
/** Text of the elements a control's aria-describedby references. */
const description = (element: HTMLElement) =>
  (element.getAttribute("aria-describedby") ?? "")
    .split(" ")
    .map((id) => document.getElementById(id)?.textContent)
    .join(" ");
const lastCall = (method: string) =>
  call.mock.calls.findLast(([name]) => name === method)?.[1];

it("assesses the active period's credit and summarises the income lines natively in Metrics & ratios", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
    </FinstackQueryProvider>,
  );
  fireEvent.click(await screen.findByRole("tab", { name: "Metrics & ratios" }));
  const report = await screen.findByRole("region", {
    name: "Credit assessment",
  });
  // The last reported actual period is active until the user picks another.
  await waitFor(() =>
    expect(
      within(report).getByRole("heading", {
        level: 3,
        name: /Credit assessment at/,
      }).textContent,
    ).toBe("Credit assessment at 2025Q1"),
  );
  const resultsJson = lastCall("creditAssessment").resultsJson;
  expect(lastCall("creditAssessment")).toEqual({
    resultsJson,
    period: "2025Q1",
  });
  const income = Object.entries(model.nodes)
    .filter(([, node]) =>
      (node as { tags?: string[] }).tags?.includes("income_statement"),
    )
    .map(([id]) => id);
  await waitFor(() =>
    expect(lastCall("plSummaryReport")).toEqual({
      resultsJson,
      lineItems: income,
      periods: model.periods.map((period) => period.id),
    }),
  );
  expect(
    within(report)
      .getByRole("region", { name: "Free cash flow (USD)" })
      .querySelector("p")!.textContent,
  ).toBe(
    String(
      native.creditAssessment(resultsJson, "2025Q1").free_cash_flow,
    ).replace(/\B(?=(\d{3})+(?!\d))/g, ","),
  );
  const pl = await within(report).findByRole("table", {
    name: "P&L summary by line item and period",
  });
  expect(within(pl).getAllByRole("row")).toHaveLength(
    income.length * model.periods.length + 1,
  );
  fireEvent.click(
    within(screen.getByRole("group", { name: "Reporting periods" })).getByRole(
      "button",
      { name: /2025Q3/ },
    ),
  );
  await waitFor(() =>
    expect(lastCall("creditAssessment")).toEqual({
      resultsJson,
      period: "2025Q3",
    }),
  );
});

it("shows a native P&L summary error in Metrics & ratios instead of hiding the summary", async () => {
  const forward = call.getMockImplementation() as (
    method: string,
    arg: unknown,
  ) => Promise<unknown>;
  call.mockImplementation(async (method: string, arg: unknown) =>
    method === "plSummaryReport"
      ? Promise.reject(new Error("Validation error: bad line item"))
      : forward(method, arg),
  );
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
    </FinstackQueryProvider>,
  );
  fireEvent.click(await screen.findByRole("tab", { name: "Metrics & ratios" }));
  const summary = await screen.findByRole("region", { name: "P&L summary" });
  await waitFor(() =>
    expect(within(summary).getByRole("alert").textContent).toBe(
      "Validation error: bad line item",
    ),
  );
});

it(
  "exports the result the grids show, calling Rust only once opened",
  async () => {
    render(
      <FinstackQueryProvider>
        <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
      </FinstackQueryProvider>,
    );
    const summary = await screen.findByText("Export table and CSV");
    const tableCalls = () =>
      call.mock.calls.filter(([method]) =>
        String(method).startsWith("statementResultToTable"),
      );
    expect(tableCalls()).toHaveLength(0);
    const details = summary.closest("details")!;
    details.open = true;
    fireEvent(details, new Event("toggle"));
    const region = await screen.findByRole(
      "region",
      { name: "Statement export" },
      { timeout: budget(10000) },
    );
    await within(region).findByText(
      "Statement result, wide layout",
      undefined,
      {
        timeout: budget(10000),
      },
    );
    expect(tableCalls()).toHaveLength(1);
    const quarterly = within(region).getAllByRole("row").length;
    // Annual: the export follows the native rollup, as the grids do.
    fireEvent.click(screen.getByRole("button", { name: "Annual" }));
    await waitFor(
      () => {
        const annual = within(
          screen.getByRole("region", { name: "Statement export" }),
        ).getAllByRole("row").length;
        expect(annual).toBeLessThan(quarterly);
        expect(tableCalls().length).toBeGreaterThan(1);
      },
      { timeout: budget(10000) },
    );
  },
  budget(30000),
);

it(
  "normalizes the reported result in Metrics & ratios, calling Rust only once opened",
  async () => {
    render(
      <FinstackQueryProvider>
        <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
      </FinstackQueryProvider>,
    );
    fireEvent.click(
      await screen.findByRole("tab", { name: "Metrics & ratios" }),
    );
    const summary = await screen.findByText(
      "Adjusted EBITDA and normalization",
    );
    const calls = (name: string) =>
      call.mock.calls.filter(([method]) => method === name);
    expect(calls("validateNormalizationConfig")).toHaveLength(0);
    expect(calls("normalize")).toHaveLength(0);
    const details = summary.closest("details")!;
    details.open = true;
    fireEvent(details, new Event("toggle"));
    const region = await screen.findByRole(
      "region",
      { name: "Normalization" },
      { timeout: budget(10000) },
    );
    fireEvent.change(
      within(region).getByRole("combobox", { name: "Target node" }),
      { target: { value: "ebitda" } },
    );
    await waitFor(
      () =>
        expect(
          calls("validateNormalizationConfig").some(
            ([, json]) => JSON.parse(json).target_node === "ebitda",
          ),
        ).toBe(true),
      { timeout: budget(10000) },
    );
    expect(calls("normalize")).toHaveLength(0);
    fireEvent.click(within(region).getByRole("button", { name: "Normalize" }));
    const table = await within(region).findByRole(
      "table",
      { name: "Adjustments by period (USD)" },
      { timeout: budget(10000) },
    );
    const request = lastCall("normalize");
    expect(JSON.parse(request.configJson).target_node).toBe("ebitda");
    const returned = native.normalize(request.resultJson, request.configJson);
    // No adjustments: the base and final rows, one column per returned period.
    const rows = within(table).getAllByRole("row");
    expect(rows).toHaveLength(3);
    expect(within(rows[0]!).getAllByRole("columnheader")).toHaveLength(
      returned.length + 1,
    );
  },
  budget(30000),
);

/** Opens Metrics & ratios and the capital structure disclosure; returns its region. */
async function openCapitalStructure() {
  fireEvent.click(await screen.findByRole("tab", { name: "Metrics & ratios" }));
  const summary = await screen.findByText("Capital structure and debt stack");
  expect(
    call.mock.calls.filter(([method]) => method === "capitalStructureSummary"),
  ).toHaveLength(0);
  const details = summary.closest("details")!;
  details.open = true;
  fireEvent(details, new Event("toggle"));
  return screen.findByRole(
    "region",
    { name: "Capital structure" },
    { timeout: budget(10000) },
  );
}
/** Picks the period and EBITDA node, then submits once the form accepts it. */
async function summarize(region: HTMLElement, period: string) {
  await userEvent.click(
    within(region).getByRole("combobox", { name: "Period" }),
  );
  // The select's popup mounts its options after the trigger click resolves.
  await userEvent.click(await screen.findByRole("option", { name: period }));
  fireEvent.change(
    within(region).getByRole("combobox", { name: "EBITDA node" }),
    { target: { value: "ebitda" } },
  );
  const submit = within(region).getByRole("button", { name: "Summarize" });
  const submitted = () =>
    call.mock.calls.filter(([method]) => method === "capitalStructureSummary");
  // No native validator runs in this loop: Rust validates the config on the call.
  await waitFor(
    () => {
      fireEvent.click(submit);
      expect(submitted().length).toBeGreaterThan(0);
    },
    { timeout: budget(10000) },
  );
}

it(
  "summarizes the debt stack in Metrics & ratios, calling Rust only once opened",
  async () => {
    render(
      <FinstackQueryProvider>
        <StatementsWorkbench
          defaultModelJson={JSON.stringify(capitalStructure.model)}
        />
      </FinstackQueryProvider>,
    );
    const region = await openCapitalStructure();
    const expected = capitalStructure.cases.noCash.summary;
    await summarize(region, expected.as_of);
    const table = await within(region).findByRole(
      "table",
      { name: `Debt stack at ${expected.as_of} (${expected.currency})` },
      { timeout: budget(10000) },
    );
    const request = lastCall("capitalStructureSummary");
    expect(JSON.parse(request.configJson)).toEqual(
      capitalStructure.cases.noCash.config,
    );
    expect(
      within(table)
        .getAllByRole("rowheader")
        .map((cell) => cell.textContent),
    ).toEqual(expected.tranches.map((tranche) => tranche.id));
    expect(
      within(region).getByText("Cash", { selector: "dt" }).nextElementSibling!
        .textContent,
    ).toBe("Undefined");
  },
  budget(30000),
);

it(
  "summarizes the sample model's TLB and RCF in their first-lien class",
  async () => {
    render(
      <FinstackQueryProvider>
        <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
      </FinstackQueryProvider>,
    );
    const region = await openCapitalStructure();
    await summarize(region, "2025Q4");
    const table = await within(region).findByRole(
      "table",
      { name: "Debt stack at 2025Q4 (USD)" },
      { timeout: budget(10000) },
    );
    expect(
      within(table)
        .getAllByRole("rowheader")
        .map((cell) => cell.textContent),
    ).toEqual(["TLB", "RCF"]);
    expect(within(region).queryByRole("alert")).toBeNull();
  },
  budget(30000),
);

it(
  "shows Rust's error for a model with no capital structure",
  async () => {
    render(
      <FinstackQueryProvider>
        <StatementsWorkbench
          defaultModelJson={JSON.stringify(
            capitalStructure.errors.noCapitalStructure.model,
          )}
        />
      </FinstackQueryProvider>,
    );
    const region = await openCapitalStructure();
    await summarize(region, "2025Q4");
    expect(
      (
        await within(region).findByRole("alert", undefined, {
          timeout: budget(10000),
        })
      ).textContent,
    ).toBe(capitalStructure.errors.noCapitalStructure.message);
  },
  budget(30000),
);

it("sets the DCF WACC from the native calculator only on the user's click", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
    </FinstackQueryProvider>,
  );
  fireEvent.click(await screen.findByRole("tab", { name: "Valuation" }));
  const form = await screen.findByRole("region", { name: "DCF assumptions" });
  const waccField = within(form).getByRole("textbox", {
    name: "WACC",
  }) as HTMLInputElement;
  const netDebt = within(form).getByRole("textbox", {
    name: "Net debt override",
  }) as HTMLInputElement;
  fireEvent.change(netDebt, { target: { value: "5" } });
  const calculator = screen.getByRole("region", { name: "WACC calculator" });
  const { inputs, rate } = waccCases.cases.blended;
  for (const [name, value] of [
    ["Equity weight", inputs.equity_weight],
    ["Cost of equity", inputs.cost_of_equity],
    ["Debt weight", inputs.debt_weight],
    ["Cost of debt", inputs.cost_of_debt],
    ["Tax rate", inputs.tax_rate],
  ] as const)
    fireEvent.change(within(calculator).getByRole("textbox", { name }), {
      target: { value: String(value) },
    });
  fireEvent.click(
    within(calculator).getByRole("button", { name: "Calculate" }),
  );
  const apply = await within(calculator).findByRole(
    "button",
    { name: "Use as WACC" },
    { timeout: budget(10000) },
  );
  // Calculating fills nothing, and neither step values the model.
  expect(waccField.value).toBe("");
  fireEvent.click(apply);
  expect(waccField.value).toBe(String(rate));
  expect(netDebt.value).toBe("5");
  expect(call.mock.calls.some(([method]) => method === "evaluateDcf")).toBe(
    false,
  );
});

it("values the current model with a native DCF, tornado and LBO in Valuation", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
    </FinstackQueryProvider>,
  );
  fireEvent.click(await screen.findByRole("tab", { name: "Valuation" }));
  const form = await screen.findByRole("region", { name: "DCF assumptions" });
  // Assumptions start empty: Rust supplies no WACC or growth default.
  expect(
    (within(form).getByRole("textbox", { name: "WACC" }) as HTMLInputElement)
      .value,
  ).toBe("");
  fireEvent.change(within(form).getByRole("textbox", { name: "WACC" }), {
    target: { value: "0.1" },
  });
  fireEvent.change(
    within(form).getByRole("textbox", { name: "Stable growth rate" }),
    { target: { value: "0.02" } },
  );
  const value = within(form).getByRole("button", { name: "Value" });
  await waitFor(() => {
    fireEvent.click(value);
    expect(lastCall("evaluateDcf")).toBeDefined();
  });
  const modelJson = lastCall("evaluateDcf").modelJson;
  const terminalValue = { type: "gordon_growth", stable_growth_rate: 0.02 };
  expect(lastCall("evaluateDcf")).toEqual({
    modelJson,
    wacc: 0.1,
    terminalValue,
    marketJson: undefined,
    asOf: undefined,
  });
  expect(lastCall("dcfSensitivity")).toEqual({
    modelJson,
    wacc: 0.1,
    terminalValue,
  });
  const report = await screen.findByRole("region", { name: "DCF valuation" });
  const expected = native.evaluateDcf(modelJson, {
    wacc: 0.1,
    terminal_value: terminalValue,
  });
  await waitFor(() =>
    expect(
      within(report)
        .getByRole("region", { name: "Enterprise value" })
        .querySelector("p")!.textContent,
    ).toBe(formatRawMoney(expected.enterprise_value)),
  );
  expect(
    await within(report).findByRole("table", {
      name: "Enterprise value change by assumption",
    }),
  ).toBeTruthy();
  // The LBO config starts empty and nothing is evaluated until it is submitted.
  const lbo = screen.getByRole("region", { name: "Leveraged buyout" });
  expect(
    (
      within(lbo).getByRole("combobox", {
        name: "Entry metric node",
      }) as HTMLInputElement
    ).value,
  ).toBe("");
  expect(within(lbo).getByText("No LBO evaluation available.")).toBeTruthy();
  expect(lastCall("evaluateLbo")).toBeUndefined();
});

it("runs a native sensitivity and applies a goal-seek solution only on explicit actions in Scenarios", async () => {
  const accepted = vi.fn();
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench
        defaultModelJson={JSON.stringify(model)}
        onModelAccepted={accepted}
      />
    </FinstackQueryProvider>,
  );
  fireEvent.click(await screen.findByRole("tab", { name: "Scenarios" }));
  const sensitivity = await screen.findByRole("region", {
    name: "Sensitivity analysis",
  });
  const choose = async (scope: HTMLElement, label: string, option: string) => {
    await userEvent.click(within(scope).getByRole("combobox", { name: label }));
    await userEvent.click(await screen.findByRole("option", { name: option }));
  };
  fireEvent.click(within(sensitivity).getByRole("radio", { name: "Diagonal" }));
  fireEvent.change(
    within(sensitivity).getByRole("combobox", { name: "Target metric" }),
    { target: { value: "ebitda" } },
  );
  await choose(sensitivity, "Tornado period", "2025Q4");
  const row = within(sensitivity).getByRole("group", { name: "Parameter 1" });
  fireEvent.change(within(row).getByRole("combobox", { name: "Node" }), {
    target: { value: "revenue" },
  });
  await choose(row, "Period", "2025Q4");
  fireEvent.change(
    within(row).getByRole("textbox", { name: "Percentage moves" }),
    { target: { value: "-10, 10" } },
  );
  expect(lastCall("runSensitivity")).toBeUndefined();
  fireEvent.click(
    within(sensitivity).getByRole("button", { name: "Run sensitivity" }),
  );
  await waitFor(() => expect(lastCall("runSensitivity")).toBeDefined());
  const modelJson = lastCall("runSensitivity").modelJson;
  const evaluated = new native.Evaluator().evaluate(modelJson);
  expect(lastCall("runSensitivity")).toEqual({
    modelJson,
    config: {
      mode: "diagonal",
      parameters: [
        {
          node_id: "revenue",
          period_id: "2025Q4",
          base_value: evaluated.nodes.revenue["2025Q4"],
          pct_range: [-10, 10],
        },
      ],
      target_metrics: ["ebitda"],
    },
    targetMetric: "ebitda",
    period: "2025Q4",
  });
  expect(
    await within(sensitivity).findByRole("table", {
      name: "Sensitivity tornado",
    }),
  ).toBeTruthy();
  // Goal seek solves on Solve with the updated model, and writes it back only on Apply.
  const seek = screen.getByRole("region", { name: "Goal seek" });
  fireEvent.change(
    within(seek).getByRole("combobox", { name: "Target node" }),
    {
      target: { value: "ebitda" },
    },
  );
  await choose(seek, "Target period", "2025Q4");
  fireEvent.change(
    within(seek).getByRole("textbox", { name: "Target value" }),
    {
      target: { value: "70000000" },
    },
  );
  fireEvent.change(
    within(seek).getByRole("combobox", { name: "Driver node" }),
    {
      target: { value: "revenue" },
    },
  );
  await choose(seek, "Driver period", "2025Q4");
  fireEvent.change(within(seek).getByRole("textbox", { name: "Lower bound" }), {
    target: { value: "100000000" },
  });
  fireEvent.change(within(seek).getByRole("textbox", { name: "Upper bound" }), {
    target: { value: "500000000" },
  });
  expect(lastCall("goalSeek")).toBeUndefined();
  fireEvent.click(within(seek).getByRole("button", { name: "Solve" }));
  await waitFor(() => expect(lastCall("goalSeek")).toBeDefined());
  expect(lastCall("goalSeek")).toEqual({
    modelJson,
    targetNode: "ebitda",
    targetPeriod: "2025Q4",
    targetValue: 70000000,
    driverNode: "revenue",
    driverPeriod: "2025Q4",
    updateModel: true,
    bounds: [100000000, 500000000],
  });
  const apply = await within(seek).findByRole("button", {
    name: "Apply to model",
  });
  await waitFor(() =>
    expect((apply as HTMLButtonElement).disabled).toBe(false),
  );
  const before = accepted.mock.calls.length;
  fireEvent.click(apply);
  await waitFor(() => expect(accepted.mock.calls.length).toBe(before + 1));
  expect(
    JSON.parse(accepted.mock.lastCall![0]).nodes.revenue.values["2025Q4"],
  ).toEqual({ amount: "244390239.98913208", currency: "USD" });
  await within(seek).findByText("Applied to the model.");
  // The solve is not repeated on the applied model, and the sensitivity of the old model is hidden.
  expect(
    call.mock.calls.filter(([method]) => method === "goalSeek"),
  ).toHaveLength(1);
  await within(sensitivity).findByText("No sensitivity run available.");
});

it("reviews the variance, bridge and forecast backtest since the last roll forward against the pre-roll model", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench
        defaultModelJson={JSON.stringify(model)}
        onRollForward={async () => JSON.stringify(reportedQ4)}
      />
    </FinstackQueryProvider>,
  );
  await screen.findByRole("region", { name: "Income statement" });
  fireEvent.click(screen.getByRole("tab", { name: "Forecast & roll forward" }));
  // Nothing to review before a roll forward.
  expect(
    screen.queryByRole("region", { name: "Since last roll forward" }),
  ).toBeNull();
  const before = lastCall("evaluateStatement").modelJson;
  fireEvent.click(
    await screen.findByRole("button", { name: "Roll forward to 2025Q2" }),
  );
  const review = await screen.findByRole("region", {
    name: "Since last roll forward",
  });
  expect(review.textContent).toContain("2025Q2, 2025Q3, 2025Q4");
  const after = lastCall("evaluateStatement").modelJson;
  const baseJson = new native.Evaluator().evaluate(before);
  const comparison = new native.Evaluator().evaluate(after);
  // Only nodes present in both results are offered.
  const metrics = within(review).getByRole("group", {
    name: "Variance metrics",
  });
  expect(
    within(metrics)
      .getAllByRole("button")
      .map((button) => button.textContent),
  ).toEqual(
    Object.keys(reportedQ4.nodes).filter(
      (id) => id in baseJson.nodes && id in comparison.nodes,
    ),
  );
  expect(lastCall("runVariance")).toBeUndefined();
  fireEvent.click(within(metrics).getByRole("button", { name: "revenue" }));
  await waitFor(() => expect(lastCall("runVariance")).toBeDefined());
  const variance = lastCall("runVariance");
  expect(JSON.parse(variance.baseJson)).toEqual(
    JSON.parse(JSON.stringify(baseJson)),
  );
  expect(JSON.parse(variance.configJson)).toEqual({
    baseline_label: "Before roll forward",
    comparison_label: "After roll forward",
    metrics: ["revenue"],
    periods: ["2025Q2", "2025Q3", "2025Q4"],
  });
  expect(
    await within(review).findByRole("table", {
      name: "Variance since last roll forward",
    }),
  ).toBeTruthy();
  // The backtest compares the pre-roll forecast with the reported values.
  await userEvent.click(
    within(review).getByRole("combobox", { name: "Backtested metric" }),
  );
  await userEvent.click(await screen.findByRole("option", { name: "revenue" }));
  await waitFor(() => expect(lastCall("backtestForecast")).toBeDefined());
  expect(lastCall("backtestForecast")).toEqual({
    actual: [218200000, 219600000, 228900000],
    forecast: ["2025Q2", "2025Q3", "2025Q4"].map(
      (period) => baseJson.nodes.revenue[period],
    ),
  });
  // The bridge runs once its metric and period are chosen.
  await userEvent.click(
    within(review).getByRole("combobox", { name: "Bridge metric" }),
  );
  await userEvent.click(await screen.findByRole("option", { name: "ebitda" }));
  await userEvent.click(
    within(review).getByRole("combobox", { name: "Bridge period" }),
  );
  await userEvent.click(await screen.findByRole("option", { name: "2025Q2" }));
  const drivers = within(review).getByRole("group", { name: "Bridge drivers" });
  fireEvent.click(within(drivers).getByRole("button", { name: "revenue" }));
  await waitFor(() =>
    expect(lastCall("varianceBridge")?.drivers).toEqual(["revenue"]),
  );
  expect(
    await within(review).findByRole("table", { name: "EBITDA bridge, 2025Q2" }),
  ).toBeTruthy();
});

it("simulates the current model in the Monte Carlo tab only once a config is submitted", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
    </FinstackQueryProvider>,
  );
  fireEvent.click(await screen.findByRole("tab", { name: "Monte Carlo" }));
  const simulation = await screen.findByRole("region", {
    name: "Monte Carlo simulation",
  });
  expect(
    within(simulation).getByText("No Monte Carlo results available."),
  ).toBeTruthy();
  fireEvent.change(
    within(simulation).getByRole("textbox", { name: "N paths" }),
    { target: { value: "8" } },
  );
  fireEvent.change(within(simulation).getByRole("textbox", { name: "Seed" }), {
    target: { value: "9007199254740993" },
  });
  const simulate = within(simulation).getByRole("button", { name: "Simulate" });
  await waitFor(() =>
    expect((simulate as HTMLButtonElement).disabled).toBe(false),
  );
  expect(lastCall("evaluateMonteCarlo")).toBeUndefined();
  fireEvent.click(simulate);
  await waitFor(() => expect(lastCall("evaluateMonteCarlo")).toBeDefined());
  // The seed beyond 2^53 reaches the worker as its exact integer token.
  expect(lastCall("evaluateMonteCarlo").configJson).toBe(
    '{"n_paths":8,"seed":9007199254740993}',
  );
  expect(
    await within(simulation).findByRole("table", {
      name: /percentiles by forecast period/,
    }),
  ).toBeTruthy();
  // Without the path table Rust reports no breach probability.
  const readout = within(simulation).getByRole("region", {
    name: "Breach probability",
  });
  fireEvent.change(
    within(readout).getByRole("textbox", { name: "Threshold" }),
    {
      target: { value: "1" },
    },
  );
  fireEvent.click(within(readout).getByRole("button", { name: "Test breach" }));
  await waitFor(() =>
    expect(lastCall("monteCarloBreachProbability")?.threshold).toBe(1),
  );
  expect(
    await within(readout).findByText(/Rust reports no probability/),
  ).toBeTruthy();
});

it("tests the host covenant engine on open in the Covenants tab and returns Rust's not_found for a template the model lacks", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench
        defaultModelJson={JSON.stringify(model)}
        defaultCovenantEngineJson={JSON.stringify(covenantEngine)}
      />
    </FinstackQueryProvider>,
  );
  fireEvent.click(await screen.findByRole("tab", { name: "Covenants" }));
  const grid = await screen.findByRole("region", {
    name: "Covenant compliance",
  });
  const header = within(grid).getAllByRole("row")[0]!;
  expect(
    [...header.querySelectorAll("th")].slice(1).map((cell) => cell.textContent),
  ).toEqual(covenantSeries.map((row) => row.as_of));
  // The engine is tested against the workbench's own canonical model and result.
  const evaluated = lastCall("evaluateCovenants");
  expect(JSON.parse(evaluated.engineJson)).toEqual(covenantEngine);
  expect(Object.keys(JSON.parse(evaluated.resultsJson).nodes)).toContain(
    "leverage",
  );
  // The breach forecast waits for a submitted config; the analyst engine
  // passes every date, so Rust's deterministic default forecasts none.
  const breaches = screen.getByRole("region", {
    name: "Covenant breach forecast",
  });
  expect(lastCall("forecastBreaches")).toBeUndefined();
  const forecast = within(breaches).getByRole("button", { name: "Forecast" });
  await waitFor(async () => {
    await userEvent.click(forecast);
    expect(lastCall("forecastBreaches")).toBeDefined();
  });
  expect(JSON.parse(lastCall("forecastBreaches").configJson)).toEqual({});
  expect(
    await within(breaches).findByText("Rust forecast no breaches"),
  ).toBeTruthy();
  const templates = within(
    screen.getByRole("region", { name: "Covenant package templates" }),
  );
  await userEvent.click(templates.getByRole("radio", { name: /^Cov lite/ }));
  await userEvent.type(templates.getByLabelText("Max leverage"), "5");
  await userEvent.type(templates.getByLabelText("Max senior leverage"), "3");
  const load = templates.getByRole("button", { name: "Load template" });
  // The template form enables Load template once its validation settles.
  await waitFor(() => expect(load.hasAttribute("disabled")).toBe(false));
  await userEvent.click(load);
  await waitFor(() =>
    expect(
      within(
        screen.getByRole("region", { name: "Covenant metrics" }),
      ).getAllByText("Missing"),
    ).toHaveLength(3),
  );
  const use = screen.getByRole("button", { name: "Use covenants" });
  await waitFor(async () => {
    await userEvent.click(use);
    expect(
      JSON.parse(lastCall("evaluateCovenants").engineJson).specs[0].metric_id,
    ).toBe("total_leverage");
  });
  expect((await screen.findByRole("alert")).textContent).toContain(
    "metric 'total_leverage' in period 2024Q1",
  );
});
it("rolls reported quarters up to native annual values with editable methods", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
    </FinstackQueryProvider>,
  );
  await screen.findByRole("region", { name: "Income statement" });
  fireEvent.click(screen.getByRole("button", { name: "Annual" }));
  const income = await screen.findByRole("region", {
    name: "Income statement",
  });
  await within(income).findByRole("columnheader", { name: "2024 Historical" });
  expect(
    within(income).getByRole("columnheader", { name: "2025 Forecast" }),
  ).toBeTruthy();
  expect(
    JSON.parse(lastCall("rollupStatement").specJson).methods,
  ).toMatchObject({
    total_debt: "period_end",
    cash: "period_end",
    leverage: "formula",
    ebitda_margin_pct: "formula",
  });
  expect(JSON.parse(lastCall("rollupStatement").specJson).methods.revenue).toBe(
    undefined,
  );
  expect(lastCall("defaultRollupSpec")).toEqual({
    modelJson: canonicalModel(),
    frequency: "annual",
  });

  const quarterly = new native.Evaluator().evaluate(canonicalModel());
  const rolled = native.statementResultRollup(
    JSON.stringify(quarterly),
    canonicalModel(),
    lastCall("rollupStatement").specJson,
  );
  // Every line's method is declared on its node, so an empty spec agrees with the view.
  expect(
    native.statementResultRollup(
      JSON.stringify(quarterly),
      canonicalModel(),
      JSON.stringify({ frequency: "annual" }),
    ).nodes,
  ).toEqual(rolled.nodes);
  // A value declared period_end takes the year end, not four quarters' sum.
  expect(rolled.nodes.debt_at_inception).toEqual({
    "2024": 420_000_000,
    "2025": 420_000_000,
  });
  // Draws carry no declaration, so they sum.
  expect(rolled.nodes.debt_draws).toEqual({
    "2024": 40_000_000,
    "2025": 55_000_000,
  });
  // debt_opening declares formula: closing debt less the year's flows is the
  // year's opening balance, not the opening of its fourth quarter.
  expect(rolled.nodes.debt_opening).toEqual({
    "2024": 420_000_000,
    "2025": 415_000_000,
  });
  expect(rolled.nodes.debt_opening["2024"]).toBe(
    quarterly.nodes.debt_opening["2024Q1"],
  );
  expect(rolled.nodes.debt_opening["2025"]).toBe(
    quarterly.nodes.debt_opening["2025Q1"],
  );
  const revenue = within(income).getByRole("button", {
    name: "Revenue 2024, inspect value",
  });
  expect(revenue.textContent).toBe(
    reportStatementCell(
      {
        periodId: "2024",
        present: true,
        text: "",
        value: rolled.monetary_nodes.revenue["2024"],
      },
      "millions",
    ),
  );
  expect(
    within(income).queryByRole("button", { name: "Revenue 2024, edit value" }),
  ).toBeNull();
  // A native rollup records no node_sources, so annual cells carry no source marker.
  expect(within(income).queryByText("Cell source:")).toBeNull();

  await userEvent.click(
    screen.getByRole("combobox", {
      name: "Debt, opening balance roll-up method",
    }),
  );
  await userEvent.click(
    await screen.findByRole("option", { name: "Period end (balance)" }),
  );
  await waitFor(() =>
    expect(
      JSON.parse(lastCall("rollupStatement").specJson).methods.debt_opening,
    ).toBe("period_end"),
  );
  // Choosing Sum must reach native: an unlisted line would keep its declaration.
  await userEvent.click(
    screen.getByRole("combobox", {
      name: "Debt, closing balance roll-up method",
    }),
  );
  await userEvent.click(
    await screen.findByRole("option", { name: "Sum (flow)" }),
  );
  await waitFor(() =>
    expect(
      JSON.parse(lastCall("rollupStatement").specJson).methods.total_debt,
    ).toBe("sum"),
  );
  fireEvent.click(screen.getByRole("button", { name: "Reported" }));
  await within(
    await screen.findByRole("region", { name: "Income statement" }),
  ).findByRole("button", { name: "Revenue 2024Q1, value source, edit value" });
});

it("seeds the annual methods from a host RollupSpec without asking for the native default", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench
        defaultModelJson={JSON.stringify(model)}
        defaultRollupSpecJson={JSON.stringify({
          frequency: "annual",
          methods: { cash: "sum", total_debt: "period_end" },
        })}
      />
    </FinstackQueryProvider>,
  );
  await screen.findByRole("region", { name: "Income statement" });
  fireEvent.click(screen.getByRole("button", { name: "Annual" }));
  await within(
    await screen.findByRole("region", { name: "Income statement" }),
  ).findByRole("columnheader", { name: "2024 Historical" });
  expect(JSON.parse(lastCall("rollupStatement").specJson).methods).toEqual({
    cash: "sum",
    total_debt: "period_end",
  });
  // A line the host spec leaves out keeps its declaration, and the picker says so.
  expect(
    screen.getByRole("combobox", {
      name: "Debt, opening balance roll-up method",
    }).textContent,
  ).toContain("Recalculate formula");
  expect(
    call.mock.calls.some(([method]) => method === "defaultRollupSpec"),
  ).toBe(false);
});
it("reports a host RollupSpec that does not parse instead of summing every line", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench
        defaultModelJson={JSON.stringify(model)}
        defaultRollupSpecJson="not json"
      />
    </FinstackQueryProvider>,
  );
  await screen.findByRole("region", { name: "Income statement" });
  fireEvent.click(screen.getByRole("button", { name: "Annual" }));
  expect((await screen.findByRole("alert")).textContent).toContain(
    "Default rollup spec:",
  );
  expect(call.mock.calls.some(([method]) => method === "rollupStatement")).toBe(
    false,
  );
});
it(
  "runs scenarios with forecast overrides and compares them natively",
  async () => {
    render(
      <FinstackQueryProvider>
        <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
      </FinstackQueryProvider>,
    );
    await screen.findByRole("region", { name: "Income statement" });
    fireEvent.click(screen.getByRole("button", { name: "2025Q3 Forecast" }));
    fireEvent.click(screen.getByRole("tab", { name: "Scenarios" }));
    fireEvent.click(screen.getByRole("button", { name: "Add scenario" }));
    const card = screen.getByRole("region", { name: "Scenario scenario_2" });
    // The empty parent is a real radio choice, and a new scenario inherits the first.
    expect(
      within(screen.getByRole("region", { name: "Scenario base" }))
        .getByRole("radio", { name: "No parent" })
        .getAttribute("aria-checked"),
    ).toBe("true");
    expect(
      within(card)
        .getByRole("radio", { name: "base" })
        .getAttribute("aria-checked"),
    ).toBe("true");
    // Repeated override pickers keep their visible label and name their scenario in the description.
    expect(
      description(
        within(screen.getByRole("region", { name: "Scenario base" })).getByRole(
          "combobox",
          { name: "Add value override" },
        ),
      ),
    ).toBe("Scenario base");
    expect(
      description(
        within(card).getByRole("combobox", { name: "Add forecast override" }),
      ),
    ).toBe("Scenario scenario_2");
    await userEvent.click(
      within(card).getByRole("combobox", { name: "Add forecast override" }),
    );
    await userEvent.click(
      await screen.findByRole("option", { name: "Revenue" }),
    );
    await userEvent.click(
      within(card).getByRole("combobox", { name: "Revenue forecast method" }),
    );
    await userEvent.click(
      await screen.findByRole("option", { name: "Fixed growth" }),
    );
    fireEvent.change(within(card).getByRole("textbox", { name: /^Rate/ }), {
      target: { value: "-0.1" },
    });
    const save = within(card).getByRole<HTMLButtonElement>("button", {
      name: "Save to scenario",
    });
    await waitFor(() => expect(save.disabled).toBe(false));
    fireEvent.click(save);
    // The button reads "Validating…" until the save has written the draft.
    await waitFor(() => expect(save.textContent).toBe("Save to scenario"));
    fireEvent.click(screen.getByRole("button", { name: "Run scenarios" }));

    const comparison = await screen.findByRole("region", {
      name: "Scenario comparison",
    });
    const setJson = lastCall("evaluateStatementScenarios").scenarioSetJson;
    expect(JSON.parse(setJson)).toEqual({
      scenarios: {
        base: {},
        scenario_2: {
          parent: "base",
          forecast_overrides: {
            revenue: { method: "growth_pct", params: { rate: -0.1 } },
          },
        },
      },
    });
    const results = native.evaluateScenarioSet(canonicalModel(), setJson);
    // Line names are row headers, so each scenario value is announced with its line.
    const lineHeaders = within(comparison).getAllByRole("rowheader");
    // One row header per body row; the first row holds the column headers.
    expect(lineHeaders).toHaveLength(
      within(comparison).getAllByRole("row").length - 1,
    );
    // scenario_2's default baseline is its parent, read from the native lineage.
    await within(comparison).findByRole("columnheader", {
      name: "Δ scenario_2 vs base",
    });
    const [delta] = native.scenarioDiff(
      setJson,
      results,
      "base",
      "scenario_2",
      ["revenue"],
      ["2025Q3"],
    ).variance.rows;
    const revenueRow = within(comparison)
      .getAllByRole("row")
      .find(
        (row) =>
          within(row).queryByRole("rowheader")?.textContent === "Revenue",
      )!;
    const cells = within(revenueRow)
      .getAllByRole("cell")
      .map((cell) => cell.textContent);
    expect(cells).toEqual([
      ...["base", "scenario_2"].map((name) =>
        reportStatementCell(
          {
            periodId: "2025Q3",
            present: true,
            text: "",
            value: results[name].monetary_nodes.revenue["2025Q3"],
          },
          "millions",
        ),
      ),
      reportStatementCell(
        {
          periodId: "2025Q3",
          present: true,
          text: "",
          value: { amount: String(delta.abs_var), currency: "USD" },
        },
        "millions",
      ),
    ]);
    expect(cells[0]).not.toBe(cells[1]);
    expect(screen.getByLabelText("Lineage of scenario_2").textContent).toBe(
      "base › scenario_2",
    );
    expect(
      screen.getByRole("region", { name: "base statements" }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "scenario_2" }));
    expect(
      await screen.findByRole("region", { name: "scenario_2 statements" }),
    ).toBeTruthy();
  },
  budget(15000),
);

it("rejects scenario amounts outside the Rust decimal contract before the worker", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
    </FinstackQueryProvider>,
  );
  await screen.findByRole("region", { name: "Income statement" });
  fireEvent.click(screen.getByRole("tab", { name: "Scenarios" }));
  const card = screen.getByRole("region", { name: "Scenario base" });
  await userEvent.click(
    within(card).getByRole("combobox", { name: "Add value override" }),
  );
  await userEvent.click(await screen.findByRole("option", { name: "Revenue" }));
  const amount = within(card).getByRole("textbox", { name: "Revenue" });
  expect(description(amount)).toBe("Scenario base");
  fireEvent.change(amount, { target: { value: "1_000" } });
  fireEvent.click(screen.getByRole("button", { name: "Run scenarios" }));
  expect(
    (
      await screen.findByText("Revenue: enter a decimal amount in base units.")
    ).getAttribute("role"),
  ).toBe("alert");
  expect(
    call.mock.calls.filter(
      ([method]) => method === "evaluateStatementScenarios",
    ),
  ).toHaveLength(0);

  fireEvent.change(amount, { target: { value: "1e3" } });
  fireEvent.click(screen.getByRole("button", { name: "Run scenarios" }));
  await waitFor(() =>
    expect(lastCall("evaluateStatementScenarios")).toBeDefined(),
  );
  expect(
    JSON.parse(lastCall("evaluateStatementScenarios").scenarioSetJson),
  ).toEqual({
    scenarios: {
      base: { overrides: { revenue: { amount: "1e3", currency: "USD" } } },
    },
  });
  expect(
    screen.queryByText("Revenue: enter a decimal amount in base units."),
  ).toBeNull();
});

it("rejects grid amounts outside the Rust decimal contract before validation", async () => {
  const accepted = vi.fn();
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench
        defaultModelJson={JSON.stringify(model)}
        onModelAccepted={accepted}
      />
    </FinstackQueryProvider>,
  );
  const income = await screen.findByRole("region", {
    name: "Income statement",
  });
  await waitFor(() => expect(accepted).toHaveBeenCalledTimes(1));
  const validations = () =>
    call.mock.calls.filter(([method]) => method === "validateStatementModel")
      .length;
  const before = validations();
  fireEvent.click(
    within(income).getByRole("button", {
      name: "Revenue 2024Q1, value source, edit value",
    }),
  );
  const exactValue = screen.getByRole("textbox", {
    name: "Revenue 2024Q1 exact value",
  });
  fireEvent.change(exactValue, { target: { value: "5e" } });
  fireEvent.click(screen.getByRole("button", { name: "Save Revenue 2024Q1" }));
  expect(
    (
      await screen.findByText("Enter a decimal amount in base units.")
    ).getAttribute("role"),
  ).toBe("alert");
  expect(validations()).toBe(before);

  fireEvent.change(exactValue, { target: { value: "1e3" } });
  fireEvent.click(screen.getByRole("button", { name: "Save Revenue 2024Q1" }));
  await waitFor(() => expect(accepted).toHaveBeenCalledTimes(2));
  expect(
    JSON.parse(lastCall("validateStatementModel")).nodes.revenue.values[
      "2024Q1"
    ].amount,
  ).toBe("1e3");
  expect(
    JSON.parse(accepted.mock.lastCall![0]).nodes.revenue.values["2024Q1"]
      .amount,
  ).toBe("1000");
});

it("starts from the base scenario and names the failing path when the host scenario set is invalid", async () => {
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench
        defaultModelJson={JSON.stringify(model)}
        defaultScenarioSetJson={JSON.stringify({
          scenarios: {
            downside: {
              overrides: { revenue: { amount: "1_000", currency: "USD" } },
            },
          },
        })}
      />
    </FinstackQueryProvider>,
  );
  await screen.findByRole("region", { name: "Income statement" });
  fireEvent.click(screen.getByRole("tab", { name: "Scenarios" }));
  expect(screen.getByRole("region", { name: "Scenario base" })).toBeTruthy();
  expect(
    screen.queryByRole("region", { name: "Scenario downside" }),
  ).toBeNull();
  expect(
    (
      await screen.findByText(/\/scenarios\/downside\/overrides\/revenue: /)
    ).getAttribute("role"),
  ).toBe("alert");
});

/** Replaces meta.warnings on every native evaluation the mocked worker returns. */
function evaluateWithWarnings(warnings: unknown[]) {
  const evaluate = call.getMockImplementation()! as (
    method: string,
    arg: any,
  ) => Promise<any>;
  call.mockImplementation(async (method: string, arg: any) => {
    const value = await evaluate(method, arg);
    if (method === "evaluateStatement") value.meta.warnings = warnings;
    return value;
  });
}

it("lists native evaluation warnings by line in a collapsed disclosure", async () => {
  evaluateWithWarnings([
    {
      non_finite_value: {
        node_id: "ltm_ebitda",
        period: "2024Q1",
        value: "nan",
      },
    },
    {
      non_finite_value: {
        node_id: "ltm_ebitda",
        period: "2024Q2",
        value: "inf",
      },
    },
    {
      capital_structure: {
        period: "2025Q1",
        warning: {
          kind: "interest_shortfall",
          amount: 125000,
          instrument_id: "TL-A",
        },
      },
    },
  ]);
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
    </FinstackQueryProvider>,
  );
  await screen.findByRole("region", { name: "Income statement" });
  const summary = screen.getByText("Evaluation warnings (3)");
  const disclosure = summary.closest("details")!;
  expect(disclosure.open).toBe(false);
  fireEvent.click(summary);
  expect(disclosure.open).toBe(true);
  const ltm = within(disclosure).getByRole("list", {
    name: "LTM EBITDA warnings",
  });
  expect(
    within(ltm)
      .getAllByRole("listitem")
      .map((item) => item.textContent),
  ).toEqual([
    "2024Q1 · non_finite_value · nan",
    "2024Q2 · non_finite_value · inf",
  ]);
  const capital = within(disclosure).getByRole("list", {
    name: "Capital structure warnings",
  });
  expect(
    within(capital)
      .getAllByRole("listitem")
      .map((item) => item.textContent),
  ).toEqual([
    "2025Q1 · interest_shortfall · amount: 125000, instrument_id: TL-A",
  ]);
});

it("renders no warning disclosure when native evaluation reports none", async () => {
  evaluateWithWarnings([]);
  render(
    <FinstackQueryProvider>
      <StatementsWorkbench defaultModelJson={JSON.stringify(model)} />
    </FinstackQueryProvider>,
  );
  await screen.findByRole("region", { name: "Income statement" });
  expect(screen.getByRole("status").textContent).toBe("Evaluated");
  expect(screen.queryByText(/Evaluation warnings/)).toBeNull();
  expect(
    screen.queryByRole("list", { name: "Evaluation warnings" }),
  ).toBeNull();
});
