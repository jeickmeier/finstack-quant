// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterEach, expect, it } from "vitest";
import { cleanup, fireEvent, render, within } from "@testing-library/react";
import { CovenantReport } from "@/components/finstack/covenants/components/covenant-report/covenant-report";
import { serializeHost } from "../../src/codec.mjs";
import fixture from "../valuations/details/cases.json";

const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);

function showOriginal(scope: HTMLElement = document.body) {
  for (const button of within(scope).queryAllByRole("button", {
    name: "Original",
    hidden: true,
  }))
    fireEvent.click(button);
}

afterEach(cleanup);

it("preserves actual native covenant reports without computing verdicts", () => {
  const r = fixture.covenantRequest;
  const reports = native.evaluateEngine(r.engineJson, r.metricsJson, r.asOf);
  expect(serializeHost(reports)).toBe(fixture.covenantReportsJson);
  render(<CovenantReport value={reports} />);
  showOriginal();
  expect(document.querySelector("pre")!.textContent).toBe(
    fixture.covenantReportsJson,
  );
});
