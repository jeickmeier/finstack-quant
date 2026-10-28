import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { afterAll, beforeAll, expect, it } from "vitest";
import { QueryClient, hashKey, type QueryKey } from "@tanstack/react-query";
import { startWorker } from "../shared/worker/harness.mjs";
import {
  unwrap,
  type Envelope,
  type StatementRequest,
  type WorkerApi,
} from "@/workers/finstack-contract";
import { evaluateStatementOptions } from "@/hooks/statements/use-evaluate-statement/use-evaluate-statement";
import { rollupStatementOptions } from "@/hooks/statements/use-rollup-statement/use-rollup-statement";
import { evaluateStatementScenariosOptions } from "@/hooks/statements/use-evaluate-statement-scenarios/use-evaluate-statement-scenarios";
import { runStatementChecksOptions } from "@/hooks/statements/use-run-statement-checks/use-run-statement-checks";
import { explainStatementOptions } from "@/hooks/statements/use-explain-statement/use-explain-statement";
import { traceStatementOptions } from "@/hooks/statements/use-trace-statement/use-trace-statement";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { budget } from "../shared/budget";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const modelJson: string = native.validateFinancialModelJson(
  readFileSync(
    new URL(
      "../../src/generated/examples/financial_model_spec.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const market = readFileSync(
  new URL("../valuations/instruments/pricing-market.json", import.meta.url),
  "utf8",
);
const analystModel: string = native.validateFinancialModelJson(
  readFileSync(
    new URL(
      "../../src/fixtures/statements/analyst-model.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const analystResults = JSON.stringify(
  new native.Evaluator().evaluate(analystModel),
);
const scenarioSetJson = JSON.stringify({
  scenarios: {
    base: {},
    downside: {
      parent: "base",
      forecast_overrides: {
        revenue: { method: "growth_pct", params: { rate: -0.05 } },
      },
    },
  },
});
const specJson = JSON.stringify({
  frequency: "annual",
  methods: { debt_closing: "period_end", leverage: "formula" },
});
const configJson = JSON.stringify({
  name: "growth",
  builtin_checks: [],
  formula_checks: [
    {
      id: "yoy",
      name: "Revenue up year on year",
      category: "internal_consistency",
      severity: "error",
      formula: "revenue > lag(revenue, 4)",
      message_template: "Revenue fell year on year in {period}",
    },
  ],
});
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());

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
  "evaluates one statement request snapshot once through the real worker",
  async () => {
    const { client, calls } = recordingClient();
    const request = { modelJson };
    const options = evaluateStatementOptions(client, 1, request);
    request.modelJson = "changed after snapshot";
    const query = new QueryClient();
    try {
      expect(await query.fetchQuery(options)).toEqual(
        new native.Evaluator().evaluate(modelJson),
      );
      // The workbench seeds with explicit undefined market fields; the hook rebuilds the options.
      await query.fetchQuery(
        evaluateStatementOptions(client, 1, {
          modelJson,
          marketJson: undefined,
          asOf: undefined,
        }),
      );
      expect(calls).toEqual([["evaluateStatement", { modelJson }]]);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it("keys every request field and the worker session, and disables a null request", () => {
  const { client } = recordingClient();
  const request: StatementRequest = {
    modelJson,
    marketJson: market,
    asOf: "2025-04-02",
  };
  const key = hashKey(evaluateStatementOptions(client, 1, request).queryKey);
  expect(
    hashKey(evaluateStatementOptions(client, 2, request).queryKey),
  ).not.toBe(key);
  for (const field of Object.keys(request) as (keyof StatementRequest)[])
    expect(
      hashKey(
        evaluateStatementOptions(client, 1, { ...request, [field]: "changed" })
          .queryKey,
      ),
    ).not.toBe(key);
  expect(evaluateStatementOptions(client, 1, request).enabled).toBe(true);
  expect(evaluateStatementOptions(client, 1, null).enabled).toBe(false);
  expect(
    hashKey(evaluateStatementOptions(client, 1, { modelJson }).queryKey),
  ).toBe(
    hashKey(
      evaluateStatementOptions(client, 1, {
        modelJson,
        marketJson: undefined,
        asOf: undefined,
      }).queryKey,
    ),
  );
});

/** Every request field and the worker session key the query; a null request is disabled. */
function expectKeyed<R extends object>(
  options: (
    client: FinstackClient,
    session: number,
    request: R | null,
  ) => { queryKey: QueryKey; enabled?: unknown },
  request: R,
) {
  const { client } = recordingClient();
  const key = hashKey(options(client, 1, request).queryKey);
  expect(hashKey(options(client, 2, request).queryKey)).not.toBe(key);
  for (const field of Object.keys(request))
    expect(
      hashKey(options(client, 1, { ...request, [field]: "changed" }).queryKey),
    ).not.toBe(key);
  expect(options(client, 1, request).enabled).toBe(true);
  expect(options(client, 1, null).enabled).toBe(false);
}

it(
  "rolls up one statement request snapshot once through the real worker",
  async () => {
    const { client, calls } = recordingClient();
    const request = {
      modelJson: analystModel,
      resultsJson: analystResults,
      specJson,
    };
    const options = rollupStatementOptions(client, 1, request);
    request.specJson = "changed after snapshot";
    const query = new QueryClient();
    try {
      expect(await query.fetchQuery(options)).toEqual(
        native.statementResultRollup(analystResults, analystModel, specJson),
      );
      await query.fetchQuery(
        rollupStatementOptions(client, 1, { ...request, specJson }),
      );
      expect(calls).toEqual([
        [
          "rollupStatement",
          { modelJson: analystModel, resultsJson: analystResults, specJson },
        ],
      ]);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it(
  "evaluates one scenario request snapshot once through the real worker",
  async () => {
    const { client, calls } = recordingClient();
    const request = { modelJson: analystModel, scenarioSetJson };
    const options = evaluateStatementScenariosOptions(client, 1, request);
    request.scenarioSetJson = "changed after snapshot";
    const query = new QueryClient();
    try {
      expect(await query.fetchQuery(options)).toEqual(
        native.evaluateScenarioSet(analystModel, scenarioSetJson),
      );
      await query.fetchQuery(
        evaluateStatementScenariosOptions(client, 1, {
          modelJson: analystModel,
          scenarioSetJson,
          marketJson: undefined,
          asOf: undefined,
        }),
      );
      expect(calls).toEqual([
        [
          "evaluateStatementScenarios",
          { modelJson: analystModel, scenarioSetJson },
        ],
      ]);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it(
  "runs one check request snapshot once through the real worker",
  async () => {
    const { client, calls } = recordingClient();
    const request = {
      modelJson: analystModel,
      resultsJson: analystResults,
      kind: "suite" as const,
      configJson,
    };
    const options = runStatementChecksOptions(client, 1, request);
    request.configJson = "changed after snapshot";
    const query = new QueryClient();
    try {
      expect(await query.fetchQuery(options)).toEqual(
        native.runChecks(
          analystModel,
          native.validateCheckSuiteSpecJson(configJson),
          analystResults,
        ),
      );
      await query.fetchQuery(
        runStatementChecksOptions(client, 1, { ...request, configJson }),
      );
      expect(calls).toEqual([
        ["runStatementChecks", { ...request, configJson }],
      ]);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it("keys rollup, scenario and check queries by every request field and the worker session", () => {
  expectKeyed(rollupStatementOptions, {
    modelJson: analystModel,
    resultsJson: analystResults,
    specJson,
  });
  expectKeyed(evaluateStatementScenariosOptions, {
    modelJson: analystModel,
    scenarioSetJson,
    marketJson: market,
    asOf: "2025-07-01",
  });
  expectKeyed(runStatementChecksOptions, {
    modelJson: analystModel,
    resultsJson: analystResults,
    kind: "suite",
    configJson,
  });
});

it(
  "explains one statement cell snapshot once through the real worker",
  async () => {
    const { client, calls } = recordingClient();
    const request = {
      modelJson: analystModel,
      resultsJson: analystResults,
      nodeId: "gross_profit",
      period: "2024Q2",
    };
    const options = explainStatementOptions(client, 1, request);
    request.nodeId = "changed after snapshot";
    const query = new QueryClient();
    try {
      expect(await query.fetchQuery(options)).toEqual(
        native.explainFormula(
          analystModel,
          analystResults,
          "gross_profit",
          "2024Q2",
        ),
      );
      await query.fetchQuery(
        explainStatementOptions(client, 1, {
          ...request,
          nodeId: "gross_profit",
        }),
      );
      expect(calls).toEqual([
        ["explainStatement", { ...request, nodeId: "gross_profit" }],
      ]);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it(
  "traces one dependency tree snapshot once through the real worker",
  async () => {
    const { client, calls } = recordingClient();
    const request = { modelJson: analystModel, nodeId: "ebitda" };
    const options = traceStatementOptions(client, 1, request);
    request.nodeId = "changed after snapshot";
    const query = new QueryClient();
    try {
      expect(await query.fetchQuery(options)).toEqual(
        new native.DependencyTracer(analystModel).dependencyTree("ebitda"),
      );
      await query.fetchQuery(
        traceStatementOptions(client, 1, {
          modelJson: analystModel,
          nodeId: "ebitda",
        }),
      );
      expect(calls).toEqual([
        ["traceStatement", { modelJson: analystModel, nodeId: "ebitda" }],
      ]);
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it("keys explanation and trace queries by every request field and the worker session", () => {
  expectKeyed(explainStatementOptions, {
    modelJson: analystModel,
    resultsJson: analystResults,
    nodeId: "gross_profit",
    period: "2024Q2",
  });
  expectKeyed(traceStatementOptions, {
    modelJson: analystModel,
    nodeId: "ebitda",
  });
});
