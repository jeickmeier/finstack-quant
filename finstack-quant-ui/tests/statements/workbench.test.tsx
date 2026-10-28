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
import { StatementsWorkbench } from "@/components/finstack/statements/blocks/statements-workbench/statements-workbench";
import { reportStatementCell } from "@/components/finstack/statements/components/statement-grid/projection";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
vi.mock(
  "@/components/finstack/statements/components/statement-diagnostics/statement-explanation",
  () => ({ StatementExplanation: () => null }),
);
vi.mock(
  "@/components/finstack/statements/components/statement-chart/statement-chart",
  () => ({ StatementChart: () => null }),
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
      case "evaluateStatement":
        return new native.Evaluator().evaluate(arg.modelJson);
      case "validateStatementFormula":
        native.parseAndCompile(arg);
        return native.parseFormula(arg);
      case "rollupStatement":
        return native.statementResultRollup(
          arg.resultsJson,
          arg.modelJson,
          arg.specJson,
        );
      case "evaluateStatementScenarios":
        return native.evaluateScenarioSet(arg.modelJson, arg.scenarioSetJson);
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

it("opens complete statements, applies an explicit edit, and rolls forward from a supplied model", async () => {
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
    within(income).getByRole("button", { name: "Revenue 2024Q1, edit value" }),
  );
  const exactValue = screen.getByRole("textbox", {
    name: "Revenue 2024Q1 exact value",
  });
  expect(document.activeElement).toBe(exactValue);
  fireEvent.change(exactValue, {
    target: { value: "190000000" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save Revenue 2024Q1" }));
  await waitFor(() => expect(accepted).toHaveBeenCalledTimes(2));
  expect(
    JSON.parse(accepted.mock.lastCall![0]).nodes.revenue.values["2024Q1"]
      .amount,
  ).toBe("190000000");
  await screen.findByRole("button", { name: "Revenue 2024Q1, edit value" });
  expect(
    screen.getByRole("button", { name: "Revenue 2024Q1, edit value" })
      .textContent,
  ).toBe("190.0");
  expect(
    call.mock.calls.filter(([method]) => method === "evaluateStatement"),
  ).toHaveLength(accepted.mock.calls.length);
  fireEvent.click(screen.getByRole("tab", { name: "Forecast & roll forward" }));
  await screen.findByRole("button", { name: "Roll forward to 2025Q2" });
  fireEvent.click(
    screen.getByRole("button", { name: "Roll forward to 2025Q2" }),
  );
  await waitFor(() => expect(rollForward).toHaveBeenCalledTimes(1));
  await screen.findByRole("button", { name: "Roll forward to 2025Q3" });
  expect(
    screen.getByRole("button", { name: "2025Q2 Historical" }),
  ).toBeTruthy();
});

it("changes a native forecast method and calculates a newly added metric", async () => {
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
  fireEvent.click(screen.getByRole("tab", { name: "Forecast & roll forward" }));
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
    within(revenueEditor).getByRole("textbox", { name: /Growth per period/ }),
    { target: { value: "0.1" } },
  );
  fireEvent.click(
    within(revenueEditor).getByRole("button", { name: "Apply forecast" }),
  );
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
    target: { value: "revenue / debt_closing" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Add and calculate" }));
  await waitFor(() => expect(accepted).toHaveBeenCalledTimes(3));
  const metricModel = JSON.parse(accepted.mock.lastCall![0]);
  expect(metricModel.nodes.revenue_to_debt).toMatchObject({
    node_type: "calculated",
    formula_text: "revenue / debt_closing",
    value_type: { type: "scalar" },
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
      name: "Revenue to debt 2024Q1, inspect value",
    }).textContent,
  ).toBe("0.445");
  fireEvent.click(screen.getByRole("tab", { name: "Statements & editing" }));
  fireEvent.click(
    screen.getByRole("button", { name: "Revenue 2024Q1, edit value" }),
  );
  fireEvent.change(
    screen.getByRole("textbox", { name: "Revenue 2024Q1 exact value" }),
    { target: { value: "190000000" } },
  );
  fireEvent.click(screen.getByRole("button", { name: "Save Revenue 2024Q1" }));
  await waitFor(() => expect(accepted).toHaveBeenCalledTimes(4));
  fireEvent.click(screen.getByRole("tab", { name: "Metrics & ratios" }));
  const refreshedRatios = await screen.findByRole("region", {
    name: "Metrics, margins & covenant ratios",
  });
  await waitFor(() =>
    expect(
      within(refreshedRatios).getByRole("button", {
        name: "Revenue to debt 2024Q1, inspect value",
      }).textContent,
    ).toBe("0.463"),
  );
});

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
    debt_closing: "period_end",
    leverage: "formula",
    ebitda_margin_pct: "formula",
  });
  expect(JSON.parse(lastCall("rollupStatement").specJson).methods.revenue).toBe(
    undefined,
  );

  const quarterly = new native.Evaluator().evaluate(canonicalModel());
  const rolled = native.statementResultRollup(
    JSON.stringify(quarterly),
    canonicalModel(),
    lastCall("rollupStatement").specJson,
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

  await userEvent.click(
    screen.getByRole("combobox", {
      name: "Debt, opening balance roll-up method",
    }),
  );
  await userEvent.click(
    await screen.findByRole("option", { name: "Recalculate formula (ratio)" }),
  );
  await waitFor(() =>
    expect(
      JSON.parse(lastCall("rollupStatement").specJson).methods.debt_opening,
    ).toBe("formula"),
  );
  fireEvent.click(screen.getByRole("button", { name: "Reported" }));
  await within(
    await screen.findByRole("region", { name: "Income statement" }),
  ).findByRole("button", { name: "Revenue 2024Q1, edit value" });
});

it("runs scenarios with forecast overrides and compares them natively", async () => {
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
  await userEvent.click(await screen.findByRole("option", { name: "Revenue" }));
  await userEvent.click(
    within(card).getByRole("combobox", { name: "Revenue forecast method" }),
  );
  await userEvent.click(
    await screen.findByRole("option", { name: "Fixed growth" }),
  );
  fireEvent.change(
    within(card).getByRole("textbox", { name: /Growth per period/ }),
    { target: { value: "-0.1" } },
  );
  fireEvent.click(
    within(card).getByRole("button", { name: "Save to scenario" }),
  );
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
  const revenueRow = within(comparison)
    .getAllByRole("row")
    .find(
      (row) => within(row).queryByRole("rowheader")?.textContent === "Revenue",
    )!;
  const cells = within(revenueRow)
    .getAllByRole("cell")
    .map((cell) => cell.textContent);
  expect(cells).toEqual(
    ["base", "scenario_2"].map((name) =>
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
  );
  expect(cells[0]).not.toBe(cells[1]);
  expect(screen.getByRole("region", { name: "base statements" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "scenario_2" }));
  expect(
    await screen.findByRole("region", { name: "scenario_2 statements" }),
  ).toBeTruthy();
});

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
  fireEvent.change(amount, { target: { value: "1." } });
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
    within(income).getByRole("button", { name: "Revenue 2024Q1, edit value" }),
  );
  const exactValue = screen.getByRole("textbox", {
    name: "Revenue 2024Q1 exact value",
  });
  fireEvent.change(exactValue, { target: { value: "+5" } });
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
              overrides: { revenue: { amount: "1.", currency: "USD" } },
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
