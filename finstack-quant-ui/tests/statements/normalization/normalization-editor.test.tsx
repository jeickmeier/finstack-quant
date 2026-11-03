// @vitest-environment jsdom
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { useState } from "react";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { hashKey } from "@tanstack/react-query";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  normalizeOptions,
  useNormalize,
} from "@/hooks/statements/use-normalize/use-normalize";
import { useNormalizationConfigValidator } from "@/hooks/statements/use-normalization-config-validator/use-normalization-config-validator";
import {
  NormalizationEditor,
  bridgeBars,
} from "@/components/finstack/statements/components/normalization-editor/normalization-editor";
import { unwrap, type NormalizationResult } from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { groupDecimal } from "@/lib/finstack/format/format";
import { startWorker } from "../../shared/worker/harness.mjs";
import { budget } from "../../shared/budget";
import analystResult from "../../../src/fixtures/statements/analyst-result.json";
import fixture from "./cases.json";

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
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
let harness: Awaited<ReturnType<typeof startWorker>>;
/** Native validations the form has run in the current test. */
let validated = 0;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
beforeEach(() => {
  validated = 0;
  vi.stubGlobal("Worker", class {});
  mock.createClient.mockReturnValue({
    call: vi.fn(async (method: string, arg: never) => {
      if (method === "initialize")
        return unwrap(await harness.proxy.initialize());
      if (method === "validateNormalizationConfig") {
        const canonical = unwrap(
          await harness.proxy.validateNormalizationConfig(arg),
        );
        validated += 1;
        return canonical;
      }
      if (method === "normalize")
        return unwrap(await harness.proxy.normalize(arg));
      throw new Error(`Unexpected worker call ${method}`);
    }),
    close: vi.fn(),
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});
const resultJson = JSON.stringify(analystResult);
const nodeIds = Object.keys(analystResult.nodes);
const results = fixture.results as unknown as NormalizationResult[];
const config = fixture.config as unknown as Record<string, unknown>;
const exact = (value: number) => groupDecimal(String(value));

it("returns the fixture's results and canonical config through the worker", async () => {
  expect(
    unwrap(
      await harness.proxy.validateNormalizationConfig(
        JSON.stringify(fixture.config),
      ),
    ),
  ).toBe(fixture.canonical);
  expect(
    unwrap(
      await harness.proxy.normalize({
        resultJson,
        configJson: fixture.canonical,
      }),
    ),
  ).toEqual(fixture.results);
  const rejected = await harness.proxy.validateNormalizationConfig(
    JSON.stringify(fixture.errors.duplicate.config),
  );
  expect(rejected.ok).toBe(false);
});

it("draws each step between the values Rust returns, with no running sum", async () => {
  // The fixture holds a capped and an uncapped step, and a deduction.
  const steps = results.flatMap((result) => result.adjustments);
  expect(steps.some((step) => step.is_capped)).toBe(true);
  expect(steps.some((step) => !step.is_capped)).toBe(true);
  expect(steps.some((step) => step.capped_amount < 0)).toBe(true);
  for (const result of results) {
    const bars = bridgeBars(result);
    expect(bars[0]).toMatchObject({ from: 0, to: result.base_value });
    expect(bars.at(-1)).toMatchObject({ from: 0, to: result.final_value });
    result.adjustments.forEach((step, index) => {
      const bar = bars[index + 1]!;
      expect(bar.to).toBe(step.adjusted_value);
      expect(bar.from).toBe(
        index === 0
          ? result.base_value
          : result.adjustments[index - 1]!.adjusted_value,
      );
      expect(bar.kind).toBe(
        step.is_capped ? "Capped adjustment" : "Adjustment",
      );
      expect(bar.amount).toBe(step.capped_amount);
    });
    expect(result.adjustments.at(-1)!.adjusted_value).toBe(result.final_value);
  }
  const source = await readFile(
    resolve(
      import.meta.dirname,
      "../../../registry/statements/components/normalization-editor/normalization-editor.tsx",
    ),
    "utf8",
  );
  expect(source).not.toMatch(/reduce\(|\+=|\+ step|previous \+/);
});

it("shows every adjustment by period as returned, with a capped one's raw amount", () => {
  render(
    <NormalizationEditor
      defaultConfig={config}
      nodeIds={nodeIds}
      validate={async (json) => json}
      onSubmit={() => {}}
      results={results}
    />,
  );
  const table = screen.getByRole("table", {
    name: "Adjustments by period (USD)",
  });
  const header = within(table).getAllByRole("columnheader");
  expect(header.map((cell) => cell.textContent)).toEqual([
    "Item",
    ...results.map((result) => result.period),
  ]);
  const rows = within(table).getAllByRole("row").slice(1);
  expect(rows).toHaveLength(results[0]!.adjustments.length + 2);
  const cells = (row: HTMLElement) =>
    within(row)
      .getAllByRole("cell")
      .map((cell) => cell.textContent);
  expect(cells(rows[0]!)).toEqual(
    results.map((result) => exact(result.base_value)),
  );
  expect(cells(rows.at(-1)!)).toEqual(
    results.map((result) => exact(result.final_value)),
  );
  results[0]!.adjustments.forEach((step, index) => {
    expect(within(rows[index + 1]!).getByRole("rowheader").textContent).toBe(
      step.name,
    );
    expect(cells(rows[index + 1]!)).toEqual(
      results.map((result) => {
        const applied = result.adjustments[index]!;
        return applied.is_capped
          ? `${exact(applied.capped_amount)} (capped; raw ${exact(applied.raw_amount)})`
          : exact(applied.capped_amount);
      }),
    );
  });
  expect(
    screen.getByRole("img", {
      name: `Base to adjusted, ${results[0]!.period}`,
    }),
  ).toBeTruthy();
});

it("switches the bridge to another returned period", async () => {
  render(
    <NormalizationEditor
      nodeIds={nodeIds}
      validate={async (json) => json}
      onSubmit={() => {}}
      results={results}
    />,
  );
  const last = results.at(-1)!.period;
  await userEvent.click(
    screen.getByRole("combobox", { name: "Bridge period" }),
  );
  await userEvent.click(screen.getByRole("option", { name: last }));
  expect(
    await screen.findByRole("img", { name: `Base to adjusted, ${last}` }),
  ).toBeTruthy();
});

it("shows native errors, loading and an empty result as such", () => {
  const props = {
    nodeIds,
    validate: async (json: string) => json,
    onSubmit: () => {},
  };
  const { rerender } = render(
    <NormalizationEditor {...props} results={null} loading />,
  );
  expect(screen.getByRole("status").textContent).toBe("Normalizing…");
  rerender(<NormalizationEditor {...props} results={undefined} />);
  expect(screen.getByRole("status").textContent).toBe(
    "No normalization available.",
  );
  rerender(<NormalizationEditor {...props} results={[]} />);
  expect(screen.getByRole("status").textContent).toBe(
    "The target node has no periods to normalize.",
  );
  for (const { message } of Object.values(fixture.errors)) {
    rerender(
      <NormalizationEditor {...props} results={results} error={message} />,
    );
    expect(screen.getByRole("alert").textContent).toBe(message);
    expect(screen.queryByRole("table")).toBeNull();
  }
});

function Live() {
  const validate = useNormalizationConfigValidator();
  const [configJson, setConfigJson] = useState<string | null>(null);
  const normalized = useNormalize(
    configJson === null ? null : { resultJson, configJson },
  );
  return (
    <NormalizationEditor
      defaultConfig={config}
      nodeIds={nodeIds}
      validate={validate}
      onSubmit={setConfigJson}
      results={normalized.data}
      loading={normalized.isFetching}
      error={normalized.error?.message}
    />
  );
}

it("edits the target as a node combobox, validates natively and normalizes through the worker", async () => {
  render(
    <FinstackQueryProvider>
      <Live />
    </FinstackQueryProvider>,
  );
  expect(
    (screen.getByRole("combobox", { name: "Target node" }) as HTMLInputElement)
      .value,
  ).toBe("ebitda");
  // Submit waits for the form's first native validation; a click before it
  // settles is not a submission, so click once validation has run.
  const submit = screen.getByRole("button", { name: "Normalize" });
  await waitFor(() => expect(validated).toBeGreaterThan(0), {
    timeout: budget(10000),
  });
  fireEvent.click(submit);
  const table = await screen.findByRole(
    "table",
    { name: "Adjustments by period (USD)" },
    { timeout: budget(10000) },
  );
  const rows = within(table).getAllByRole("row");
  expect(within(rows.at(-1)!).getAllByRole("cell")[0]!.textContent).toBe(
    exact(results[0]!.final_value),
  );
});

it("shows the native rejection of a target the result lacks", async () => {
  function Missing() {
    const normalized = useNormalize({
      resultJson,
      configJson: JSON.stringify(fixture.errors.missingTarget.config),
    });
    return (
      <NormalizationEditor
        nodeIds={nodeIds}
        validate={async (json) => json}
        onSubmit={() => {}}
        results={normalized.data}
        error={normalized.error?.message}
      />
    );
  }
  render(
    <FinstackQueryProvider>
      <Missing />
    </FinstackQueryProvider>,
  );
  expect(
    (await screen.findByRole("alert", undefined, { timeout: budget(10000) }))
      .textContent,
  ).toBe(fixture.errors.missingTarget.message);
});

it("keys the query on the result and config", () => {
  const client = { call: vi.fn() } as unknown as FinstackClient;
  const request = { resultJson: "{}", configJson: "{}" };
  const key = hashKey(normalizeOptions(client, 1, request).queryKey);
  expect(hashKey(normalizeOptions(client, 2, request).queryKey)).not.toBe(key);
  for (const field of Object.keys(request))
    expect(
      hashKey(
        normalizeOptions(client, 1, { ...request, [field]: "changed" })
          .queryKey,
      ),
    ).not.toBe(key);
  expect(normalizeOptions(client, 1, null).enabled).toBe(false);
});
