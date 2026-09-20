"use client";
import { useEffect, useRef, useState } from "react";
import { PricingWorkbench } from "@/components/finstack/blocks/pricing-workbench/pricing-workbench";
import {
  FinstackChart,
  type FigureHandle,
} from "@/components/finstack/primitives/finstack-chart/finstack-chart";
import data from "./data.json";
import { figureExample } from "./figure-data";
export function InstalledItem() {
  const [kind, setKind] = useState("bond");
  const figure = useRef<FigureHandle>(null);
  const request =
    kind === "bond"
      ? data.bond.request
      : data.cashflows.cases.find((entry) => entry.type === "xccy_swap")!
          .request;
  useEffect(() => {
    Object.assign(window, {
      exportPublication: async () => {
        const options = {
          width: 900,
          height: 600,
          scale: 2,
          theme: "light" as const,
        };
        return {
          svg: await (await figure.current!.exportSvg(options)).text(),
          png: Array.from(
            new Uint8Array(
              await (await figure.current!.exportPng(options)).arrayBuffer(),
            ),
          ),
        };
      },
    });
  }, []);
  return (
    <>
      <nav className="print:hidden">
        <button onClick={() => setKind("bond")}>Bond report</button>
        <button onClick={() => setKind("xccy_swap")}>
          Mixed currency report
        </button>
      </nav>
      <PricingWorkbench key={kind} defaultRequest={request} />
      <div className="print:hidden">
        <FinstackChart
          {...figureExample}
          ref={figure}
          width={880}
          height={560}
        />
      </div>
    </>
  );
}
