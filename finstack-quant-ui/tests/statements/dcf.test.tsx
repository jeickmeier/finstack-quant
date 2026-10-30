// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterAll, afterEach, beforeAll, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { QueryClient, hashKey } from "@tanstack/react-query";
import model from "../../src/fixtures/statements/analyst-model.json";
import assumptions from "../../src/fixtures/statements/analyst-dcf-assumptions.json";
import captured from "../../src/fixtures/statements/analyst-dcf.json";
import capturedSensitivity from "../../src/fixtures/statements/analyst-dcf-sensitivity.json";
import {
  DcfReport,
  type CorporateValuationResultWire,
} from "@/components/finstack/statements/components/dcf-report/dcf-report";
import {
  DcfAssumptionsForm,
  type DcfAssumptions,
  type DcfAssumptionsSubmission,
} from "@/components/finstack/statements/components/dcf-assumptions-form/dcf-assumptions-form";
import { TornadoChart } from "@/components/finstack/statements/components/tornado-chart/tornado-chart";
import { evaluateDcfOptions } from "@/hooks/statements/use-evaluate-dcf/use-evaluate-dcf";
import { dcfSensitivityOptions } from "@/hooks/statements/use-dcf-sensitivity/use-dcf-sensitivity";
import type { DcfSensitivityResultWire } from "@/lib/finstack/generated/types/dcf_sensitivity_result";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import {
  unwrap,
  type Envelope,
  type EvaluateDcfRequest,
  type WorkerApi,
} from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
import { labelFor } from "../../src/format/label.mjs";
import { formatRawMoney } from "../../src/format/format";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";

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
const modelJson: string = native.validateFinancialModelJson(
  JSON.stringify(model),
);
const result = captured as unknown as CorporateValuationResultWire;
const sensitivity = capturedSensitivity as unknown as DcfSensitivityResultWire;
const gordon: { type: "gordon_growth"; stable_growth_rate: number } = {
  type: "gordon_growth",
  stable_growth_rate: 0.02,
};
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
afterEach(cleanup);

/** Client over the real worker that records every operation and argument it forwards. */
function recordingClient() {
  const calls: [keyof WorkerApi, unknown][] = [];
  const client = {
    call: async (method: keyof WorkerApi, argument: unknown) => {
      calls.push([method, argument]);
      const invoke = harness.proxy[method] as unknown as (
        argument: unknown,
      ) => Promise<Envelope<unknown>>;
      return unwrap(await invoke(argument));
    },
  } as unknown as FinstackClient;
  return { client, calls };
}

