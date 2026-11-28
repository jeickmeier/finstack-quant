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
import request from "../../src/fixtures/statements/analyst-lbo-request.json";
import captured from "../../src/fixtures/statements/analyst-lbo.json";
import checkReport from "../../src/fixtures/statements/analyst-check-report.json";
import { LboReport } from "@/components/finstack/statements/components/lbo-report/lbo-report";
import { evaluateLboOptions } from "@/hooks/statements/use-evaluate-lbo/use-evaluate-lbo";
import type { LboRequestWire } from "@/lib/finstack/generated/types/lbo_request";
import type { LboResultWire } from "@/lib/finstack/generated/types/lbo_result";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import {
  unwrap,
  type Envelope,
  type WorkerApi,
} from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
import { formatRawMoney } from "../../src/format/format";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const modelJson: string = native.validateFinancialModelJson(
  JSON.stringify(model),
);
const requestJson = JSON.stringify(request);
const defaultRequest = request as unknown as LboRequestWire;
const result = captured as unknown as LboResultWire;
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
});
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
  "evaluates the dated LBO through the real worker exactly as the facade returns it",
  async () => {
    const { client, calls } = recordingClient();
    const evaluation = { modelJson, requestJson };
    const options = evaluateLboOptions(client, 1, evaluation);
    evaluation.requestJson = "changed after snapshot";
    const query = new QueryClient();
    try {
      const returned = await query.fetchQuery(options);
      expect(serializeHost(returned)).toBe(
        serializeHost(native.evaluateLbo(modelJson, requestJson)),
      );
      // The captured gallery fixture is this same native result.
      expect(JSON.parse(serializeHost(returned))).toEqual(captured);
      expect(calls).toEqual([["evaluateLbo", { modelJson, requestJson }]]);
      const fails = (overrides: Record<string, unknown>) =>
        query.fetchQuery(
          evaluateLboOptions(client, 1, {
            modelJson,
            requestJson: JSON.stringify({ ...request, ...overrides }),
          }),
        );
      // Unknown keys, a missing node, and an entry the model's observations
      // cannot support are rejected natively.
      await expect(fails({ bogus: 1 })).rejects.toThrow(/unknown field/);
      await expect(fails({ exit_debt_node: "net_debt" })).rejects.toThrow(
        /net_debt/,
      );
      await expect(
        fails({ funding: { ...request.funding, date: "2025-03-30" } }),
      ).rejects.toThrow(/matching entry and funding dates/);
      await expect(
        fails({
          cut: {
            ...request.cut,
            as_of: "2024-03-31",
            observed_through: "2024-03-31",
          },
          funding: { ...request.funding, date: "2024-03-31" },
          entry: { ...request.entry, period: "2024Q1" },
        }),
      ).rejects.toThrow(/hidden_by_cutoff/);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it("keys the query by every request field and the worker session, and disables a null request", () => {
  const { client } = recordingClient();
  const evaluation = { modelJson, requestJson };
  const key = hashKey(evaluateLboOptions(client, 1, evaluation).queryKey);
  expect(hashKey(evaluateLboOptions(client, 2, evaluation).queryKey)).not.toBe(
    key,
  );
  for (const field of Object.keys(evaluation))
    expect(
      hashKey(
        evaluateLboOptions(client, 1, { ...evaluation, [field]: "changed" })
          .queryKey,
      ),
    ).not.toBe(key);
  expect(evaluateLboOptions(client, 1, evaluation).enabled).toBe(true);
  expect(evaluateLboOptions(client, 1, null).enabled).toBe(false);
});

it("edits the request with node fields as comboboxes and submits it unchanged", async () => {
  const submit = vi.fn<(json: string) => void>();
  render(
    <LboReport
      defaultRequest={defaultRequest}
      nodeIds={Object.keys(model.nodes)}
      onSubmit={submit}
      result={null}
    />,
  );
  for (const [name, value] of [
    ["Exit debt node", request.exit_debt_node],
    ["Exit cash node", request.exit_cash_node],
  ] as const)
    expect(
      (screen.getByRole("combobox", { name }) as HTMLInputElement).value,
    ).toBe(value);
  // The entry and the exit earnings each name a node.
  expect(
    screen
      .getAllByRole("combobox", { name: "Node" })
      .map((input) => (input as HTMLInputElement).value),
  ).toEqual([request.entry.node, request.exit.node]);
  const evaluate = screen.getByRole("button", { name: "Evaluate LBO" });
  await waitFor(() => {
    fireEvent.click(evaluate);
    expect(submit).toHaveBeenCalled();
  });
  expect(JSON.parse(submit.mock.lastCall![0])).toEqual(request);
  expect(screen.getByText("No LBO evaluation available.")).toBeTruthy();
});

it("presents the returned result as returned, with both MOICs and no checks", () => {
  render(
    <LboReport
      defaultRequest={defaultRequest}
      nodeIds={[]}
      onSubmit={() => {}}
      result={result}
    />,
  );
  const tile = (name: string) =>
    screen.getByRole("region", { name }).querySelector("p")!.textContent;
  expect(tile("Required sponsor cash")).toBe(
    formatRawMoney(result.funding.required_sponsor_cash),
  );
  expect(tile("Sponsor proceeds")).toBe(
    formatRawMoney(result.exit.sponsor_proceeds),
  );
  expect(tile("Sized MOIC")).toBe("1.1309070545377986");
  expect(tile("Funded MOIC")).toBe("1.1309070545377986");
  expect(
    within(screen.getByLabelText("Returned sources and uses"))
      .getAllByRole("definition")
      .map((value) => value.textContent),
  ).toEqual([
    formatRawMoney(result.funding.cash_uses),
    formatRawMoney(result.funding.debt_cash_proceeds),
    formatRawMoney(result.funding.opening_debt_face),
    formatRawMoney(result.funding.required_sponsor_cash),
    formatRawMoney(result.funding.committed_cash_sources!),
    formatRawMoney(result.funding.funding_gap!),
  ]);
  expect(
    within(screen.getByLabelText("Returned entry and exit"))
      .getAllByRole("definition")
      .map((value) => value.textContent),
  ).toEqual([
    "2025-03-31",
    formatRawMoney(result.entry_earnings),
    formatRawMoney(result.entry_enterprise_value),
    "2025-12-31",
    formatRawMoney(result.exit_earnings),
    formatRawMoney(result.exit.enterprise_value),
    formatRawMoney(result.exit.net_debt),
    formatRawMoney(result.exit.other_claims),
    formatRawMoney(result.exit.common_equity_residual),
    formatRawMoney(result.exit.sponsor_proceeds),
    "1.1309070545377986",
    "1.1309070545377986",
  ]);
  expect(
    within(screen.getByLabelText("Returned statement run"))
      .getAllByRole("definition")
      .map((value) => value.textContent),
  ).toEqual([
    "ACME-SOFTWARE-3STMT",
    "analyst-published-2025Q1",
    "2025-03-31",
    result.run.model_content_hash,
  ]);
  expect(
    screen.getByText(/IRR is\s+not part of the native LBO result/),
  ).toBeTruthy();
  expect(
    within(screen.getByRole("region", { name: "LBO checks" })).getByText(
      /the request elects no check suite/,
    ),
  ).toBeTruthy();
});

it("shows a returned check report, returns Rust withholds and native errors", () => {
  const { rerender } = render(
    <LboReport
      nodeIds={[]}
      onSubmit={() => {}}
      result={{
        ...result,
        funding: {
          ...result.funding,
          committed_cash_sources: null,
          funding_gap: null,
        },
        exit: { ...result.exit, funded_moic: null },
        checks: checkReport as unknown as LboResultWire["checks"],
      }}
    />,
  );
  // A sizing-only result has no commitment, gap or funded return: each shows as undefined.
  expect(
    screen.getByRole("region", { name: "Funded MOIC" }).querySelector("p")!
      .textContent,
  ).toBe("Undefined");
  expect(
    within(screen.getByLabelText("Returned sources and uses"))
      .getAllByRole("definition")
      .slice(-2)
      .map((value) => value.textContent),
  ).toEqual(["Undefined", "Undefined"]);
  expect(
    within(screen.getByRole("region", { name: "LBO checks" })).getAllByRole(
      "table",
    ).length,
  ).toBeGreaterThan(0);
  rerender(
    <LboReport
      nodeIds={[]}
      onSubmit={() => {}}
      result={null}
      error="Evaluation error: Node 'net_debt' at 2025Q4 must be monetary in USD"
    />,
  );
  expect(
    screen.getAllByRole("alert").map((alert) => alert.textContent),
  ).toContain(
    "Evaluation error: Node 'net_debt' at 2025Q4 must be monetary in USD",
  );
});
