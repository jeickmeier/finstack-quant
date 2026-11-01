// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  bondWorkoutLadderOptions,
  useBondWorkoutLadder,
  type BondWorkoutLadderRequest,
} from "@/hooks/valuations/use-bond-workout-ladder/use-bond-workout-ladder";
import {
  YieldWorkoutLadder,
  type YieldWorkoutLadderProps,
} from "@/components/finstack/valuations/components/yield-workout-ladder/yield-workout-ladder";
import { unwrap } from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { startWorker } from "../../shared/worker/harness.mjs";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";

const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  mock.createClient.mockReturnValue({
    call: vi.fn(async (method: string, request: unknown) => {
      if (method === "initialize")
        return unwrap(await harness.proxy.initialize());
      if (method === "bondWorkoutLadder")
        return unwrap(
          await harness.proxy.bondWorkoutLadder(
            request as BondWorkoutLadderRequest,
          ),
        );
      throw new Error(`Unexpected worker call ${method}`);
    }),
    close: vi.fn(),
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const { cases, unquoted } = fixture;
type Ladder = NonNullable<YieldWorkoutLadderProps["ladder"]>;
const metadata = fixture.metadata as YieldWorkoutLadderProps["metadata"];
const ladderOf = (entry: { ladder: unknown }) => entry.ladder as Ladder;
const bodyRows = () => screen.getAllByRole("row").slice(1);

// Values are regenerated and compared by `mise run ui-fixtures --check`.
it("pins the JSON inputs the fixture was built from", () => {
  const repository = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../../../..",
  );
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(repository, source.path)))
        .digest("hex"),
      `${source.path} changed since tests/valuations/workout-ladder/generate.mjs ran; rerun mise run ui-fixtures`,
    ).toBe(source.sha256);
});
it.each(Object.entries(cases))(
  "the worker returns the native ladder of %s",
  async (_name, entry) => {
    expect(
      unwrap(await harness.proxy.bondWorkoutLadder(entry.request)),
    ).toEqual(entry.ladder);
  },
  budget(30000),
);
it.each(Object.entries(cases))(
  "the row Rust marks in %s holds the ytw metric's yield",
  (_name, entry) => {
    const marked = ladderOf(entry).rows.filter((row) => row.is_worst);
    expect(marked).toHaveLength(1);
    // A null metric is negative infinity through JSON: no finite yield either way.
    expect(marked[0].yield_rate ?? null).toBe(entry.ytw);
  },
);
it(
  "the worker returns the native error of a callable bond with no quote",
  async () => {
    const result = await harness.proxy.bondWorkoutLadder(unquoted.request);
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error.message).toBe(unquoted.error!.message);
    expect(result.error.kind).toBe(unquoted.error!.kind);
  },
  budget(30000),
);
it("marks only the row Rust marks, whatever the yields", () => {
  const native = ladderOf(cases.premium);
  // The lowest yield moved off the marked row: the mark must not follow it.
  const swapped: Ladder = {
    ...native,
    rows: native.rows.map((row, index) => ({
      ...row,
      is_worst: index === 2,
    })),
  };
  render(<YieldWorkoutLadder ladder={swapped} metadata={metadata} />);
  const marks = bodyRows().map(
    (row) => within(row).queryByText("Worst") !== null,
  );
  expect(marks).toEqual([false, false, true, false]);
});
it("draws each exit in Rust's order with its redemption, yield and spread", () => {
  const ladder = ladderOf(cases.premium);
  render(<YieldWorkoutLadder ladder={ladder} metadata={metadata} />);
  expect(
    screen.getByRole("columnheader", { name: "Yield (decimal)" }),
  ).toBeTruthy();
  const rows = bodyRows();
  expect(rows).toHaveLength(ladder.rows.length);
  ladder.rows.forEach((row, index) => {
    const cells = within(rows[index]);
    expect(cells.getByText(row.date)).toBeTruthy();
    expect(cells.getByTitle(String(row.yield_rate))).toBeTruthy();
    expect(cells.getByTitle(String(row.i_spread))).toBeTruthy();
    expect(rows[index].textContent).toContain(String(row.redemption_price_pct));
  });
  expect(screen.getByText(ladder.quote_date)).toBeTruthy();
  expect(within(rows[0]).getByText("Worst")).toBeTruthy();
});
it("shows a null yield as no yield, never zero, and a make-whole redemption as such", () => {
  const { unmount } = render(
    <YieldWorkoutLadder ladder={ladderOf(cases.quoteDateCall)} />,
  );
  const first = within(bodyRows()[0]);
  expect(first.getAllByText("No yield")).toHaveLength(2);
  expect(first.getByText("Worst")).toBeTruthy();
  unmount();
  render(<YieldWorkoutLadder ladder={ladderOf(cases.makeWhole)} />);
  expect(bodyRows()[0].textContent).toContain("make-whole");
  expect(bodyRows()[1].textContent).not.toContain("make-whole");
});
it("a bond with no exercise rights shows one maturity row", () => {
  render(<YieldWorkoutLadder ladder={ladderOf(cases.bullet)} />);
  expect(bodyRows()).toHaveLength(1);
  expect(within(bodyRows()[0]).getByText("Maturity")).toBeTruthy();
});
it("states loading, an error and no result", () => {
  const { rerender } = render(<YieldWorkoutLadder ladder={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Solving exits…");
  rerender(<YieldWorkoutLadder ladder={null} error="boom" />);
  expect(screen.getByRole("alert").textContent).toBe("boom");
  rerender(<YieldWorkoutLadder ladder={null} />);
  expect(screen.getByRole("status").textContent).toBe("No workout ladder");
});
it("the hook is disabled without a request and keys on the whole request", () => {
  const client = {} as FinstackClient;
  expect(bondWorkoutLadderOptions(client, 1, null).enabled).toBe(false);
  const options = bondWorkoutLadderOptions(client, 1, cases.premium.request);
  expect(options.enabled).toBe(true);
  expect(options.queryKey).toEqual([
    "finstack",
    1,
    "bondWorkoutLadder",
    cases.premium.request,
  ]);
});
function Live() {
  const ladder = useBondWorkoutLadder(cases.discount.request);
  return (
    <YieldWorkoutLadder
      ladder={(ladder.data as Ladder | undefined) ?? null}
      error={ladder.error?.message}
    />
  );
}
it(
  "the hook reaches the worker and draws the native result",
  async () => {
    render(
      <FinstackQueryProvider>
        <Live />
      </FinstackQueryProvider>,
    );
    await waitFor(() => expect(screen.getByText("Worst")).toBeTruthy(), {
      timeout: budget(30000),
    });
    expect(within(bodyRows()[3]).getByText("Worst")).toBeTruthy();
  },
  budget(40000),
);
