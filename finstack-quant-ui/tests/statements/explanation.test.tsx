// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { createRequire } from "node:module";
import type { DependencyTree } from "finstack-quant-wasm";
import model from "../../src/fixtures/statements/analyst-model.json";
import result from "../../src/fixtures/statements/analyst-result.json";
import { StatementExplanation } from "@/components/finstack/statements/components/statement-diagnostics/statement-explanation";
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
const period = "2025Q4";
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  const call = vi.fn(async (method: string, arg: any) => {
    switch (method) {
      case "initialize":
        return { state: "ready", worker: true };
      case "explainStatement":
        return native.explainFormula(
          arg.modelJson,
          arg.resultsJson,
          arg.nodeId,
          arg.period,
        );
      case "traceStatement":
        return new native.DependencyTracer(arg.modelJson).dependencyTree(
          arg.nodeId,
        );
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
function renderExplanation(nodeId: string) {
  render(
    <FinstackQueryProvider>
      <StatementExplanation
        request={{ modelJson, resultsJson, nodeId, period }}
      />
    </FinstackQueryProvider>,
  );
}
/** Native node IDs in pre-order, one entry per occurrence. */
const preorder = (tree: DependencyTree): string[] => [
  tree.node_id,
  ...tree.children.flatMap(preorder),
];

it("lists every occurrence of a recurring dependency in the native trace, in pre-order", async () => {
  renderExplanation("gross_profit");
  const table = await screen.findByRole("table", {
    name: "Native dependency tree",
  });
  const tree: DependencyTree = new native.DependencyTracer(
    modelJson,
  ).dependencyTree("gross_profit");
  const ids = preorder(tree);
  // revenue is reached directly and again through cogs.
  expect(new Set(ids).size).toBeLessThan(ids.length);
  const body = within(table).getAllByRole("row").slice(1);
  expect(body).toHaveLength(ids.length);
  expect(
    body.map((row) => within(row).getAllByRole("cell")[0].textContent),
  ).toEqual(ids);
  const explanation = screen.getByRole("region", {
    name: "Statement explanation",
  });
  expect(within(explanation).queryByText(/Forecast method/)).toBeNull();
});

it("names the native source and forecast method of a forecast cell", async () => {
  const { source, forecast } = native.explainFormula(
    modelJson,
    resultsJson,
    "revenue",
    period,
  );
  expect(source).toBe("forecast");
  renderExplanation("revenue");
  const explanation = await screen.findByRole("region", {
    name: "Statement explanation",
  });
  const method = await within(explanation).findByText(/Forecast method/);
  expect(method.textContent).toBe(`Forecast method: ${forecast.method}`);
  expect(within(explanation).getByText(/Returned value/).textContent).toMatch(
    new RegExp(` · ${source}$`),
  );
});
