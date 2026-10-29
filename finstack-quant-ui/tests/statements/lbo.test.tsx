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
import config from "../../src/fixtures/statements/analyst-lbo-config.json";
import captured from "../../src/fixtures/statements/analyst-lbo.json";
import checkReport from "../../src/fixtures/statements/analyst-check-report.json";
import { LboReport } from "@/components/finstack/statements/components/lbo-report/lbo-report";
import { evaluateLboOptions } from "@/hooks/statements/use-evaluate-lbo/use-evaluate-lbo";
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
const configJson = JSON.stringify(config);
const result = captured as unknown as LboResultWire;
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
  "evaluates the LBO through the real worker exactly as the facade returns it",
  async () => {
    const { client, calls } = recordingClient();
    const request = { modelJson, configJson };
    const options = evaluateLboOptions(client, 1, request);
    request.configJson = "changed after snapshot";
    const query = new QueryClient();
    try {
      const returned = await query.fetchQuery(options);
      expect(serializeHost(returned)).toBe(
        serializeHost(native.evaluateLbo(modelJson, configJson)),
      );
      // The captured gallery fixture is this same native result.
      expect(JSON.parse(serializeHost(returned))).toEqual(captured);
      expect(calls).toEqual([["evaluateLbo", { modelJson, configJson }]]);
      const fails = (overrides: Record<string, unknown>) =>
        query.fetchQuery(
          evaluateLboOptions(client, 1, {
            modelJson,
            configJson: JSON.stringify({ ...config, ...overrides }),
          }),
        );
      // Unknown keys, a missing node and a non-positive equity check are rejected natively.
      await expect(fails({ bogus: 1 })).rejects.toThrow(/unknown field/);
      await expect(fails({ exit_net_debt_node: "net_debt" })).rejects.toThrow(
        /net_debt/,
      );
      await expect(
        fails({ sources: [{ name: "Unitranche", amount: 400000000 }] }),
      ).rejects.toThrow(/strictly positive equity check/);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it("keys the query by every request field and the worker session, and disables a null request", () => {
  const { client } = recordingClient();
  const request = { modelJson, configJson };
  const key = hashKey(evaluateLboOptions(client, 1, request).queryKey);
  expect(hashKey(evaluateLboOptions(client, 2, request).queryKey)).not.toBe(
    key,
  );
  for (const field of Object.keys(request))
    expect(
      hashKey(
        evaluateLboOptions(client, 1, { ...request, [field]: "changed" })
          .queryKey,
      ),
    ).not.toBe(key);
  expect(evaluateLboOptions(client, 1, request).enabled).toBe(true);
  expect(evaluateLboOptions(client, 1, null).enabled).toBe(false);
});

it("edits the config with node fields as comboboxes and submits it unchanged", async () => {
  const submit = vi.fn<(json: string) => void>();
  render(
    <LboReport
      defaultConfig={config}
      nodeIds={Object.keys(model.nodes)}
      onSubmit={submit}
      result={null}
    />,
  );
  for (const name of [
    "Entry metric node",
    "Exit metric node",
    "Exit net debt node",
  ])
    expect(
      (screen.getByRole("combobox", { name }) as HTMLInputElement).value,
    ).toBe(
      config[
        name.toLowerCase().replaceAll(" ", "_") as keyof typeof config
      ] as string,
    );
  const evaluate = screen.getByRole("button", { name: "Evaluate LBO" });
  await waitFor(() => {
    fireEvent.click(evaluate);
    expect(submit).toHaveBeenCalled();
  });
  expect(JSON.parse(submit.mock.lastCall![0])).toEqual(config);
  expect(screen.getByText("No LBO evaluation available.")).toBeTruthy();
});

it("presents the returned result as returned, with MOIC only and no checks", () => {
  render(
    <LboReport
      defaultConfig={config}
      nodeIds={[]}
      onSubmit={() => {}}
      result={result}
    />,
  );
  const tile = (name: string) =>
    screen.getByRole("region", { name }).querySelector("p")!.textContent;
  expect(tile("Equity check")).toBe(formatRawMoney(result.equity_check));
  expect(tile("Exit equity proceeds")).toBe(
    formatRawMoney(result.exit_equity_proceeds),
  );
  expect(tile("MOIC")).toBe("1.0802243251029904");
  expect(
    within(screen.getByLabelText("Returned sources and uses"))
      .getAllByRole("definition")
      .map((value) => value.textContent),
  ).toEqual([
    formatRawMoney(result.debt_total),
    formatRawMoney(result.equity_check),
    formatRawMoney(result.sources_total),
    formatRawMoney(result.entry_enterprise_value),
    formatRawMoney(result.uses_total),
    "Yes",
  ]);
  expect(
    within(screen.getByLabelText("Returned entry and exit"))
      .getAllByRole("definition")
      .map((value) => value.textContent),
  ).toEqual([
    "40,160,000",
    formatRawMoney(result.entry_enterprise_value),
    "61,383,803.277075",
    formatRawMoney(result.exit_enterprise_value),
    formatRawMoney(result.exit_net_debt),
    formatRawMoney(result.exit_equity_proceeds),
    "1.0802243251029904",
  ]);
  expect(
    screen.getByText(/IRR is\s+not part of the native LBO result/),
  ).toBeTruthy();
  expect(
    within(screen.getByRole("region", { name: "LBO checks" })).getByText(
      /the config maps no LBO checks/,
    ),
  ).toBeTruthy();
});

it("shows a returned check report, an unbalanced flag and native errors", () => {
  const { rerender } = render(
    <LboReport
      nodeIds={[]}
      onSubmit={() => {}}
      result={{
        ...result,
        sources_uses_balanced: false,
        checks: checkReport as unknown as LboResultWire["checks"],
      }}
    />,
  );
  // The empty config form reports its own structural errors; the result's alert is separate.
  expect(
    screen
      .getAllByRole("alert")
      .some((alert) =>
        /sources do not reconcile to uses/.test(alert.textContent!),
      ),
  ).toBe(true);
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
      error="Evaluation error: LBO exit bridge requires node 'net_debt'"
    />,
  );
  expect(
    screen.getAllByRole("alert").map((alert) => alert.textContent),
  ).toContain("Evaluation error: LBO exit bridge requires node 'net_debt'");
});
