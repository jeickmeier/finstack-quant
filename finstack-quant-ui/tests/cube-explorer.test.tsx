// @vitest-environment jsdom
import { beforeAll, afterAll, afterEach, it, expect, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  render,
  screen,
  cleanup,
  fireEvent,
  waitFor,
  within,
} from "@testing-library/react";
import { VolCubeExplorer } from "../registry/components/vol-cube-explorer/vol-cube-explorer";
import { startWorker } from "./worker/harness.mjs";
import {
  unwrap,
  type CubeSampleRequest,
} from "../registry/workers/finstack-contract";
import fixtures from "./cubes/cases.json";
const transport = vi.hoisted(() => ({ call: vi.fn() }));
vi.mock("../registry/hooks/use-finstack/use-finstack", () => ({
  useFinstack: () => ({
    client: transport,
    status: "ready",
    session: 1,
    error: null,
  }),
}));
vi.mock(
  "../registry/primitives/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: (props: { ariaLabel: string }) => (
      <div role="img" aria-label={props.ariaLabel} />
    ),
  }),
);
let worker: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  worker = await startWorker();
  transport.call.mockImplementation(async (_method, arg) =>
    unwrap(await worker.proxy.sampleCube(arg)),
  );
}, 30000);
afterAll(async () => worker?.close());
afterEach(() => {
  cleanup();
  transport.call.mockClear();
});
it("preserves raw parameter/forward data, selects explicit evaluator and strike, and suppresses incomplete edits", async () => {
  const cube = fixtures.market.vol_cubes.find(
    (c) => c.id !== "SHIFTED-BLACK",
  )! as CubeSampleRequest["cube"];
  const cache = new QueryClient();
  render(
    <QueryClientProvider client={cache}>
      <VolCubeExplorer
        cube={cube}
        initialStrike={0.05}
        initialConvention="normal"
        colorDomains={{ normal: [0, 0.02], black_lognormal: [0, 1] }}
      />
    </QueryClientProvider>,
  );
  await screen.findByRole("region", { name: `${cube.id} evaluated surface` });
  expect(transport.call.mock.calls[0]![1].convention).toBe("normal");
  const raw = screen.getByRole("table", { name: "Raw cube nodes" });
  const first = within(raw).getAllByRole("row")[1]!;
  expect(
    within(first)
      .getAllByRole("cell")
      .map((cell) => cell.textContent),
  ).toEqual([
    String(cube.expiries[0]),
    String(cube.tenors[0]),
    String(cube.forwards[0]),
    ...(["alpha", "beta", "rho", "nu", "shift"] as const).map((key) =>
      String(cube.params[0]![key] ?? "Not supplied"),
    ),
  ]);
  expect(
    JSON.parse(
      screen
        .getByRole("region", { name: "Complete cube state" })
        .querySelector("pre")!.textContent!,
    ),
  ).toEqual(cube);
  fireEvent.click(screen.getByRole("radio", { name: "Black/lognormal" }));
  await waitFor(() =>
    expect(transport.call.mock.calls.at(-1)![1].convention).toBe(
      "black_lognormal",
    ),
  );
  const strike = screen.getByLabelText("Absolute strike (rate units)");
  fireEvent.focus(strike);
  fireEvent.change(strike, { target: { value: "0.051" } });
  await waitFor(() =>
    expect(
      transport.call.mock.calls
        .at(-1)![1]
        .coordinates.every((p: { strike: number }) => p.strike === 0.051),
    ).toBe(true),
  );
  const count = transport.call.mock.calls.length;
  fireEvent.change(strike, { target: { value: "-" } });
  expect(screen.getByText(/Enter a finite absolute strike/)).toBeTruthy();
  expect(transport.call).toHaveBeenCalledTimes(count);
  cache.clear();
});
