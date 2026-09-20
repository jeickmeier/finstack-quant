import type { Ref } from "react";
import type { FigureHandle } from "@/components/finstack/primitives/finstack-chart/finstack-chart";
let figure: FigureHandle | null = null;
export const captureFigure: Ref<FigureHandle> = (value) => {
  figure = value;
};
// Export the primary figure, excluding surface row/column slices with their own titles.
export const captureFigures = new Proxy<Record<string, Ref<FigureHandle>>>(
  {},
  {
    get: (_target, key) =>
      key === "row" || key === "column" ? undefined : captureFigure,
  },
);
export const wrapperProbe = { activations: 0, detail: false };
export function recordActivation() {
  wrapperProbe.activations++;
}
export function TooltipAction({ dismiss }: { dismiss: () => void }) {
  return (
    <button
      onClick={() => {
        wrapperProbe.detail = true;
        dismiss();
      }}
    >
      Wrapper original detail
    </button>
  );
}
if (typeof window !== "undefined")
  Object.assign(window, {
    wrapperProbe,
    exportInstalledWrapper: async () => {
      if (!figure)
        throw Error("Wrapper did not expose its public figure handle");
      const options = {
        width: 900,
        height: 600,
        scale: 2,
        theme: "light" as const,
      };
      return {
        svg: await (await figure.exportSvg(options)).text(),
        png: Array.from(
          new Uint8Array(await (await figure.exportPng(options)).arrayBuffer()),
        ),
      };
    },
  });
