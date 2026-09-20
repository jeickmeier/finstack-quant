// @vitest-environment jsdom
import { createRequire } from "node:module";
import { it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import {
  surfaceNodes,
  surfaceSlices,
  surfaceLabels,
} from "../registry/components/vol-surface-chart/stored";
import { VolSurfaceChart } from "../registry/components/vol-surface-chart/vol-surface-chart";
import { FxDeltaQuotes } from "../registry/components/fx-delta-quotes/fx-delta-quotes";
import { useLinkedSelection } from "../registry/hooks/use-linked-selection/use-linked-selection";
import { storedSurface, tenorSurface, fxQuotes } from "./surfaces/fixtures";
import bond from "../src/fixtures/results/bond.json";
// Native marks/export/keyboard are exercised by the installed browser fixture.
vi.mock(
  "../registry/primitives/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: (props: { ariaLabel: string }) => (
      <div role="img" aria-label={props.ariaLabel} />
    ),
  }),
);
afterEach(cleanup);
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
it.each([storedSurface, tenorSurface])(
  "retains every native-validated stored node and exact slices: $id",
  (surface) => {
    const input = {
      ...JSON.parse(bond.request.marketJson),
      surfaces: [surface],
      fx_delta_vol_surfaces: [fxQuotes],
    };
    const market = new native.Market(JSON.stringify(input));
    try {
      expect(JSON.parse(market.toJson()).surfaces[0]).toEqual(surface);
    } finally {
      market.free();
    }
    const nodes = surfaceNodes(surface);
    nodes.forEach((node, i) => {
      expect(node.surface).toBe(surface);
      expect([node.expiry, node.secondary, node.value]).toEqual([
        surface.expiries[Math.floor(i / 3)],
        surface.strikes[i % 3],
        surface.vols_row_major[i],
      ]);
    });
    const slices = surfaceSlices(nodes, nodes[4]!.key);
    expect(slices.row).toEqual(nodes.slice(3, 6));
    expect(slices.column).toEqual([nodes[1], nodes[4], nodes[7]]);
    expect(surfaceSlices(nodes, "removed")).toEqual({
      selected: undefined,
      row: [],
      column: [],
    });
    expect(surfaceLabels(surface).secondary).toBe(
      surface.secondary_axis === "strike" ? "Strike" : "Tenor (years)",
    );
    expect(surfaceLabels(surface).value).toContain(
      surface.quote_type === "normal" ? "Normal" : "Black/lognormal",
    );
  },
);
it("shares table/control/external selection and clears removed coordinates without writing parent state", () => {
  const changed = vi.fn();
  function App({ surface = storedSurface }) {
    const link = useLinkedSelection({ onSelectedKeyChange: changed });
    return (
      <>
        <button onClick={() => link.select(surfaceNodes(surface)[4]!.key)}>
          External selection
        </button>
        <output aria-label="key">{link.selectedKey}</output>
        <VolSurfaceChart surface={surface} link={link} colorDomain={[0, 0.3]} />
      </>
    );
  }
  const view = render(<App />);
  fireEvent.click(screen.getByRole("cell", { name: "0.24" }));
  expect(screen.getByLabelText("key").textContent).toBe(
    surfaceNodes(storedSurface)[4]!.key,
  );
  const labels = surfaceLabels(storedSurface);
  expect(
    screen
      .getByRole("combobox", { name: "Stored coordinate" })
      .querySelector("[data-slot=select-value]")!.textContent,
  ).toBe(`${labels.expiry} 1 · ${labels.secondary} 100`);
  expect(
    screen.getByRole("img", { name: "Supplied surface row slice" }),
  ).toBeTruthy();
  fireEvent.click(screen.getByText("Clear selected node"));
  expect(
    screen.queryByRole("img", { name: "Supplied surface row slice" }),
  ).toBeNull();
  fireEvent.click(screen.getByText("External selection"));
  expect(changed).toHaveBeenCalledTimes(3);
  view.rerender(
    <App
      surface={{
        ...storedSurface,
        expiries: [0.5, 2],
        vols_row_major: [
          ...storedSurface.vols_row_major.slice(0, 3),
          ...storedSurface.vols_row_major.slice(6),
        ],
      }}
    />,
  );
  expect(
    screen
      .getByRole("combobox", { name: "Stored coordinate" })
      .querySelector("[data-slot=select-value]")!.textContent,
  ).toBe("Choose a node");
  expect(changed).toHaveBeenCalledTimes(3);
  expect(
    screen.queryByRole("img", { name: "Supplied surface row slice" }),
  ).toBeNull();
});
it("preserves complete raw FX quote arrays without inventing missing wings", () => {
  render(<FxDeltaQuotes surface={fxQuotes} />);
  expect(screen.getByRole("cell", { name: "0.012" })).toBeTruthy();
  expect(screen.getAllByRole("cell", { name: "Not supplied" })).toHaveLength(4);
  const raw = screen.getByLabelText(
    "Complete FX quote state",
  ) as HTMLTextAreaElement;
  expect(JSON.parse(raw.querySelector("pre")!.textContent!)).toEqual(fxQuotes);
});
it("rejects malformed row-major dimensions", () =>
  expect(() =>
    surfaceNodes({ ...storedSurface, vols_row_major: [0.2] }),
  ).toThrow(/dimensions/));
