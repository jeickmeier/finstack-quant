// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import { useCovenantEngineValidator } from "@/hooks/covenants/use-covenant-engine-validator/use-covenant-engine-validator";
import { useCovenantTemplate } from "@/hooks/covenants/use-covenant-template/use-covenant-template";
import {
  evaluateCovenantsOptions,
  useEvaluateCovenants,
} from "@/hooks/covenants/use-evaluate-covenants/use-evaluate-covenants";
import { CovenantEngineEditor } from "@/components/finstack/covenants/components/covenant-engine-editor/covenant-engine-editor";
import { CovenantCompliance } from "@/components/finstack/covenants/components/covenant-compliance/covenant-compliance";
import {
  unwrap,
  type CovenantTemplateRequest,
} from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
import { useWorkerHarness } from "../shared/worker/use-worker-harness";
import { budget } from "../shared/budget";
import series from "../../src/fixtures/covenants/analyst-covenant-series.json";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const read = (name: string) =>
  readFileSync(
    resolve(import.meta.dirname, "../../src/fixtures/", name),
    "utf8",
  );
const modelJson = read("statements/analyst-model.json");
const resultsJson = read("statements/analyst-result.json");
const engineJson = read("covenants/analyst-covenants.json");
const nodeIds = Object.keys(JSON.parse(modelJson).nodes);
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../shared/worker/client"),
);
type Resources = Record<
  "covenantEngineConstructed" | "covenantEngineFreed",
  number
>;
const harness = useWorkerHarness([
  "covenantTemplate",
  "validateCovenantEngine",
  "evaluateCovenants",
]);
const resources = () =>
  (harness.proxy as unknown as { resources(): Promise<Resources> }).resources();
/** CovenantEngine handles constructed and freed by one call. */
async function counted<T>(call: () => Promise<T>) {
  const before = await resources();
  const value = await call();
  const after = await resources();
  return {
    value,
    engine: [
      after.covenantEngineConstructed - before.covenantEngineConstructed,
      after.covenantEngineFreed - before.covenantEngineFreed,
    ],
  };
}
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it("returns each Rust package template as the facade does", async () => {
  const requests: CovenantTemplateRequest[] = [
    { kind: "cov_lite", max_leverage: 5, max_senior_leverage: 3 },
    {
      kind: "lbo_standard",
      initial_leverage: 6,
      interest_coverage: 2,
      fixed_charge_coverage: 1.5,
      max_capex: 50_000_000,
    },
    {
      kind: "real_estate",
      min_dscr: 1.25,
      min_debt_yield: 0.08,
      max_ltv: 0.65,
    },
    {
      kind: "project_finance",
      min_dscr: 1.2,
      distribution_lockup_dscr: 1.1,
      min_liquidity: 1_000_000,
      max_net_leverage: 4,
    },
  ];
  for (const request of requests)
    expect(unwrap(await harness.proxy.covenantTemplate(request))).toEqual(
      native.covenantTemplate(request),
    );
  // A cleared threshold crosses as NaN, which Rust rejects.
  const cleared = await harness.proxy.covenantTemplate({
    kind: "cov_lite",
    max_leverage: Number.NaN,
    max_senior_leverage: 3,
  });
  expect(cleared.ok).toBe(false);
  if (!cleared.ok) {
    expect(cleared.error.kind).toBe("invalid_type");
    expect(cleared.error.message).toContain("template");
  }
});

it("validates an engine natively and returns its canonical text", async () => {
  expect(unwrap(await harness.proxy.validateCovenantEngine(engineJson))).toBe(
    native.validateCovenantEngineJson(engineJson),
  );
  const unknown = await harness.proxy.validateCovenantEngine(
    JSON.stringify({ ...JSON.parse(engineJson), extra: 1 }),
  );
  expect(unknown.ok).toBe(false);
  if (!unknown.ok) expect(unknown.error.message).toContain("unknown field");
});

it("evaluates the analyst engine over every result period and frees the engine", async () => {
  const call = await counted(
    async () =>
      await harness.proxy.evaluateCovenants({
        engineJson,
        resultsJson,
      }),
  );
  expect(serializeHost(unwrap(call.value))).toBe(serializeHost(series));
  expect(call.engine).toEqual([1, 1]);
});

