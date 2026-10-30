// @vitest-environment jsdom
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
import userEvent from "@testing-library/user-event";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  migrationMatrixOptions,
  useMigrationMatrix,
  type MigrationMatrix,
  type MigrationMatrixRequest,
} from "@/hooks/models/use-migration-matrix/use-migration-matrix";
import {
  migrationProjectionOptions,
  useMigrationProjection,
  type MigrationProjection,
  type MigrationProjectionRequest,
} from "@/hooks/models/use-migration-projection/use-migration-projection";
import {
  TransitionMatrixHeatmap,
  type MigrationMatrixData,
  type MigrationProjectionData,
} from "@/components/finstack/models/components/transition-matrix-heatmap/transition-matrix-heatmap";
import { unwrap } from "@/workers/finstack-contract";
import { startWorker } from "../../shared/worker/harness.mjs";
import { budget } from "../../shared/budget";
import fixture from "./migration.json";
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
// Figures are drawn by the shared chart in the browser gates; here each one
// is a labelled placeholder (the heatmap preset still validates its domain).
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
type Resources = Record<
  | "matrixConstructed"
  | "matrixFreed"
  | "generatorConstructed"
  | "generatorFreed"
  | "scaleConstructed"
  | "scaleFreed",
  number
>;
let harness: Awaited<ReturnType<typeof startWorker>>;
const resources = () =>
  (harness.proxy as unknown as { resources(): Promise<Resources> }).resources();
