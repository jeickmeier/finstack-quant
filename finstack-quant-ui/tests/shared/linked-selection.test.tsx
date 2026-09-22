// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { useLinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
import { chartSelection } from "@/components/finstack/shared/chart/finstack-chart/selection";
import { createChartRuntime, defineChart, dot } from "@tanstack/charts";
import { whenSelected } from "@tanstack/charts/selection";
import { scaleLinear } from "@tanstack/charts/scales/linear";
afterEach(cleanup);
const rows = [
  { id: "a", x: 1, y: 3 },
  { id: "b", x: 2, y: 4 },
];
const marks = (rows: { id: string; x: number; y: number }[]) =>
  dot(rows, { x: "x", y: "y", key: "id", id: "points" });
function scene(
  rows: { id: string; x: number; y: number }[],
  selectedKey: string | null,
  select = vi.fn(),
) {
  const selection = chartSelection<
    { id: string; x: number; y: number },
    number,
    number
  >({ selectedKey, select }, (row) => row.id);
  const definition = defineChart({
    marks: [
      marks(rows),
      whenSelected(
        dot(rows, { x: "x", y: "y", key: "id", id: "highlight", r: 9 }),
        selection,
      ),
    ],
    scales: { x: { scale: scaleLinear }, y: { scale: scaleLinear } },
    selection,
  });
  return {
    scene: createChartRuntime<
      { id: string; x: number; y: number },
      number,
      number
    >().render(definition, { width: 500, height: 300 }),
    selection,
  };
}
it("owns local accepted keys, ignores equal proposals, clears and isolates parents", () => {
  const changed = vi.fn();
  const first = renderHook(() =>
    useLinkedSelection({
      defaultSelectedKey: "a",
      onSelectedKeyChange: changed,
    }),
  );
  const second = renderHook(() => useLinkedSelection());
  act(() => first.result.current.select("a"));
  expect(changed).not.toHaveBeenCalled();
  act(() => first.result.current.select("b"));
  expect(first.result.current.selectedKey).toBe("b");
  expect(second.result.current.selectedKey).toBeNull();
  act(() => first.result.current.clear());
  act(() => first.result.current.clear());
  expect(changed.mock.calls).toEqual([["b"], [null]]);
});
it("derives from controlled acceptance, rejection, programmatic changes and clears", () => {
  const changed = vi.fn();
  const hook = renderHook(
    ({ key }: { key: string | null }) =>
      useLinkedSelection({ selectedKey: key, onSelectedKeyChange: changed }),
    { initialProps: { key: "a" as string | null } },
  );
  act(() => hook.result.current.select("b"));
  expect(hook.result.current.selectedKey).toBe("a");
  hook.rerender({ key: "b" });
  expect(hook.result.current.selectedKey).toBe("b");
  act(() => hook.result.current.select("b"));
  hook.rerender({ key: "c" });
  act(() => hook.result.current.clear());
  expect(hook.result.current.selectedKey).toBe("c");
  hook.rerender({ key: null });
  expect(changed.mock.calls).toEqual([["b"], [null]]);
});
it("native changes retain original data, while decorative matches add no hit targets", () => {
  const select = vi.fn();
  const original = scene(rows, "b", select);
  expect(original.scene.points).toHaveLength(rows.length);
  const point = original.scene.points.find((p) => p.datum.id === "b")!;
  expect(point.datum).toBe(rows[1]);
  expect(original.selection.matches(point)).toBe(true);
  original.selection.change(point, "keyboard");
  original.selection.change(null, "pointer");
  expect(select.mock.calls).toEqual([["b"], [null]]);
  const reversed = scene([...rows].reverse(), "b");
  expect(
    reversed.scene.points
      .filter(reversed.selection.matches)
      .map((p) => p.datum.id),
  ).toEqual(["b"]);
  const removed = scene(rows.slice(0, 1), "b");
  expect(removed.scene.points.filter(removed.selection.matches)).toEqual([]);
  expect(removed.selection.selected.value).toBe("b");
  const duplicates = scene([...rows, { id: "b", x: 3, y: 5 }], "b");
  expect(duplicates.scene.points).toHaveLength(3);
  expect(
    duplicates.scene.points.filter(duplicates.selection.matches),
  ).toHaveLength(2);
});
