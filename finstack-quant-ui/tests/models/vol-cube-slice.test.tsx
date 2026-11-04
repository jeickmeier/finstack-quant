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
import userEvent from "@testing-library/user-event";
import { VolCubeSlice } from "@/components/finstack/models/components/vol-cube-slice/vol-cube-slice";
import { VolCubeExplorer } from "@/components/finstack/models/components/vol-cube-explorer/vol-cube-explorer";
import { startWorker } from "../shared/worker/harness.mjs";
import { unwrap, type CubeSliceRequest } from "@/workers/finstack-contract";
import fixtures from "../core/cubes/cases.json";
import { budget } from "../shared/budget";
const transport = vi.hoisted(() => ({ call: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/use-finstack", () => ({
  useFinstack: () => ({
    client: transport,
    status: "ready",
    session: 1,
    error: null,
  }),
}));
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
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
  transport.call.mockImplementation(async (method, arg) =>
    unwrap(await worker.proxy[method as "cubeSlice"](arg)),
  );
});
afterAll(async () => worker?.close());
afterEach(() => {
  cleanup();
  transport.call.mockClear();
});
const cube = fixtures.market.vol_cubes.find(
  (c) => c.id === "USD-SWAPTION-NORMAL-VOL",
)! as CubeSliceRequest["cube"];
const strikes = [0.03, 0.04, 0.05, 0.06, 0.07];
const colorDomains = {
  normal: [0, 0.02],
  black_lognormal: [0, 1],
} as const;
const calls = (method: string) =>
  transport.call.mock.calls
    .filter(([m]) => m === method)
    .map(([, request]) => request);
it("slices the cube natively at the seeded strikes, fixed axis and node", async () => {
  const cache = new QueryClient();
  render(
    <QueryClientProvider client={cache}>
      <VolCubeSlice
        cube={cube}
        initialStrikes={strikes}
        initialConvention="normal"
        colorDomains={colorDomains}
      />
    </QueryClientProvider>,
  );
  const region = await screen.findByRole("region", {
    name: `${cube.id} tenor slice evaluated surface`,
  });
  expect(calls("cubeSlice")[0]).toEqual({
    cube,
    axis: "tenor",
    at: cube.tenors[0],
    strikes,
    convention: "normal",
  });
  // No rate was supplied, so no smile is requested.
  expect(calls("cubeSmile")).toHaveLength(0);
  expect(
    screen.getByText(/Enter every strike and a continuously/),
  ).toBeTruthy();
  const expected = unwrap(await worker.proxy.cubeSlice(calls("cubeSlice")[0]!));
  if (expected.axis !== "tenor") throw new Error("expected a tenor slice");
  expect(
    within(region).getAllByText(String(expected.surface.vols_row_major[0]))
      .length,
  ).toBeGreaterThan(0);
  fireEvent.click(screen.getByRole("radio", { name: "Option expiry" }));
  await screen.findByRole("region", {
    name: `${cube.id} expiry slice evaluated surface`,
  });
  expect(calls("cubeSlice").at(-1)).toMatchObject({
    axis: "expiry",
    at: cube.expiries[0],
  });
  fireEvent.click(screen.getByRole("radio", { name: "Black/lognormal" }));
  await waitFor(() =>
    expect(calls("cubeSlice").at(-1)).toMatchObject({
      convention: "black_lognormal",
    }),
  );
  const count = transport.call.mock.calls.length;
  const second = screen.getByLabelText("Strike 2");
  fireEvent.focus(second);
  fireEvent.change(second, { target: { value: "-" } });
  expect(
    screen.getByText(/Enter every strike as a finite decimal/),
  ).toBeTruthy();
  expect(transport.call).toHaveBeenCalledTimes(count);
  // Strike order is validated by Rust, and its error is shown as returned.
  fireEvent.change(second, { target: { value: "0.02" } });
  expect((await screen.findByRole("alert")).textContent).toMatch(
    /strictly increasing/,
  );
  cache.clear();
});
it("evaluates the node smile only with an explicit rate and shows the native repair", async () => {
  const cache = new QueryClient();
  render(
    <QueryClientProvider client={cache}>
      <VolCubeSlice
        cube={cube}
        initialStrikes={strikes}
        initialConvention="normal"
        initialRate={0.04}
        repairIterations={10}
        colorDomains={colorDomains}
      />
    </QueryClientProvider>,
  );
  const table = await screen.findByRole("table", {
    name: "Node smile values",
  });
  expect(calls("cubeSmile")[0]).toEqual({
    cube,
    expiryIndex: 0,
    tenorIndex: 0,
    strikes,
    rate: 0.04,
    repairIterations: 10,
  });
  const native = unwrap(await worker.proxy.cubeSmile(calls("cubeSmile")[0]!));
  const rows = within(table).getAllByRole("row").slice(1);
  expect(
    rows.map((row) =>
      within(row)
        .getAllByRole("cell")
        .map((cell) => cell.textContent),
    ),
  ).toEqual(
    strikes.map((strike, i) => [
      String(strike),
      String(native.vols[i]),
      String(native.repaired![i]),
    ]),
  );
  expect(
    screen.getByText(/no butterfly or monotonicity violations/),
  ).toBeTruthy();
  expect(
    screen.getByRole("img", { name: `${cube.id} node smile` }),
  ).toBeTruthy();
  await userEvent.click(
    screen.getByRole("combobox", { name: "Tenor node (years)" }),
  );
  await userEvent.click(await screen.findByRole("option", { name: "10" }));
  await waitFor(() =>
    expect(calls("cubeSmile").at(-1)).toMatchObject({ tenorIndex: 2 }),
  );
  const rate = screen.getByLabelText(/Rate for the arbitrage check/);
  fireEvent.focus(rate);
  fireEvent.change(rate, { target: { value: "" } });
  expect(
    screen.getByText(/Enter every strike and a continuously/),
  ).toBeTruthy();
  cache.clear();
});
it("mounts the slice in the explorer only when strikes are supplied", async () => {
  const cache = new QueryClient();
  const view = (slice?: { initialStrikes: number[] }) => (
    <QueryClientProvider client={cache}>
      <VolCubeExplorer
        cube={cube}
        initialStrike={0.05}
        initialConvention="normal"
        colorDomains={colorDomains}
        slice={slice}
      />
    </QueryClientProvider>
  );
  const bare = render(view());
  expect(screen.queryByRole("region", { name: `${cube.id} cube slice` })).toBe(
    null,
  );
  bare.unmount();
  render(view({ initialStrikes: strikes }));
  await screen.findByRole("region", {
    name: `${cube.id} tenor slice evaluated surface`,
  });
  expect(calls("cubeSlice").at(-1)).toMatchObject({ strikes });
  cache.clear();
});