it("returns Rust's not_found for a template metric the model lacks, freeing the engine", async () => {
  const specs = unwrap(
    await harness.proxy.covenantTemplate({
      kind: "cov_lite",
      max_leverage: 5,
      max_senior_leverage: 3,
    }),
  );
  // cov_lite reads total_leverage, senior_leverage and negative_debt.
  expect(specs.map((spec) => spec.metric_id)).toEqual([
    "total_leverage",
    "senior_leverage",
    "negative_debt",
  ]);
  expect(nodeIds).not.toContain("total_leverage");
  const missing = await counted(
    async () =>
      await harness.proxy.evaluateCovenants({
        engineJson: JSON.stringify({ specs }),
        resultsJson,
      }),
  );
  expect(missing.value.ok).toBe(false);
  if (!missing.value.ok) {
    expect(missing.value.error.kind).toBe("not_found");
    expect(missing.value.error.message).toContain(
      "metric 'total_leverage' in period 2024Q1",
    );
  }
  expect(missing.engine).toEqual([1, 1]);
  // A malformed engine is rejected before any handle exists.
  const malformed = await counted(
    async () =>
      await harness.proxy.evaluateCovenants({
        engineJson: "{}",
        resultsJson,
      }),
  );
  expect(malformed.value.ok).toBe(false);
  expect(malformed.engine).toEqual([0, 0]);
});

it("keys the evaluation on the worker session and the complete snapshotted request", () => {
  const request = { engineJson, resultsJson };
  const options = evaluateCovenantsOptions(null, 4, request);
  expect(options.queryKey).toEqual([
    "finstack",
    4,
    "evaluateCovenants",
    request,
  ]);
  expect(options.queryKey[3]).not.toBe(request);
  expect(evaluateCovenantsOptions(null, 4, null).enabled).toBe(false);
});

it("lists each covenant's metric id and whether the model has that node", async () => {
  render(
    <CovenantEngineEditor
      defaultJson={engineJson}
      validate={async (json) => json}
      onSubmit={() => {}}
      metricIds={nodeIds}
    />,
  );
  const metrics = within(
    screen.getByRole("region", { name: "Covenant metrics" }),
  );
  await waitFor(() => expect(metrics.getAllByRole("row")).toHaveLength(4));
  const rows = metrics
    .getAllByRole("row")
    .slice(1)
    .map((row) =>
      [...row.querySelectorAll("th,td")].map((cell) => cell.textContent),
    );
  expect(rows).toEqual([
    ["Specs", "min_interest_coverage", "metric_id", "interest_coverage", "Yes"],
    [
      "Window 2024-10-01 to 2025-12-31",
      "max_debt_to_ebitda",
      "metric_id",
      "leverage",
      "Yes",
    ],
    [
      "Window 2024-10-01 to 2025-12-31",
      "min_interest_coverage",
      "metric_id",
      "interest_coverage",
      "Yes",
    ],
  ]);
  // Every metric id (top-level and windowed) is an id-combobox over the node ids.
  const combos = screen
    .getAllByRole("combobox", { name: /^Metric id/ })
    .map((box) => (box as HTMLInputElement).value);
  expect(combos).toEqual([
    "interest_coverage",
    "leverage",
    "interest_coverage",
  ]);
  // Templates are hidden without a loader.
  expect(
    screen.queryByRole("region", { name: "Covenant package templates" }),
  ).toBeNull();
});

