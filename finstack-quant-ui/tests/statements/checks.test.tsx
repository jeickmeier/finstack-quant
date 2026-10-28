// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRequire } from "node:module";
import model from "../../src/fixtures/statements/analyst-model.json";
import result from "../../src/fixtures/statements/analyst-result.json";
import suite from "../../src/fixtures/statements/analyst-checks.json";
import { StatementChecks } from "@/components/finstack/statements/components/statement-diagnostics/statement-checks";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
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
const resultsJson = JSON.stringify(result);
const analystChecks = JSON.stringify(suite);
let call: ReturnType<typeof vi.fn>;
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  call = vi.fn(async (method: string, arg: any) => {
    switch (method) {
      case "initialize":
        return { state: "ready", worker: true };
      case "validateCheckSuiteSpec":
        return native.validateCheckSuiteSpecJson(arg);
      case "runStatementChecks":
        switch (arg.kind) {
          case "suite":
            return native.runChecks(
              arg.modelJson,
              native.validateCheckSuiteSpecJson(arg.configJson),
              arg.resultsJson,
            );
          case "three-statement":
            return native.runThreeStatementChecks(
              arg.modelJson,
              arg.configJson,
              arg.resultsJson,
            );
        }
        throw new Error(`Unexpected check runner: ${arg.kind}`);
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
  call.mock.calls.filter(([name]) => name === method);
function renderChecks(defaultConfigJson?: string) {
  return render(
    <FinstackQueryProvider>
      <StatementChecks
        modelJson={modelJson}
        resultsJson={resultsJson}
        defaultConfigJson={defaultConfigJson}
      />
    </FinstackQueryProvider>,
  );
}
async function runButton() {
  const button = screen.getByRole("button", { name: "Run native checks" });
  await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
  return button;
}

it("runs the supplied suite as native canonical JSON and renders the native report", async () => {
  const user = userEvent.setup();
  renderChecks(analystChecks);
  await waitFor(() =>
    expect(calls("validateCheckSuiteSpec").length).toBeGreaterThan(0),
  );
  await user.click(await runButton());
  const report = await screen.findByRole("region", {
    name: "Statement check report",
  });
  const canonical: string = native.validateCheckSuiteSpecJson(analystChecks);
  expect(calls("runStatementChecks").at(-1)?.[1]).toEqual({
    modelJson,
    resultsJson,
    kind: "suite",
    configJson: canonical,
  });
  const { summary } = native.runChecks(modelJson, canonical, resultsJson);
  expect(
    within(report).getByText(
      `${summary.passed} passed · ${summary.failed} failed`,
      { exact: false },
    ),
  ).toBeTruthy();
});

it("starts an unseeded suite empty and blocks submission until the required name is supplied", async () => {
  const user = userEvent.setup();
  renderChecks();
  const [name] = screen.getAllByRole("textbox", { name: "Name" });
  expect(name).toHaveProperty("value", "");
  expect(name.getAttribute("aria-invalid")).toBe("true");
  // The Rust example (three_statement_integrity with a builtin and a formula check) is never synthesized.
  expect(screen.getAllByRole("textbox")).toHaveLength(1);
  expect(screen.queryByRole("group", { name: /^Builtin checks/ })).toBeNull();
  expect(screen.queryByRole("group", { name: /^Formula checks/ })).toBeNull();
  await user.click(await runButton());
  await waitFor(() => expect(document.activeElement).toBe(name));
  expect(calls("validateCheckSuiteSpec")).toHaveLength(0);
  expect(calls("runStatementChecks")).toHaveLength(0);

  await user.type(name, "Desk review");
  await waitFor(() =>
    expect(name.getAttribute("aria-invalid")).not.toBe("true"),
  );
  await user.click(await runButton());
  await screen.findByRole("region", { name: "Statement check report" });
  expect(calls("runStatementChecks").at(-1)?.[1].configJson).toBe(
    native.validateCheckSuiteSpecJson(JSON.stringify({ name: "Desk review" })),
  );
});

it("edits builtin struct fields and runs a raw three-statement mapping", async () => {
  const user = userEvent.setup();
  renderChecks(analystChecks);
  await user.click(screen.getByRole("button", { name: "Add Builtin checks" }));
  await user.click(
    screen.getByRole("combobox", { name: "Builtin checks 2 type" }),
  );
  await user.click(
    await screen.findByRole("option", { name: "Balance sheet articulation" }),
  );
  const check = screen.getByRole("group", { name: "Builtin checks 2" });
  for (const name of ["Assets nodes", "Liabilities nodes", "Equity nodes"])
    expect(within(check).getByRole("group", { name })).toBeTruthy();

  await user.click(
    screen.getByRole("radio", { name: "Three statement mapping" }),
  );
  expect(screen.queryByRole("group", { name: "Builtin checks 2" })).toBeNull();
  const mapping = screen.getByRole("textbox", {
    name: "Canonical node mapping JSON",
  });
  expect(mapping).toHaveProperty("value", "");
  const run = screen.getByRole("button", { name: "Run native checks" });
  expect(run.hasAttribute("disabled")).toBe(true);
  const mappingJson = JSON.stringify({
    assets_nodes: ["total_assets"],
    liabilities_nodes: ["total_liabilities"],
    equity_nodes: ["total_equity"],
    cash_node: "cash",
    retained_earnings_node: "retained_earnings",
    net_income_node: "net_income",
  });
  await user.click(mapping);
  await user.paste(mappingJson);
  await user.click(await runButton());
  await screen.findByRole("region", { name: "Statement check report" });
  expect(calls("runStatementChecks").at(-1)?.[1]).toEqual({
    modelJson,
    resultsJson,
    kind: "three-statement",
    configJson: mappingJson,
  });
});

const threeStatementMapping = JSON.stringify({
  assets_nodes: ["total_assets"],
  liabilities_nodes: ["total_liabilities"],
  equity_nodes: ["total_equity"],
  cash_node: "cash",
  retained_earnings_node: "retained_earnings",
  net_income_node: "net_income",
});

it("keeps the suite draft and its report when the runner is toggled away and back", async () => {
  const user = userEvent.setup();
  renderChecks(analystChecks);
  const [name] = screen.getAllByRole("textbox", { name: "Name" });
  await user.clear(name);
  await user.type(name, "edited suite");
  await user.click(await runButton());
  await screen.findByRole("region", { name: "Statement check report" });
  expect(calls("runStatementChecks")).toHaveLength(1);

  await user.click(
    screen.getByRole("radio", { name: "Three statement mapping" }),
  );
  expect(screen.queryByRole("textbox", { name: "Name" })).toBeNull();
  expect(
    screen.queryByRole("region", { name: "Statement check report" }),
  ).toBeNull();
  await user.click(screen.getByRole("radio", { name: "Check suite" }));

  const [after] = await screen.findAllByRole("textbox", { name: "Name" });
  expect(after).toBe(name);
  expect(after).toHaveProperty("value", "edited suite");
  expect(
    await screen.findByRole("region", { name: "Statement check report" }),
  ).toBeTruthy();
  expect(calls("runStatementChecks")).toHaveLength(1);
  expect(calls("runStatementChecks")[0][1].configJson).toBe(
    native.validateCheckSuiteSpecJson(
      JSON.stringify({ ...suite, name: "edited suite" }),
    ),
  );
});

it("keeps a completed mapping report across a suite toggle and a suite reseed", async () => {
  const user = userEvent.setup();
  const view = renderChecks(analystChecks);
  await user.click(
    screen.getByRole("radio", { name: "Three statement mapping" }),
  );
  await user.click(
    screen.getByRole("textbox", { name: "Canonical node mapping JSON" }),
  );
  await user.paste(threeStatementMapping);
  await user.click(await runButton());
  await screen.findByRole("region", { name: "Statement check report" });

  await user.click(screen.getByRole("radio", { name: "Check suite" }));
  expect(
    screen.queryByRole("region", { name: "Statement check report" }),
  ).toBeNull();
  await user.click(
    screen.getByRole("radio", { name: "Three statement mapping" }),
  );
  expect(
    screen.getByRole("textbox", { name: "Canonical node mapping JSON" }),
  ).toHaveProperty("value", threeStatementMapping);
  expect(
    await screen.findByRole("region", { name: "Statement check report" }),
  ).toBeTruthy();

  // Reseeding remounts the suite form, whose cleared validation must not drop a mapping run.
  const validations = calls("validateCheckSuiteSpec").length;
  view.rerender(
    <FinstackQueryProvider>
      <StatementChecks
        modelJson={modelJson}
        resultsJson={resultsJson}
        defaultConfigJson={JSON.stringify({ ...suite, name: "Reseeded" })}
      />
    </FinstackQueryProvider>,
  );
  await waitFor(() =>
    expect(calls("validateCheckSuiteSpec").length).toBeGreaterThan(validations),
  );
  expect(
    screen.getByRole("region", { name: "Statement check report" }),
  ).toBeTruthy();
  expect(calls("runStatementChecks")).toHaveLength(1);
  expect(calls("runStatementChecks")[0][1]).toEqual({
    modelJson,
    resultsJson,
    kind: "three-statement",
    configJson: threeStatementMapping,
  });
});
