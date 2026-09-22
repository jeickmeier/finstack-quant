import { readFile } from "node:fs/promises";
import path from "node:path";

const nativeForms = new Set([
  "schema-form",
  "instrument-form",
  "market-context-form",
  "calibration-form",
]);
const wrappers = new Set([
  "curve-chart",
  "calibration-fit-chart",
  "scenario-heatmap",
  "vol-surface-chart",
  "fx-surface-chart",
  "vol-cube-explorer",
]);

/** Copy one explicit example: unrelated gallery imports never enter its install closure. */
export async function visualHarness(repo, item) {
  const fixture = (name) =>
    readFile(
      path.join(repo, "finstack-quant-ui/tests/install/fixture", name),
      "utf8",
    );
  const example = (name) =>
    readFile(
      path.join(
        repo,
        "docs-site/src/components/registry-gallery/examples",
        name,
      ),
      "utf8",
    );
  if (
    item.name === "finstack-chart" ||
    (item.name === "pricing-workbench" &&
      process.env.REGISTRY_INSTALL_PUBLISHING === "1")
  ) {
    return {
      source: await fixture(
        item.name === "finstack-chart" ? "chart.tsx" : "publishing.tsx",
      ),
      extra: {
        "figure-data.ts": await readFile(
          path.join(
            repo,
            "finstack-quant-ui/registry/shared/chart/figure-example/figure-data.ts",
          ),
          "utf8",
        ),
      },
    };
  }
  if (nativeForms.has(item.name)) {
    return {
      source: await fixture(`${item.name}.tsx`),
      extra:
        item.name === "schema-form"
          ? {
              "bond.schema.json": await readFile(
                path.join(
                  repo,
                  "finstack-quant-ui/src/generated/schemas/bond.json",
                ),
                "utf8",
              ),
            }
          : {},
    };
  }
  const extra = {
    [`examples/${item.name}.tsx`]: await example(`${item.name}.tsx`),
    "examples/props.ts": await example("props.ts"),
  };
  if (["valuation-details", "monte-carlo-diagnostics"].includes(item.name)) {
    // The example restores captured wire fixtures; the display item needs no result codec/schema.
    extra[`examples/${item.name}.tsx`] = extra[
      `examples/${item.name}.tsx`
    ].replace(
      "@/lib/finstack/generated/schemas/valuation_result.json",
      "../valuation-result.schema.json",
    );
    extra["valuation-result.schema.json"] = await readFile(
      path.join(
        repo,
        "finstack-quant-ui/src/generated/schemas/valuation_result.json",
      ),
      "utf8",
    );
  }
  if (!wrappers.has(item.name))
    return {
      source: `"use client";import {Example} from "./examples/${item.name}";export function InstalledItem(){return <Example/>;}`,
      extra,
    };
  extra["wrapper-probe.tsx"] = await fixture("wrapper-probe.tsx");
  const refs =
    item.name === "scenario-heatmap"
      ? "figureRef: captureFigure"
      : "figureRefs: captureFigures";
  return {
    source: `"use client";
import {Example} from "./examples/${item.name}";
import {captureFigure,captureFigures,recordActivation,TooltipAction} from "./wrapper-probe";
export function InstalledItem(){return <Example presentation={{
  ${refs}, title: "Independent wrapper figure",
  subtitle: "Supplied presentation through public wrapper props",
  onSelect: recordActivation,
  renderTooltipBody: ({defaultBody,pinned,dismiss}) => <>{defaultBody}{pinned&&<TooltipAction dismiss={dismiss}/>}</>,
}}/>;}`,
    extra,
  };
}
