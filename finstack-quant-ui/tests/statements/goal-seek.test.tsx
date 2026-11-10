// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterAll, afterEach, beforeAll, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { QueryClient, hashKey } from "@tanstack/react-query";
import type { GoalSeekResult } from "finstack-quant-wasm";
import model from "../../src/fixtures/statements/analyst-model.json";
import run from "../../src/fixtures/statements/analyst-goal-seek-run.json";
import captured from "../../src/fixtures/statements/analyst-goal-seek.json";
import {
  GoalSeekForm,
  type GoalSeekSubmission,
} from "@/components/finstack/statements/components/goal-seek-form/goal-seek-form";
import { goalSeekOptions } from "@/hooks/statements/use-goal-seek/use-goal-seek";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import {
  unwrap,
  type Envelope,
  type GoalSeekRequest,
  type WorkerApi,
} from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const modelJson: string = native.validateFinancialModelJson(
  JSON.stringify(model),
);
const nodeIds = Object.keys(model.nodes);
const periods = model.periods.map((period) => period.id);
const solved = captured as unknown as GoalSeekResult;
const bounds = run.bounds as [number, number];
const request: GoalSeekRequest = {
  modelJson,
  targetNode: run.targetNode,
  targetPeriod: run.targetPeriod,
  targetValue: run.targetValue,
  driverNode: run.driverNode,
  driverPeriod: run.driverPeriod,
  updateModel: false,
  bounds,
};
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
  "solves through the real worker exactly as the facade returns, with and without the updated model",
  async () => {
    const { client, calls } = recordingClient();
    const query = new QueryClient();
    try {
      const plain = await query.fetchQuery(goalSeekOptions(client, 1, request));
      // The captured gallery fixture is this same native solve.
      expect(JSON.parse(serializeHost(plain))).toEqual(captured);
      expect(plain.model).toBeNull();
      const updated = await query.fetchQuery(
        goalSeekOptions(client, 1, { ...request, updateModel: true }),
      );
      expect(serializeHost(updated)).toBe(
        serializeHost(
          native.goalSeek(modelJson, {
            target_node: run.targetNode,
            target_period: run.targetPeriod,
            target_value: run.targetValue,
            driver_node: run.driverNode,
            driver_period: run.driverPeriod,
            update_model: true,
            bounds,
          }),
        ),
      );
      // The returned model carries the solved driver value and validates natively.
      expect(updated.model!.nodes.revenue!.values!["2025Q4"]).toEqual({
        amount: "244390239.98913208",
        currency: "USD",
      });
      native.validateFinancialModelJson(serializeHost(updated.model));
      expect(calls.map(([method]) => method)).toEqual(["goalSeek", "goalSeek"]);
      expect(calls[0]![1]).toEqual(request);
      const fails = (change: Partial<GoalSeekRequest>) =>
        query.fetchQuery(goalSeekOptions(client, 1, { ...request, ...change }));
      // Unset bounds stay unset: Rust's own bracket fails for this driver.
      const { bounds: _bounds, ...unbounded } = request;
      await expect(
        query.fetchQuery(goalSeekOptions(client, 1, unbounded)),
      ).rejects.toThrow(/Goal seek failed to find solution/);
      // A cleared target or bound is NaN, which the facade's JSON boundary rejects.
      await expect(fails({ targetValue: Number.NaN })).rejects.toMatchObject({
        kind: "invalid_type",
      });
      await expect(
        fails({ bounds: [Number.NaN, 500000000] }),
      ).rejects.toMatchObject({ kind: "invalid_type" });
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it("keys the solve by every request field and the worker session, and keeps NaN distinct", () => {
  const { client } = recordingClient();
  const key = hashKey(goalSeekOptions(client, 1, request).queryKey);
  expect(hashKey(goalSeekOptions(client, 2, request).queryKey)).not.toBe(key);
  for (const field of [
    "modelJson",
    "targetNode",
    "targetPeriod",
    "driverNode",
    "driverPeriod",
  ])
    expect(
      hashKey(
        goalSeekOptions(client, 1, { ...request, [field]: "changed" }).queryKey,
      ),
    ).not.toBe(key);
  expect(
    hashKey(
      goalSeekOptions(client, 1, { ...request, updateModel: true }).queryKey,
    ),
  ).not.toBe(key);
  expect(
    hashKey(
      goalSeekOptions(client, 1, { ...request, targetValue: Number.NaN })
        .queryKey,
    ),
  ).not.toBe(
    hashKey(
      goalSeekOptions(client, 1, { ...request, targetValue: null as never })
        .queryKey,
    ),
  );
  expect(goalSeekOptions(client, 1, null).enabled).toBe(false);
});

it("submits only on Solve, as typed, leaving an empty bracket unset and a cleared value as NaN", () => {
  const submit = vi.fn<(submission: GoalSeekSubmission) => void>();
  render(
    <GoalSeekForm
      nodeIds={nodeIds}
      periods={periods}
      defaultValues={{ ...run, bounds }}
      onSubmit={submit}
      result={null}
    />,
  );
  expect(submit).not.toHaveBeenCalled();
  const solve = screen.getByRole("button", { name: "Solve" });
  fireEvent.click(solve);
  expect(submit).toHaveBeenLastCalledWith({ ...run, bounds });
  fireEvent.change(screen.getByRole("textbox", { name: "Lower bound" }), {
    target: { value: "" },
  });
  fireEvent.click(solve);
  expect(submit.mock.lastCall![0].bounds).toEqual([Number.NaN, 500000000]);
  fireEvent.change(screen.getByRole("textbox", { name: "Upper bound" }), {
    target: { value: "" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "Target value" }), {
    target: { value: "" },
  });
  fireEvent.click(solve);
  expect(submit.mock.lastCall![0]).toEqual({
    targetNode: run.targetNode,
    targetPeriod: run.targetPeriod,
    targetValue: Number.NaN,
    driverNode: run.driverNode,
    driverPeriod: run.driverPeriod,
  });
});

it("presents the returned solution and hides Apply without a handler", () => {
  render(
    <GoalSeekForm
      nodeIds={nodeIds}
      periods={periods}
      onSubmit={() => {}}
      result={solved}
    />,
  );
  const solution = screen.getByLabelText("Returned goal-seek solution");
  expect(
    [...solution.querySelectorAll("dd")].map((node) => node.textContent),
  ).toEqual([
    "244,390,239.98913208",
    "-0.000000014901161193847656",
    "0.07",
    "7",
    "Yes",
  ]);
  expect(screen.queryByRole("button", { name: "Apply to model" })).toBeNull();
});

it("applies the returned model only on an explicit Apply, and blocks a stale or model-less result", async () => {
  const updated = native.goalSeek(modelJson, {
    target_node: run.targetNode,
    target_period: run.targetPeriod,
    target_value: run.targetValue,
    driver_node: run.driverNode,
    driver_period: run.driverPeriod,
    update_model: true,
    bounds,
  }) as GoalSeekResult;
  const apply = vi.fn(async () => null);
  const { rerender } = render(
    <GoalSeekForm
      nodeIds={nodeIds}
      periods={periods}
      onSubmit={() => {}}
      result={updated}
      onApply={apply}
    />,
  );
  expect(apply).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Apply to model" }));
  await waitFor(() => expect(apply).toHaveBeenCalledWith(updated.model));
  await screen.findByText("Applied to the model.");
  rerender(
    <GoalSeekForm
      nodeIds={nodeIds}
      periods={periods}
      onSubmit={() => {}}
      result={solved}
      onApply={apply}
      applyBlocked="The model changed after this solve; solve again before applying."
    />,
  );
  const button = screen.getByRole("button", {
    name: "Apply to model",
  }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  expect(
    screen.getByText(
      "This result carries no model (solved without updating it).",
    ),
  ).toBeTruthy();
});

it("shows a native apply error as returned", async () => {
  const updated = native.goalSeek(modelJson, {
    target_node: run.targetNode,
    target_period: run.targetPeriod,
    target_value: run.targetValue,
    driver_node: run.driverNode,
    driver_period: run.driverPeriod,
    update_model: true,
    bounds,
  }) as GoalSeekResult;
  render(
    <GoalSeekForm
      nodeIds={nodeIds}
      periods={periods}
      onSubmit={() => {}}
      result={updated}
      onApply={async () => "Validation error: rejected"}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Apply to model" }));
  expect((await screen.findByRole("alert")).textContent).toBe(
    "Validation error: rejected",
  );
});