it(
  "values the model and ranks its assumptions through the real worker exactly as the facade returns them",
  async () => {
    const { client, calls } = recordingClient();
    const optionsJson = JSON.stringify(assumptions.options);
    const request = {
      modelJson,
      wacc: 0.1,
      terminalValue: { ...gordon },
      ufcfNode: "ufcf",
      optionsJson,
    };
    const dcf = evaluateDcfOptions(client, 1, request);
    const tornado = dcfSensitivityOptions(client, 1, request);
    request.terminalValue.stable_growth_rate = 0.03;
    const query = new QueryClient();
    try {
      const valued = await query.fetchQuery(dcf);
      expect(serializeHost(valued)).toBe(
        serializeHost(
          native.evaluateDcf(
            modelJson,
            0.1,
            gordon,
            "ufcf",
            undefined,
            optionsJson,
          ),
        ),
      );
      // The captured gallery fixtures are these same native results.
      expect(JSON.parse(serializeHost(valued))).toEqual(captured);
      expect(
        JSON.parse(serializeHost(await query.fetchQuery(tornado))),
      ).toEqual(capturedSensitivity);
      // Unset optional arguments are never filled in by the worker.
      expect(calls).toEqual([
        [
          "evaluateDcf",
          {
            modelJson,
            wacc: 0.1,
            terminalValue: gordon,
            ufcfNode: "ufcf",
            optionsJson,
          },
        ],
        [
          "dcfSensitivity",
          {
            modelJson,
            wacc: 0.1,
            terminalValue: gordon,
            ufcfNode: "ufcf",
            optionsJson,
          },
        ],
      ]);
      const fails = (request: EvaluateDcfRequest) =>
        query.fetchQuery(evaluateDcfOptions(client, 1, request));
      // A cleared WACC reaches Rust as NaN and Rust rejects it.
      await expect(
        fails({ modelJson, wacc: Number.NaN, terminalValue: gordon }),
      ).rejects.toMatchObject({ kind: "validation" });
      // A cleared terminal field is rejected at the facade boundary.
      await expect(
        fails({
          modelJson,
          wacc: 0.1,
          terminalValue: {
            type: "gordon_growth",
            stable_growth_rate: Number.NaN,
          },
        }),
      ).rejects.toMatchObject({ name: "TypeError", kind: "invalid_type" });
      // Rust, not the worker, requires the as-of date with a market.
      await expect(
        fails({
          modelJson,
          wacc: 0.1,
          terminalValue: gordon,
          marketJson: "{}",
        }),
      ).rejects.toMatchObject({ kind: "validation" });
      // Unknown option keys are rejected natively.
      await expect(
        fails({
          modelJson,
          wacc: 0.1,
          terminalValue: gordon,
          optionsJson: '{"bogus":1}',
        }),
      ).rejects.toThrow(/unknown field `bogus`/);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it("keys both queries by every request field and the worker session, and keeps NaN distinct", () => {
  const { client } = recordingClient();
  const request = {
    modelJson,
    wacc: 0.1,
    terminalValue: gordon,
    ufcfNode: "ufcf",
    netDebtOverride: 1,
    optionsJson: "{}",
  };
  for (const options of [evaluateDcfOptions, dcfSensitivityOptions]) {
    const key = hashKey(options(client, 1, request).queryKey);
    expect(hashKey(options(client, 2, request).queryKey)).not.toBe(key);
    for (const field of Object.keys(request))
      expect(
        hashKey(
          options(client, 1, { ...request, [field]: "changed" }).queryKey,
        ),
      ).not.toBe(key);
    expect(
      hashKey(options(client, 1, { ...request, wacc: Number.NaN }).queryKey),
    ).not.toBe(
      hashKey(options(client, 1, { ...request, wacc: null as never }).queryKey),
    );
    expect(options(client, 1, request).enabled).toBe(true);
    expect(options(client, 1, null).enabled).toBe(false);
  }
  const market = { ...request, marketJson: "{}", asOf: "2025-03-31" };
  const key = hashKey(evaluateDcfOptions(client, 1, market).queryKey);
  for (const field of ["marketJson", "asOf"])
    expect(
      hashKey(
        evaluateDcfOptions(client, 1, { ...market, [field]: "changed" })
          .queryKey,
      ),
    ).not.toBe(key);
});

it("presents the returned valuation, bridge, periods, terminal value and tornado as returned", () => {
  render(<DcfReport result={result} sensitivity={sensitivity} />);
  const report = screen.getByRole("region", { name: "DCF valuation" });
  const tile = (name: string) =>
    within(report).getByRole("region", { name }).querySelector("p")!
      .textContent;
  expect(tile("Enterprise value")).toBe(
    formatRawMoney(result.enterprise_value),
  );
  expect(tile("Equity value")).toBe(formatRawMoney(result.equity_value));
  expect(tile("Equity value per share")).toBe(
    String(result.equity_value_per_share),
  );
  const inputs = within(report).getByLabelText("Equity bridge inputs");
  expect(
    within(inputs)
      .getAllByRole("term")
      .map((term) => term.textContent),
  ).toEqual(
    Object.keys(result.equity_bridge!).map((name) => `${labelFor(name)} (USD)`),
  );
  const periods = within(report).getByRole("table", {
    name: "Discounted unlevered free cash flow by period",
  });
  const rows = within(periods).getAllByRole("row");
  expect(rows).toHaveLength(result.periods!.length + 1);
  expect(
    within(rows[1]!)
      .getAllByRole("cell")
      .map((cell) => cell.textContent),
  ).toEqual([
    "2025Q2",
    "2025-06-30",
    "32,515,040.0625",
    "0.2464065708418891",
    "0.9767885730831344",
    "31,760,319.586390324",
  ]);
  const terminal = within(report).getByLabelText("Returned terminal value");
  expect(
    within(terminal)
      .getAllByRole("definition")
      .map((d) => d.textContent),
  ).toEqual([
    formatRawMoney(result.terminal_value!),
    "0.7501711156741958",
    formatRawMoney(result.terminal_value_pv),
    "0.1",
  ]);
  const shocks = within(report).getByLabelText("Effective shocks");
  expect(
    within(shocks)
      .getAllByRole("definition")
      .map((d) => d.textContent),
  ).toEqual([
    formatRawMoney(sensitivity.baseline_enterprise_value),
    "0.09000000000000001",
    "0.03",
  ]);
  const tornado = within(report).getByRole("table", {
    name: "Enterprise value change by assumption",
  });
  expect(
    within(tornado)
      .getAllByRole("row")
      .slice(1)
      .map((row) =>
        within(row)
          .getAllByRole("cell")
          .map((cell) => cell.textContent),
      ),
  ).toEqual([
    ["Terminal growth", "-191,902,948.3931589", "246,732,362.21977592"],
    ["Wacc", "241,830,333.02504706", "-188,031,363.85586596"],
  ]);
});

it("shows native errors and marks clamped shocks without computing", () => {
  const { rerender } = render(
    <DcfReport
      result={null}
      error="Evaluation error: bad wacc"
      sensitivity={null}
      sensitivityError="Evaluation error: bad growth"
    />,
  );
  expect(screen.getAllByRole("alert").map((node) => node.textContent)).toEqual([
    "Evaluation error: bad wacc",
    "Evaluation error: bad growth",
  ]);
  rerender(
    <DcfReport
      result={result}
      sensitivity={{
        ...sensitivity,
        wacc_down: 0.025,
        wacc_down_clamped: true,
        entries: [{ parameter_id: "wacc", downside: Number.NaN, upside: -0 }],
      }}
    />,
  );
  const shocks = screen.getByLabelText("Effective shocks");
  expect(within(shocks).getAllByRole("definition")[1]!.textContent).toBe(
    "0.025 (clamped)",
  );
  const rows = within(
    screen.getByRole("table", {
      name: "Enterprise value change by assumption",
    }),
  ).getAllByRole("row");
  expect(
    within(rows[1]!)
      .getAllByRole("cell")
      .map((cell) => cell.textContent),
  ).toEqual(["Wacc", "NaN", "0"]);
});

it("renders an empty tornado without a chart", () => {
  render(
    <TornadoChart
      entries={[]}
      valueLabel="Value change"
      caption="Empty tornado"
    />,
  );
  expect(screen.queryByRole("img")).toBeNull();
  expect(screen.getByText("No tornado entries returned")).toBeTruthy();
});

it("submits the assumptions as typed, leaving unset optional values unset", async () => {
  const submit = vi.fn<(submission: DcfAssumptionsSubmission) => void>();
  render(
    <DcfAssumptionsForm
      defaultAssumptions={assumptions as DcfAssumptions}
      nodeIds={Object.keys(model.nodes)}
      onSubmit={submit}
    />,
  );
  const value = screen.getByRole("button", { name: "Value" });
  await waitFor(() =>
    expect((value as HTMLButtonElement).disabled).toBe(false),
  );
  fireEvent.click(value);
  expect(submit).toHaveBeenLastCalledWith({
    wacc: 0.1,
    terminalValue: { type: "gordon_growth", stable_growth_rate: 0.02 },
    ufcfNode: "ufcf",
    optionsJson: JSON.stringify(assumptions.options),
  });
  // A cleared required number is NaN, never 0; switching the method sends its own fields.
  fireEvent.change(screen.getByRole("textbox", { name: "WACC" }), {
    target: { value: "" },
  });
  fireEvent.click(screen.getByRole("radio", { name: "Exit multiple" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Multiple" }), {
    target: { value: "9" },
  });
  fireEvent.change(screen.getByRole("combobox", { name: "UFCF node" }), {
    target: { value: "" },
  });
  fireEvent.click(value);
  expect(submit.mock.lastCall![0]).toEqual({
    wacc: Number.NaN,
    terminalValue: {
      type: "exit_multiple",
      multiple: 9,
      terminal_metric: Number.NaN,
    },
    optionsJson: JSON.stringify(assumptions.options),
  });
});

it("leaves empty options unset", async () => {
  const submit = vi.fn<(submission: DcfAssumptionsSubmission) => void>();
  render(<DcfAssumptionsForm nodeIds={[]} onSubmit={submit} />);
  const value = screen.getByRole("button", { name: "Value" });
  await waitFor(() =>
    expect((value as HTMLButtonElement).disabled).toBe(false),
  );
  fireEvent.click(value);
  expect(submit.mock.lastCall![0]).toEqual({
    wacc: Number.NaN,
    terminalValue: { type: "gordon_growth", stable_growth_rate: Number.NaN },
  });
});
