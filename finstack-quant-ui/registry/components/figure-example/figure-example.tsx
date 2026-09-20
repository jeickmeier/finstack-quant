"use client";
import { figureExample } from "./figure-data";
import { Button } from "@/components/ui/button";
import { useRef, useState } from "react";
import {
  FinstackChart,
  type FigureHandle,
} from "../../primitives/finstack-chart/finstack-chart";
/** Installable standalone example with an explicit 6×4-inch, 300-PPI publication export. */
export function FigureExample() {
  const figure = useRef<FigureHandle>(null);
  const [error, setError] = useState<string | null>(null);
  async function download(format: "svg" | "png") {
    try {
      const options = {
        width: 900,
        height: 600,
        scale: 2,
        theme: "light" as const,
      };
      const blob = await (format === "svg"
        ? figure.current!.exportSvg(options)
        : figure.current!.exportPng(options));
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `supplied-observations.${format}`;
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 0);
      setError(null);
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    }
  }
  return (
    <section className="space-y-2 font-sans text-foreground">
      <div className="flex gap-2 print:hidden">
        <Button
          variant="outline"
          size="sm"

          onClick={() => void download("svg")}
        >
          Download SVG
        </Button>
        <Button
          variant="outline"
          size="sm"

          onClick={() => void download("png")}
        >
          Download PNG · 1800 × 1200
        </Button>
      </div>
      {error && (
        <p role="alert" className="text-error">
          {error}
        </p>
      )}
      <FinstackChart {...figureExample} ref={figure} height={520} />
    </section>
  );
}
