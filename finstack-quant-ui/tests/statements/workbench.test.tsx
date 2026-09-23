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
import rolledQ2 from "../../src/fixtures/statements/roll-forward-2025Q2.json";
import { StatementsWorkbench } from "@/components/finstack/statements/blocks/statements-workbench/statements-workbench";
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
      case "statementNodeIds":
        return native.modelNodeIds(arg);
      case "evaluateStatement":
        return native.evaluateModel(arg.modelJson);
      case "validateStatementFormula":
        native.validateFormula(arg);
        return native.parseFormulaText(arg);
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
  fireEvent.change(
    screen.getByRole("textbox", { name: "Revenue 2024Q1 exact value" }),
    {
      target: { value: "190000000" },
    },
  );
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
  fireEvent.click(
    screen.getByRole("button", { name: "Forecast & roll forward" }),
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
  fireEvent.click(
    screen.getByRole("button", { name: "Forecast & roll forward" }),
  );
  fireEvent.change(
    screen.getByRole("combobox", { name: "Revenue forecast method" }),
    { target: { value: "growth_pct" } },
  );
  const revenueEditor = screen
    .getByRole("combobox", { name: "Revenue forecast method" })
    .closest("div.rounded-lg") as HTMLElement;
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
  fireEvent.click(screen.getByRole("button", { name: "Metrics & ratios" }));
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
  const nativeResult = native.evaluateModel(accepted.mock.lastCall![0]);
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
  fireEvent.click(screen.getByRole("button", { name: "Statements & editing" }));
  fireEvent.click(
    screen.getByRole("button", { name: "Revenue 2024Q1, edit value" }),
  );
  fireEvent.change(
    screen.getByRole("textbox", { name: "Revenue 2024Q1 exact value" }),
    { target: { value: "190000000" } },
  );
  fireEvent.click(screen.getByRole("button", { name: "Save Revenue 2024Q1" }));
  await waitFor(() => expect(accepted).toHaveBeenCalledTimes(4));
  fireEvent.click(screen.getByRole("button", { name: "Metrics & ratios" }));
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