/** Handles constructed and freed by one call. */
async function counted<T>(call: () => Promise<T>) {
  const before = await resources();
  const value = await call();
  const after = await resources();
  const delta = Object.fromEntries(
    Object.entries(after).map(([key, count]) => [
      key,
      count - before[key as keyof Resources],
    ]),
  ) as Resources;
  return { value, delta };
}
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
      if (method === "migrationMatrix")
        return unwrap(
          await harness.proxy.migrationMatrix(
            request as MigrationMatrixRequest,
          ),
        );
      if (method === "migrationProjection")
        return unwrap(
          await harness.proxy.migrationProjection(
            request as MigrationProjectionRequest,
          ),
        );
      throw new Error(`Unexpected worker call ${method}`);
    }),
    close() {},
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const request = fixture.request as MigrationProjectionRequest;
const matrix = fixture.matrix as MigrationMatrix;
const projection = fixture.projection as MigrationProjection;
// The fixture is regenerated and compared by `mise run ui-fixtures --check`
// (tests/models/credit/generate.mjs).
it(
  "returns the native matrix through the real worker and frees the matrix and its scale",
  async () => {
    const { value, delta } = await counted(
      async () => await harness.proxy.migrationMatrix(request),
    );
    expect(unwrap(value)).toEqual(matrix);
    expect(delta).toMatchObject({
      matrixConstructed: 1,
      matrixFreed: 1,
      scaleConstructed: 1,
      scaleFreed: 1,
      generatorConstructed: 0,
    });
  },
  budget(15000),
);
it(
  "returns the native generator and projections and frees every handle",
  async () => {
    const { value, delta } = await counted(
      async () => await harness.proxy.migrationProjection(request),
    );
    // The fixture went through JSON, which writes the absorbing row's -0 as 0.
    expect(JSON.parse(JSON.stringify(unwrap(value)))).toEqual(projection);
    // The labelled matrix plus one projected matrix per horizon.
    expect(delta).toMatchObject({
      matrixConstructed: 1 + request.horizons.length,
      matrixFreed: 1 + request.horizons.length,
      generatorConstructed: 1,
      generatorFreed: 1,
      scaleConstructed: 1,
      scaleFreed: 1,
    });
  },
  budget(15000),
);
it("pins the illustrative matrix's native values", () => {
  expect(matrix.labels).toEqual(request.labels);
  expect(matrix.defaultState).toBe(7);
  expect(matrix.defaultProbabilities).toEqual(matrix.rows.map((r) => r[7]));
  // Rust's default tolerance accepts the fit; the error is reported, not hidden.
  expect(projection.generator.roundTripError).toBeCloseTo(5.107e-4, 6);
  expect(projection.generator.regularizationL1).toBeGreaterThan(0);
  expect(projection.projections.map((p) => p.horizon)).toEqual(
    request.horizons,
  );
  // A generator's diagonal is negative: the heatmap domain must contain it.
  expect(Math.min(...projection.generator.rows.flat())).toBeLessThan(0);
});
it("returns native errors unchanged with every handle freed", async () => {
  const tight = await counted(
    async () =>
      await harness.proxy.migrationProjection({
        ...request,
        roundTripTol: 1e-9,
      }),
  );
  expect(tight.value.ok).toBe(false);
  if (!tight.value.ok) {
    expect(tight.value.error.kind).toBe("computation");
    expect(tight.value.error.message).toContain("exceeds tolerance");
  }
  expect(tight.delta).toMatchObject({
    matrixConstructed: 1,
    matrixFreed: 1,
    generatorConstructed: 0,
  });
  const horizon = await counted(
    async () =>
      await harness.proxy.migrationProjection({
        ...request,
        horizons: [1, -1],
      }),
  );
  expect(horizon.value.ok).toBe(false);
  if (!horizon.value.ok) expect(horizon.value.error.kind).toBe("validation");
  // The labelled matrix and the first projection, both freed.
  expect(horizon.delta).toMatchObject({
    matrixConstructed: 2,
    matrixFreed: 2,
    generatorConstructed: 1,
    generatorFreed: 1,
  });
  const rows = request.rows.map((row) => [...row]);
  rows[0] = [0.9, 0.2, 0, 0, 0, 0, 0, 0];
  const invalid = await counted(
    async () => await harness.proxy.migrationMatrix({ ...request, rows }),
  );
  expect(invalid.value.ok).toBe(false);
  if (!invalid.value.ok) {
    expect(invalid.value.error.kind).toBe("validation");
    expect(invalid.value.error.message).toContain("row 0 sums to");
  }
  expect(invalid.delta.matrixConstructed).toBe(0);
});
it("keys both queries on the worker session and the complete snapshotted request", () => {
  const options = migrationMatrixOptions(null, 4, request);
  expect(options.queryKey).toEqual(["finstack", 4, "migrationMatrix", request]);
  expect(options.queryKey[3]).not.toBe(request);
  expect(migrationMatrixOptions(null, 4, null).enabled).toBe(false);
  const projected = migrationProjectionOptions(null, 4, request);
  expect(projected.queryKey).toEqual([
    "finstack",
    4,
    "migrationProjection",
    request,
  ]);
  // An omitted tolerance stays omitted in the key.
  expect(projected.queryKey[3]).not.toHaveProperty("roundTripTol");
  expect(migrationProjectionOptions(null, 4, null).enabled).toBe(false);
});
const data = {
  matrix: matrix as MigrationMatrixData,
  projection: projection as MigrationProjectionData,
};
it("draws the returned matrix, generator and projections, then switches to tables", async () => {
  render(
    <TransitionMatrixHeatmap
      matrix={data.matrix}
      projection={data.projection}
    />,
  );
  for (const label of [
    "Transition probability heatmap",
    "Generator intensity heatmap",
    "Cumulative default probability by horizon",
  ])
    expect(screen.getByRole("img", { name: label })).toBeTruthy();
  expect(screen.getByText(/default state D/)).toBeTruthy();
  // Every returned digit by default.
  expect(screen.getByLabelText("Generator fit").textContent).toContain(
    String(projection.generator.roundTripError),
  );
  await userEvent.click(screen.getByRole("radio", { name: "Table" }));
  const transition = within(
    screen.getByRole("region", { name: "Transition probabilities over 1y" }),
  ).getByRole("table");
  expect(within(transition).getAllByRole("row")).toHaveLength(
    1 + matrix.labels.length,
  );
  expect(within(transition).getByText(String(matrix.rows[6]![7]))).toBeTruthy();
  const cumulative = within(
    screen.getByRole("region", { name: "Cumulative default probability" }),
  ).getByRole("table");
  expect(
    within(cumulative)
      .getAllByRole("columnheader")
      .map((h) => h.textContent),
  ).toEqual(["State", ...request.horizons.map((t) => `${t}y`)]);
  expect(
    within(cumulative).getByText(
      String(projection.projections[3]!.defaultProbabilities![3]),
    ),
  ).toBeTruthy();
});
it("shows a projection error inline while the matrix stays visible", () => {
  render(
    <TransitionMatrixHeatmap
      matrix={data.matrix}
      projectionError="round-trip error exceeds tolerance"
    />,
  );
  expect(
    screen.getByRole("img", { name: "Transition probability heatmap" }),
  ).toBeTruthy();
  expect(screen.getByRole("alert").textContent).toBe(
    "round-trip error exceeds tolerance",
  );
  expect(
    screen.queryByRole("region", { name: "Cumulative default probability" }),
  ).toBeNull();
});
it("shows the empty, loading and native-error states", () => {
  const { rerender } = render(<TransitionMatrixHeatmap matrix={null} />);
  expect(screen.getByRole("status").textContent).toBe("No transition matrix");
  rerender(<TransitionMatrixHeatmap matrix={null} loading />);
  expect(screen.getByRole("status").textContent).toBe(
    "Building transition matrix…",
  );
  rerender(<TransitionMatrixHeatmap matrix={null} error="native failure" />);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
  rerender(<TransitionMatrixHeatmap matrix={data.matrix} projectionLoading />);
  expect(screen.getByText("Projecting from the generator…")).toBeTruthy();
});
function Connected({ request }: { request: MigrationProjectionRequest }) {
  const result = useMigrationMatrix(request);
  const projected = useMigrationProjection(request);
  return (
    <TransitionMatrixHeatmap
      matrix={result.data ?? null}
      loading={result.isFetching}
      error={result.error?.message}
      projection={projected.data ?? null}
      projectionLoading={projected.isFetching}
      projectionError={projected.error?.message}
    />
  );
}
it("renders the worker results through the hooks, then the native projection error", async () => {
  const { unmount } = render(
    <FinstackQueryProvider>
      <Connected request={request} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () =>
      expect(
        screen.getByRole("img", {
          name: "Cumulative default probability by horizon",
        }),
      ).toBeTruthy(),
    { timeout: budget(5000) },
  );
  unmount();
  render(
    <FinstackQueryProvider>
      <Connected request={{ ...request, roundTripTol: 1e-9 }} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () =>
      expect(screen.getByRole("alert").textContent).toContain(
        "exceeds tolerance",
      ),
    { timeout: budget(5000) },
  );
  expect(
    screen.getByRole("img", { name: "Transition probability heatmap" }),
  ).toBeTruthy();
});
