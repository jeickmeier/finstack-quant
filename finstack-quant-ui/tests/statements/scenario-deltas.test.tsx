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
import { createRequire } from "node:module";
import model from "../../src/fixtures/statements/analyst-model.json";
import scenarioSet from "../../src/fixtures/statements/analyst-scenarios.json";
import diffs from "../../src/fixtures/statements/analyst-scenario-diff.json";
import {
  VarianceTable,
  type VarianceReportWire,
} from "@/components/finstack/statements/components/variance-table/variance-table";
import { ScenarioPanel } from "@/components/finstack/statements/blocks/statements-workbench/scenario-panel";
import { reportStatementCell } from "@/components/finstack/statements/components/statement-grid/projection";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import type { FinancialModelSpecWire } from "@/lib/finstack/generated/types/financial_model_spec";

const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const modelJson: string = native.validateFinancialModelJson(
  JSON.stringify(model),
);
const wire = JSON.parse(modelJson) as FinancialModelSpecWire;
const setJson = JSON.stringify(scenarioSet);
let call: ReturnType<typeof vi.fn>;
let failDiff: Error | null;
beforeEach(() => {
  failDiff = null;
  vi.stubGlobal("Worker", class {});
  call = vi.fn(async (method: string, arg: any) => {
    switch (method) {
      case "initialize":
        return { state: "ready", worker: true };
      case "evaluateStatementScenarios":
        return native.evaluateScenarioSet(arg.modelJson, arg.scenarioSetJson);
      case "traceStatementScenario":
        return native.scenarioSetTrace(arg.scenarioSetJson, arg.scenario);
      case "diffStatementScenarios":
        if (failDiff) throw failDiff;
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
      default:
        throw new Error(`Unexpected worker call: ${method}`);
    }
  });
  mock.createClient.mockReset().mockReturnValue({ call, close: vi.fn() });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const calls = (method: string) =>
  call.mock.calls.filter(([name]) => name === method).map(([, arg]) => arg);
const bodyRows = (table: HTMLElement) =>
  within(table)
    .getAllByRole("row")
    .slice(1)
    .map((row) =>
      [...row.querySelectorAll("th, td")].map((cell) => cell.textContent),
    );

it("shows native variance rows in native order with native labels and n/m for an unset fraction", () => {
  const report: VarianceReportWire = {
    baseline_label: "management_case",
    comparison_label: "bank_case",
    rows: [
      {
        period: "2025Q2",
        metric: "net_income",
        baseline: 1234.5,
        comparison: 1000,
        abs_var: -234.5,
        pct_var: -0.18995544754,
      },
      {
        period: "2025Q1",
        metric: "debt_draws",
        baseline: 0,
        comparison: 5,
        abs_var: 5,
        pct_var: null,
      },
      {
        period: "2025Q1",
        metric: "revenue",
        baseline: 0,
        comparison: 0,
        abs_var: 0,
      },
    ],
  };
  render(<VarianceTable report={report} caption="Bank against management" />);
  const table = screen.getByRole("table", { name: "Bank against management" });
  expect(
    within(table)
      .getAllByRole("columnheader")
      .map((cell) => cell.textContent),
  ).toEqual([
    "Metric",
    "Period",
    "management_case",
    "bank_case",
    "Variance",
    "Variance %",
  ]);
  // Rows keep native order (no sorting by metric or period) and every supplied digit.
  expect(bodyRows(table)).toEqual([
    ["Net income", "2025Q2", "1,234.5", "1,000", "-234.5", "-18.995544754%"],
    ["Debt draws", "2025Q1", "0", "5", "5", "n/m"],
    ["Revenue", "2025Q1", "0", "0", "0", "n/m"],
  ]);
});

it("takes host metric labels and value text without recomputing the report", () => {
  const report = diffs[1].variance as unknown as VarianceReportWire;
  const formatValue = vi.fn((_metric: string, value: number) => `<${value}>`);
  render(
    <VarianceTable
      report={report}
      caption="Severe against downside"
      metricLabel={(metric) => `node ${metric}`}
      formatValue={formatValue}
    />,
  );
  const rows = bodyRows(
    screen.getByRole("table", { name: "Severe against downside" }),
  );
  expect(rows).toHaveLength(report.rows.length);
  report.rows.forEach((row, index) =>
    expect(rows[index].slice(0, 5)).toEqual([
      `node ${row.metric}`,
      row.period,
      `<${row.baseline}>`,
      `<${row.comparison}>`,
      `<${row.abs_var}>`,
    ]),
  );
});

it("diffs each scenario against its traced parent and leaves the root without a delta until a baseline is chosen", async () => {
  render(
    <FinstackQueryProvider>
      <ScenarioPanel
        model={wire}
        modelJson={modelJson}
        nodeIds={["revenue", "opex", "ebitda"]}
        activePeriod="2025Q3"
        displayMode="millions"
        defaultScenarioSetJson={setJson}
      />
    </FinstackQueryProvider>,
  );
  fireEvent.click(await screen.findByRole("button", { name: "Run scenarios" }));
  const comparison = await screen.findByRole("region", {
    name: "Scenario comparison",
  });
  await within(comparison).findByRole("columnheader", {
    name: "Δ severe vs downside",
  });
  const sentSet = calls("evaluateStatementScenarios")[0].scenarioSetJson;
  // One native trace per scenario; defaults are the entry before each scenario.
  expect(
    calls("traceStatementScenario")
      .map((arg) => arg.scenario)
      .sort(),
  ).toEqual(["base", "downside", "severe"]);
  expect(screen.getByLabelText("Lineage of severe").textContent).toBe(
    "base › downside › severe",
  );
  const [request] = calls("diffStatementScenarios");
  expect(request.pairs).toEqual([
    { baseline: "base", comparison: "downside" },
    { baseline: "downside", comparison: "severe" },
  ]);
  expect(request.metrics).toEqual(["revenue", "opex", "ebitda"]);
  expect(request.periods).toEqual(["2025Q3"]);
  expect(
    within(comparison)
      .getAllByRole("columnheader")
      .map((cell) => cell.textContent),
  ).toEqual([
    "Line item",
    "base",
    "downside",
    "severe",
    "Δ downside vs base",
    "Δ severe vs downside",
  ]);
  // Each delta cell is Rust's abs_var on the line's own display path.
  const results = native.evaluateScenarioSet(modelJson, sentSet);
  const [opex] = native.scenarioDiff(
    sentSet,
    results,
    "downside",
    "severe",
    ["opex"],
    ["2025Q3"],
  ).variance.rows;
  const opexRow = within(comparison)
    .getAllByRole("row")
    .find(
      (row) =>
        within(row).queryByRole("rowheader")?.textContent ===
        "Operating expenses, ex D&A",
    )!;
  expect(within(opexRow).getAllByRole("cell").at(-1)!.textContent).toBe(
    reportStatementCell(
      {
        periodId: "2025Q3",
        present: true,
        text: "",
        value: { amount: String(opex.abs_var), currency: "USD" },
      },
      "millions",
    ),
  );
  // The root has no parent, so no baseline until the user picks one.
  const baseChoice = screen.getByRole("radiogroup", {
    name: "Baseline for base",
  });
  expect(
    within(baseChoice)
      .getByRole("radio", { name: "No baseline" })
      .getAttribute("aria-checked"),
  ).toBe("true");
  fireEvent.click(within(baseChoice).getByRole("radio", { name: "severe" }));
  await within(comparison).findByRole("columnheader", {
    name: "Δ base vs severe",
  });
  expect(calls("diffStatementScenarios").at(-1).pairs).toEqual([
    { baseline: "severe", comparison: "base" },
    { baseline: "base", comparison: "downside" },
    { baseline: "downside", comparison: "severe" },
  ]);
  // The shown scenario's native variance report sits above its statements.
  expect(
    await screen.findByRole("table", { name: "base against severe, 2025Q3" }),
  ).toBeTruthy();
});

it("shows the native diff error without hiding the scenario values", async () => {
  failDiff = Object.assign(new Error("metric 'opex' not found at 2025Q3"), {
    kind: "not_found",
  });
  render(
    <FinstackQueryProvider>
      <ScenarioPanel
        model={wire}
        modelJson={modelJson}
        nodeIds={["revenue", "opex"]}
        activePeriod="2025Q3"
        displayMode="exact"
        defaultScenarioSetJson={setJson}
      />
    </FinstackQueryProvider>,
  );
  fireEvent.click(await screen.findByRole("button", { name: "Run scenarios" }));
  const comparison = await screen.findByRole("region", {
    name: "Scenario comparison",
  });
  await waitFor(() =>
    expect(within(comparison).getByRole("alert").textContent).toContain(
      "metric 'opex' not found at 2025Q3",
    ),
  );
  expect(within(comparison).getByRole("table")).toBeTruthy();
  expect(
    within(comparison).queryByRole("columnheader", { name: /^Δ / }),
  ).toBeNull();
});
