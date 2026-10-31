"use client";
import { useState } from "react";
import {
  CurveQuery,
  type CurveQueryData,
} from "@/components/finstack/core/components/curve-query/curve-query";

import data from "./data.json";
// The result is the supplied native fixture: the item installs without a worker.
const { request, result } = data.curveQuery.cases.discount;
export function InstalledItem() {
  const [count, setCount] = useState(0);
  return (
    <>
      <p>Supplied valid fixture; standalone curve query.</p>
      <CurveQuery
        target={{ kind: "discount", id: request.id }}
        defaultPoints={request.points.join(", ")}
        onSubmit={() => setCount((n) => n + 1)}
        result={result as unknown as CurveQueryData}
      />
      <output aria-label="Submission count">{count}</output>
    </>
  );
}
