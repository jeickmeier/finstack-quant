import type { ChartValue } from "@tanstack/charts";
import { controlledSignal } from "@tanstack/charts/interaction/signal";
import {
  keyedSelection,
  type KeyedSelectionOptions,
} from "@tanstack/charts/selection";

/** Derive native selection from accepted state each render; no effects or focus writes. */
export function chartSelection<T, X extends ChartValue, Y extends ChartValue>(
  binding: {
    readonly selectedKey: string | null;
    select(key: string | null): void;
  },
  key: KeyedSelectionOptions<T, string, X, Y>["key"],
) {
  return keyedSelection<T, string, X, Y>({
    selected: controlledSignal(binding.selectedKey, (value) =>
      binding.select(value),
    ),
    key,
  });
}
