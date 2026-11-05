// @vitest-environment jsdom
import { beforeAll, afterAll, afterEach, it, expect, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  render,
  screen,
  cleanup,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SurfaceArbitrageBadge } from "@/components/finstack/models/components/surface-arbitrage-badge/surface-arbitrage-badge";
import { startWorker } from "../shared/worker/harness.mjs";
import { unwrap, type CheckSurfaceRequest } from "@/workers/finstack-contract";
import calibration from "../calibration/cases.json";
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
let worker: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  worker = await startWorker();
  transport.call.mockImplementation(async (method, arg) =>
    unwrap(await worker.proxy[method as "checkSurface"](arg)),
  );
});
afterAll(async () => worker?.close());
afterEach(() => {
  cleanup();
  transport.call.mockClear();
});
const surface = (
  calibration.find((c) => c.source.includes("08_equity"))!.result.result
    .final_market.surfaces as CheckSurfaceRequest["surface"][]
).find((s) => s.id === "AAPL-EQUITY-VOL")!;
const requests = () =>
  transport.call.mock.calls
    .filter(([method]) => method === "checkSurface")
    .map(([, request]) => request);
function mount(props: Parameters<typeof SurfaceArbitrageBadge>[0]) {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <SurfaceArbitrageBadge {...props} />
    </QueryClientProvider>,
  );
}
const trigger = () => screen.getByRole("button", { name: "Arbitrage details" });
/** The badge row: label, status badge and the popover trigger. */
const summary = () => trigger().parentElement!;
it("shows a passed check of the supplied config and sends it unchanged", async () => {
  mount({ surface, config: { forward_prices: [175] } });
  await waitFor(() => expect(summary().textContent).toContain("Passed"), {
    timeout: budget(5000),
  });
  expect(requests()).toEqual([{ surface, config: { forward_prices: [175] } }]);
  await userEvent.click(trigger());
  const dialog = await screen.findByRole("dialog");
  expect(within(dialog).getByText("No violations returned")).toBeTruthy();
  cleanup();
  transport.call.mockClear();
  mount({ surface, config: { forward_prices: [175], tolerance: 1e-8 } });
  await waitFor(() => expect(summary().textContent).toContain("Passed"));
  expect(requests()).toEqual([
    { surface, config: { forward_prices: [175], tolerance: 1e-8 } },
  ]);
});
it("counts an injected butterfly violation by type and severity and lists each violation", async () => {
  const bumped = {
    ...surface,
    vols_row_major: surface.vols_row_major.map((v, i) => (i === 4 ? 0.4 : v)),
  };
  const report = unwrap(
    await worker.proxy.checkSurface({
      surface: bumped,
      config: { forward_prices: [175] },
    }),
  );
  mount({ surface: bumped, config: { forward_prices: [175] } });
  await waitFor(() => expect(summary().textContent).toContain("6 violations"), {
    timeout: budget(5000),
  });
  await userEvent.click(trigger());
  const dialog = await screen.findByRole("dialog");
  const byType = within(dialog).getByRole("table", {
    name: "Violations by type",
  });
  expect(within(byType).getByRole("row", { name: "Butterfly 1" })).toBeTruthy();
  const bySeverity = within(dialog).getByRole("table", {
    name: "Violations by severity",
  });
  expect(
    within(bySeverity).getByRole("row", { name: "Critical 6" }),
  ).toBeTruthy();
  const list = within(dialog).getByRole("table", {
    name: "Violations, most severe first",
  });
  // Header plus one row per returned violation, in Rust's order.
  const rows = within(list).getAllByRole("row").slice(1);
  expect(rows).toHaveLength(report.violations.length);
  expect(rows[0]!.textContent).toContain(report.violations[0]!.description);
});
it("shows the native convention error of a normal-quoted surface as not checked", async () => {
  mount({
    surface: { ...surface, quote_type: "normal" },
    config: { forward_prices: [175] },
  });
  await waitFor(() => expect(summary().textContent).toContain("Not checked"), {
    timeout: budget(5000),
  });
  await userEvent.click(trigger());
  const dialog = await screen.findByRole("dialog");
  expect(within(dialog).getByRole("alert").textContent).toContain(
    "stores 'normal' quotes",
  );
});
it("requests nothing when the host supplies no check config", () => {
  mount({ surface });
  expect(summary().textContent).toContain("Forwards not supplied");
  mount({ surface, config: null });
  expect(screen.getAllByText("Forwards not supplied")).toHaveLength(2);
  expect(requests()).toEqual([]);
});