/** Click Load template once the form's debounced validation has settled. */
async function loadTemplateIn(templates: ReturnType<typeof within>) {
  const button = templates.getByRole("button", { name: "Load template" });
  await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
  await userEvent.click(button);
}
it("picks a package in the generated template form, blocks a blank threshold and shows the native rejection", async () => {
  const user = userEvent.setup();
  const loadTemplate = vi.fn(async (template: CovenantTemplateRequest) => {
    throw new Error(`rejected ${template.kind}`);
  });
  render(
    <CovenantEngineEditor
      validate={async (json) => json}
      onSubmit={() => {}}
      loadTemplate={loadTemplate}
      metricIds={nodeIds}
    />,
  );
  const templates = within(
    screen.getByRole("region", { name: "Covenant package templates" }),
  );
  // No package is chosen until the user picks one.
  expect(templates.queryByLabelText("Max leverage")).toBeNull();
  await user.click(templates.getByRole("radio", { name: /^Cov lite/ }));
  for (const label of ["Max leverage", "Max senior leverage"])
    expect(templates.getByLabelText(label)).toHaveProperty("value", "");
  await user.type(templates.getByLabelText("Max leverage"), "5");
  await loadTemplateIn(templates);
  // A blank required threshold fails the generated schema and never reaches Rust.
  await waitFor(() =>
    expect(
      templates
        .getByLabelText("Max senior leverage")
        .getAttribute("aria-invalid"),
    ).toBe("true"),
  );
  expect(loadTemplate).not.toHaveBeenCalled();
  await user.type(templates.getByLabelText("Max senior leverage"), "3");
  await loadTemplateIn(templates);
  await waitFor(() =>
    expect(loadTemplate).toHaveBeenCalledWith({
      kind: "cov_lite",
      max_leverage: 5,
      max_senior_leverage: 3,
    }),
  );
  expect(await templates.findByText("rejected cov_lite")).toBeTruthy();
});

function Connected({ defaultJson }: { defaultJson?: string }) {
  const validate = useCovenantEngineValidator();
  const loadTemplate = useCovenantTemplate();
  const [engine, setEngine] = useState<string | null>(null);
  const compliance = useEvaluateCovenants(
    engine ? { engineJson: engine, resultsJson } : null,
  );
  return (
    <>
      <CovenantEngineEditor
        defaultJson={defaultJson}
        validate={validate}
        loadTemplate={loadTemplate}
        onSubmit={setEngine}
        metricIds={nodeIds}
      />
      {compliance.error ? (
        <p role="alert">{compliance.error.message}</p>
      ) : compliance.data ? (
        <CovenantCompliance series={compliance.data} />
      ) : null}
    </>
  );
}
async function submit() {
  const button = await screen.findByRole("button", { name: "Use covenants" });
  await waitFor(
    async () => {
      await userEvent.click(button);
      expect(
        screen.queryByRole("region", { name: "Covenant compliance" }) ??
          screen.queryByRole("alert"),
      ).toBeTruthy();
    },
    { timeout: budget(10000) },
  );
}
it(
  "loads a Rust template, flags its missing metrics and surfaces the native not_found on evaluation",
  async () => {
    render(
      <FinstackQueryProvider>
        <Connected />
      </FinstackQueryProvider>,
    );
    const templates = within(
      await screen.findByRole("region", { name: "Covenant package templates" }),
    );
    await userEvent.click(templates.getByRole("radio", { name: /^Cov lite/ }));
    await userEvent.type(templates.getByLabelText("Max leverage"), "5");
    await userEvent.type(templates.getByLabelText("Max senior leverage"), "3");
    await loadTemplateIn(templates);
    const metrics = within(
      screen.getByRole("region", { name: "Covenant metrics" }),
    );
    await waitFor(() => expect(metrics.getAllByRole("row")).toHaveLength(4), {
      timeout: budget(5000),
    });
    expect(
      metrics
        .getAllByRole("row")
        .slice(1)
        .map((row) => {
          const { cells } = row as HTMLTableRowElement;
          return [cells[3]!.textContent, cells[4]!.textContent];
        }),
    ).toEqual([
      ["total_leverage", "Missing"],
      ["senior_leverage", "Missing"],
      ["negative_debt", "Missing"],
    ]);
    await submit();
    expect(screen.getByRole("alert").textContent).toContain(
      "metric 'total_leverage' in period 2024Q1",
    );
  },
  budget(30000),
);
it(
  "validates and evaluates the supplied engine into the native compliance grid",
  async () => {
    render(
      <FinstackQueryProvider>
        <Connected defaultJson={engineJson} />
      </FinstackQueryProvider>,
    );
    await submit();
    const grid = within(
      screen.getByRole("region", { name: "Covenant compliance" }),
    );
    const header = grid.getAllByRole("row")[0]!;
    expect(
      [...header.querySelectorAll("th")]
        .slice(1)
        .map((cell) => cell.textContent),
    ).toEqual(series.map((row) => row.as_of));
  },
  budget(30000),
);
