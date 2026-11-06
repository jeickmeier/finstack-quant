// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  curveQueryOptions,
  useCurveQuery,
  type CurveQueryRequest,
} from "@/hooks/core/use-curve-query/use-curve-query";
import {
  CurveQuery,
  curveQueryTarget,
  type CurveLookup,
  type CurveQueryData,
} from "@/components/finstack/core/components/curve-query/curve-query";
import { MarketContextBrowser } from "@/components/finstack/core/components/market-context-browser/market-context-browser";
import type { MarketContextStateWire } from "../../../src/generated/types/market_context_state";
import { unwrap } from "@/workers/finstack-contract";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<
      typeof import("@/components/finstack/shared/chart/finstack-chart/finstack-chart")
    >()),
    FinstackChart: () => <div>Native chart</div>,
  }),
);
const harness = useWorkerHarness(["curveQuery"]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
type Case = { request: CurveLookup; result: CurveQueryData };
const cases = fixture.cases as unknown as Record<string, Case>;
const marketJson = JSON.stringify(fixture.market);
const market = fixture.market as unknown as MarketContextStateWire;
const request = (lookup: CurveLookup) =>
  ({ ...lookup, marketJson }) as unknown as CurveQueryRequest;
const sixPlaces = (value: number) => value.toFixed(6);
const native = (outcome: unknown) => {
  const value = outcome as { ok: boolean; value: number };
  if (!value.ok) throw new Error("Rust rejected a point this test reads");
  return value.value;
};
// Results are regenerated and compared by `mise run ui-fixtures --check`
// (tests/core/curve-query/generate.mjs).
it.each(Object.keys(cases))(
  "returns the %s native values through the real worker",
  async (name) => {
    const { request: lookup, result } = cases[name]!;
    const envelope = await harness.proxy.curveQuery(request(lookup));
    expect(unwrap(envelope)).toEqual(result);
  },
  budget(15000),
);
it("fails the whole lookup on an id the market does not hold", async () => {
  const envelope = await harness.proxy.curveQuery(
    request({ ...cases.discount!.request, id: "NO-SUCH-CURVE" }),
  );
  expect(envelope).toEqual(fixture.unknownId);
});
it("rejects the NaN of a malformed number in Rust, per point", async () => {
  const curve = await harness.proxy.curveQuery(
    request({
      kind: "discount",
      id: "USD-OIS",
      points: [],
      forwards: [[Number.NaN, 2]],
    }),
  );
  const { forwards } = unwrap(curve) as {
    forwards: { rate: { ok: boolean } }[];
  };
  expect(forwards[0]!.rate.ok).toBe(false);
  const surface = await harness.proxy.curveQuery(
    request({
      kind: "surface",
      id: "EQ-VOL",
      points: [{ expiry: 1, strike: Number.NaN }],
    }),
  );
  const { points } = unwrap(surface) as { points: { vol: { ok: boolean } }[] };
  expect(points[0]!.vol.ok).toBe(false);
});
it("keeps the cached market usable after freeing each curve handle", async () => {
  const { request: lookup, result } = cases.hazard!;
  await harness.proxy.curveQuery(request(lookup));
  expect(unwrap(await harness.proxy.curveQuery(request(lookup)))).toEqual(
    result,
  );
});
it("keys the query on the complete request and waits for one", () => {
  const full = request(cases.surface!.request);
  const options = curveQueryOptions(null, 2, full);
  expect(options.queryKey).toEqual(["finstack", 2, "curveQuery", full]);
  expect(curveQueryOptions(null, 2, null).enabled).toBe(false);
});
it("names the queryable target of a browser entry, and nothing else", () => {
  const curve = (id: string) =>
    market.curves.find((stored) => stored.id === id);
  expect(
    curveQueryTarget({ path: ["curves", 3], value: curve("USD-OIS") }),
  ).toEqual({ kind: "discount", id: "USD-OIS" });
  expect(
    curveQueryTarget({ path: ["curves", 0], value: curve("FORWARD") }),
  ).toEqual({ kind: "forward", id: "FORWARD" });
  expect(curveQueryTarget({ path: ["curves", 1], value: curve("HZ") })).toEqual(
    { kind: "hazard", id: "HZ" },
  );
  expect(
    curveQueryTarget({ path: ["curves", 2], value: curve("PRICE") }),
  ).toBeNull();
  expect(
    curveQueryTarget({ path: ["surfaces", 0], value: market.surfaces[0] }),
  ).toEqual({ kind: "surface", id: "EQ-VOL" });
  expect(curveQueryTarget({ path: ["fx"], value: market.fx })).toBeNull();
});
it("submits dates untouched, years as numbers and forward intervals in years", async () => {
  const onSubmit = vi.fn();
  render(
    <CurveQuery
      target={{ kind: "discount", id: "USD-OIS" }}
      defaultPoints="0.5, 2030-06-15, not-a-date"
      defaultForwards={"1, 2\n\n2, 5"}
      onSubmit={onSubmit}
      result={null}
    />,
  );
  await userEvent.click(screen.getByRole("button", { name: "Look up" }));
  expect(onSubmit).toHaveBeenCalledWith({
    kind: "discount",
    id: "USD-OIS",
    points: [0.5, "2030-06-15", "not-a-date"],
    forwards: [
      [1, 2],
      [2, 5],
    ],
  });
});
it("submits one expiry and strike per line, a missing strike as NaN", async () => {
  const onSubmit = vi.fn();
  render(
    <CurveQuery
      target={{ kind: "surface", id: "EQ-VOL" }}
      defaultPoints={"0.75, 100\n1.5"}
      onSubmit={onSubmit}
      result={null}
    />,
  );
  await userEvent.click(screen.getByRole("button", { name: "Look up" }));
  const { points } = onSubmit.mock.lastCall![0];
  expect(points[0]).toEqual({ expiry: 0.75, strike: 100 });
  expect(points[1].expiry).toBe(1.5);
  expect(points[1].strike).toBeNaN();
});
it("offers forward intervals on a discount curve only", () => {
  render(
    <CurveQuery
      target={{ kind: "hazard", id: "HZ" }}
      onSubmit={() => undefined}
      result={null}
    />,
  );
  expect(screen.queryByLabelText(/Forward intervals/)).toBeNull();
  expect(screen.getByRole("textbox", { name: /^Points/ })).toBeTruthy();
});
function table(name: string) {
  return within(screen.getByRole("table", { name }));
}
it("shows the native discount values and forwards, echoing each point as entered", () => {
  const { result } = cases.discount!;
  render(
    <CurveQuery
      target={{ kind: "discount", id: "USD-OIS" }}
      onSubmit={() => undefined}
      result={result}
      formatValue={sixPlaces}
    />,
  );
  if (result.kind !== "discount") throw new Error("Discount fixture expected");
  const rows = table("Curve values").getAllByRole("row");
  expect(rows).toHaveLength(5);
  const dated = result.points[3]!;
  expect(rows[4]!.textContent).toContain("2030-06-15");
  expect(rows[4]!.textContent).toContain(sixPlaces(native(dated.df)));
  expect(rows[4]!.textContent).toContain(sixPlaces(native(dated.zeroRate)));
  const forwards = table("Forward rates").getAllByRole("row");
  expect(forwards[1]!.textContent).toContain(
    sixPlaces(native(result.forwards[0]!.rate)),
  );
  expect(
    table("Curve values").getByRole("columnheader", {
      name: /Zero rate \(continuously compounded\)/,
    }),
  ).toBeTruthy();
});
it.each([
  ["forward", "FORWARD", "Forward rate (simple, on the curve day count)"],
  ["hazard", "HZ", "Survival probability"],
  ["surface", "EQ-VOL", "Volatility"],
] as const)("shows the native %s values", (kind, id, header) => {
  const { result } = cases[kind]!;
  render(
    <CurveQuery
      target={{ kind, id }}
      onSubmit={() => undefined}
      result={result}
      formatValue={sixPlaces}
    />,
  );
  const name = kind === "surface" ? "Surface values" : "Curve values";
  expect(
    table(name).getByRole("columnheader", {
      name: (name) => name.includes(header),
    }),
  ).toBeTruthy();
  expect(table(name).getAllByRole("row")).toHaveLength(
    result.points.length + 1,
  );
  const measure = Object.values(result.points[0]!).find(
    (value) => typeof value === "object",
  );
  expect(table(name).getAllByRole("row")[1]!.textContent).toContain(
    sixPlaces(native(measure)),
  );
});
it("keeps each rejected point's native error in its own cell beside the rest", () => {
  const { result } = cases.rejected_points!;
  if (result.kind !== "discount") throw new Error("Discount fixture expected");
  render(
    <CurveQuery
      target={{ kind: "discount", id: "USD-OIS" }}
      onSubmit={() => undefined}
      result={result}
      formatValue={sixPlaces}
    />,
  );
  const rows = table("Curve values").getAllByRole("row");
  expect(rows[1]!.textContent).toContain(
    sixPlaces(native(result.points[0]!.df)),
  );
  const bad = result.points[1]!.df;
  if (bad.ok) throw new Error("Rust should reject this date");
  expect(within(rows[2]!).getAllByRole("alert")[0]!.textContent).toBe(
    bad.error.message,
  );
  const reversed = result.forwards[0]!.rate;
  if (reversed.ok) throw new Error("Rust should reject this interval");
  expect(table("Forward rates").getAllByRole("alert")[0]!.textContent).toBe(
    reversed.error.message,
  );
});
it("keeps a coordinate outside the surface grid as its native error", () => {
  const { result } = cases.rejected_coordinate!;
  render(
    <CurveQuery
      target={{ kind: "surface", id: "EQ-VOL" }}
      onSubmit={() => undefined}
      result={result}
    />,
  );
  expect(screen.getByRole("alert").textContent).toContain("out of bounds");
});
it("surfaces the native error of the whole lookup and the loading and empty states", () => {
  const target = { kind: "discount", id: "NO-SUCH-CURVE" } as const;
  const { rerender } = render(
    <CurveQuery target={target} onSubmit={() => undefined} result={null} />,
  );
  expect(screen.getByRole("status").textContent).toBe("No lookup result");
  rerender(
    <CurveQuery
      target={target}
      onSubmit={() => undefined}
      result={null}
      loading
    />,
  );
  expect(screen.getByRole("status").textContent).toBe("Looking up points…");
  const { unknownId } = fixture;
  if (unknownId.ok) throw new Error("Rust should reject this id");
  rerender(
    <CurveQuery
      target={target}
      onSubmit={() => undefined}
      result={null}
      error={unknownId.error!.message}
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(unknownId.error!.message);
});
function Mounted({ lookup }: { lookup: CurveLookup }) {
  const query = useCurveQuery(request(lookup));
  const { id, kind } = lookup;
  return (
    <CurveQuery
      target={{ id, kind }}
      onSubmit={() => undefined}
      result={query.data ?? null}
      loading={query.isFetching}
      error={query.error?.message}
      formatValue={sixPlaces}
    />
  );
}
it("mounts in the browser's slot and reads the selected curve through the worker", async () => {
  render(
    <FinstackQueryProvider>
      <MarketContextBrowser
        state={market}
        renderObject={(entry) => {
          const target = curveQueryTarget(entry);
          if (!target) return undefined;
          const name = target.kind === "hazard" ? "hazard" : "discount";
          return target.kind === name ? (
            <Mounted key={target.id} lookup={cases[name]!.request} />
          ) : undefined;
        }}
      />
    </FinstackQueryProvider>,
  );
  // USD-OIS, the first stored discount curve, is the browser's own first selection.
  const first = await screen.findByRole(
    "table",
    { name: "Curve values" },
    { timeout: budget(15000) },
  );
  const discount = cases.discount!.result;
  if (discount.kind !== "discount")
    throw new Error("Discount fixture expected");
  expect(first.textContent).toContain(
    sixPlaces(native(discount.points[0]!.df)),
  );
  fireEvent.click(screen.getByRole("button", { name: /^Inspect HZ, / }));
  await screen.findByRole("region", { name: "Point lookup for HZ" });
  const hazard = cases.hazard!.result;
  if (hazard.kind !== "hazard") throw new Error("Hazard fixture expected");
  expect(
    (
      await screen.findByRole(
        "table",
        { name: "Curve values" },
        { timeout: budget(15000) },
      )
    ).textContent,
  ).toContain(sixPlaces(native(hazard.points[0]!.sp)));
  // A kind with no lookup keeps the browser's own view.
  fireEvent.click(screen.getByRole("button", { name: /^Inspect PRICE, / }));
  expect(screen.queryByRole("region", { name: /^Point lookup/ })).toBeNull();
});
