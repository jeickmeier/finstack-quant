// @vitest-environment jsdom
import { beforeAll, afterAll, afterEach, it, expect, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  render,
  screen,
  cleanup,
  waitFor,
  within,
  fireEvent,
} from "@testing-library/react";
import { FxSurfaceChart } from "@/components/finstack/models/components/fx-surface-chart/fx-surface-chart";
import { useLinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import { startWorker } from "../shared/worker/harness.mjs";
import { unwrap } from "@/workers/finstack-contract";
import { fxQuotes } from "./surfaces/fixtures";
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
    unwrap(await worker.proxy[method as "sampleFxDelta"](arg)),
  );
}, budget(30000));
afterAll(async () => worker?.close());
afterEach(() => {
  cleanup();
  transport.call.mockClear();
});
it("makes no request without every forward, samples only explicit coordinates, and shows native errors", async () => {
  const cache = new QueryClient();
  const coordinates = [
    { expiry: 0.5, strike: 1.1, forward: undefined as number | undefined },
  ];
  const view = (points = coordinates) => (
    <QueryClientProvider client={cache}>
      <FxSurfaceChart
        surface={fxQuotes}
        coordinates={points}
        colorDomain={[0, 0.3]}
      />
    </QueryClientProvider>
  );
  const mounted = render(view());
  expect(screen.getByText(/Supply an explicit forward/)).toBeTruthy();
  expect(transport.call).not.toHaveBeenCalled();
  mounted.rerender(view([{ expiry: 0.5, strike: 1.1, forward: 1.12 }]));
  await screen.findByRole("region", { name: "EURUSD evaluated surface" });
  expect(transport.call).toHaveBeenCalledTimes(1);
  expect(transport.call.mock.calls[0]![1].coordinates).toEqual([
    { expiry: 0.5, strike: 1.1, forward: 1.12 },
  ]);
  expect(
    screen.getByText(/evaluated samples, not stored quote nodes/),
  ).toBeTruthy();
  mounted.rerender(view([{ expiry: 0.5, strike: 1.1, forward: -1 }]));
  await waitFor(() =>
    expect(screen.getByRole("alert").textContent).toBe(
      "Values must be positive",
    ),
  );
  expect(
    screen.queryByRole("region", { name: "EURUSD evaluated surface" }),
  ).toBeNull();
  mounted.rerender(view());
  expect(screen.queryByRole("alert")).toBeNull();
  expect(transport.call).toHaveBeenCalledTimes(2);
  cache.clear();
});
it("selects an evaluated sample by its FX surface, expiry and strike key", async () => {
  const cache = new QueryClient();
  const coordinates = [
    { expiry: 0.5, strike: 1.1, forward: 1.12 },
    { expiry: 1, strike: 1.15, forward: 1.13 },
  ];
  function App() {
    const link = useLinkedSelection();
    return (
      <QueryClientProvider client={cache}>
        <output aria-label="key">{link.selectedKey}</output>
        <FxSurfaceChart
          surface={fxQuotes}
          coordinates={coordinates}
          link={link}
          colorDomain={[0, 0.3]}
        />
      </QueryClientProvider>
    );
  }
  render(<App />);
  const surface = await screen.findByRole("region", {
    name: "EURUSD evaluated surface",
  });
  const sample = within(surface)
    .getAllByRole("row")
    .map((row) => [...row.querySelectorAll("td")])
    .find(
      (cells) =>
        cells.length === 3 &&
        cells[0]!.textContent === "1" &&
        cells[1]!.textContent === "1.15",
    )!;
  expect(screen.getByLabelText("key").textContent).toBe("");
  fireEvent.click(sample[2]!);
  expect(screen.getByLabelText("key").textContent).toBe(
    JSON.stringify([fxQuotes.id, 1, 1.15]),
  );
  cache.clear();
});
