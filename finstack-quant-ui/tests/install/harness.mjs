import { access, readFile } from "node:fs/promises";
import path from "node:path";
import ts from "typescript";
import { resolveInstalled } from "../../scripts/check-registry.mjs";

const exists = (file) =>
  access(file).then(
    () => true,
    () => false,
  );

/**
 * Copy each `@/` import of `source` whose owner is outside the item's closure into
 * app/ and point the specifier at the copy. `from` is where `source` lands in app/.
 * The copy never lands on an installed target, so an item's own imports still fail
 * when its closure misses a dependency.
 */
async function localize(repo, item, graph, source, from) {
  const closure = graph.closures.get(item.name),
    extra = {};
  for (const { fileName: specifier } of ts.preProcessFile(source, true, true)
    .importedFiles) {
    if (!specifier.startsWith("@/")) continue;
    const target = resolveInstalled(graph.owners, from, specifier);
    if (!target || closure.has(graph.owners.get(target))) continue;
    const file = graph.byName
      .get(graph.owners.get(target))
      .files.find((file) => file.target === target);
    extra[target] = await readFile(
      path.join(repo, "finstack-quant-ui", file.path),
      "utf8",
    );
    let local = path.posix.relative(
      path.posix.dirname(from),
      specifier.slice(2),
    );
    if (!local.startsWith(".")) local = `./${local}`;
    source = source.replaceAll(`"${specifier}"`, `"${local}"`);
  }
  return { source, extra };
}

/**
 * Build the installed page body for one visual item: its fixture under
 * tests/install/fixture/<name>.tsx when one exists, otherwise its gallery example.
 * Unrelated gallery imports never enter its install closure.
 */
export async function visualHarness(repo, item, graph) {
  const fixtures = path.join(repo, "finstack-quant-ui/tests/install/fixture");
  const fixture =
    item.name === "pricing-workbench" &&
    process.env.REGISTRY_INSTALL_PUBLISHING === "1"
      ? path.join(fixtures, "publishing.tsx")
      : path.join(fixtures, `${item.name}.tsx`);
  if (await exists(fixture))
    return localize(
      repo,
      item,
      graph,
      await readFile(fixture, "utf8"),
      "harness.tsx",
    );
  const examples = path.join(
    repo,
    "docs-site/src/components/registry-gallery/examples",
  );
  const from = `examples/${item.name}.tsx`;
  const example = await localize(
    repo,
    item,
    graph,
    await readFile(path.join(examples, `${item.name}.tsx`), "utf8"),
    from,
  );
  const extra = {
    ...example.extra,
    [from]: example.source,
    "examples/props.ts": await readFile(
      path.join(examples, "props.ts"),
      "utf8",
    ),
  };
  // An example that forwards a `presentation` prop wraps a chart: probe it through those public props.
  if (!/\bpresentation\?:/.test(example.source))
    return {
      source: `"use client";import {Example} from "./examples/${item.name}";export function InstalledItem(){return <Example/>;}`,
      extra,
    };
  const ref = example.source.match(
    /presentation\?:\s*Pick<[\s\S]*?"(figureRefs?)"/,
  )?.[1];
  if (!ref)
    throw Error(
      `${item.name} example declares presentation without figureRef or figureRefs`,
    );
  extra["wrapper-probe.tsx"] = await readFile(
    path.join(fixtures, "wrapper-probe.tsx"),
    "utf8",
  );
  return {
    source: `"use client";
import {Example} from "./examples/${item.name}";
import {captureFigure,captureFigures,recordActivation,TooltipAction} from "./wrapper-probe";
export function InstalledItem(){return <Example presentation={{
  ${ref === "figureRef" ? "figureRef: captureFigure" : "figureRefs: captureFigures"}, title: "Independent wrapper figure",
  subtitle: "Supplied presentation through public wrapper props",
  onSelect: recordActivation,
  renderTooltipBody: ({defaultBody,pinned,dismiss}) => <>{defaultBody}{pinned&&<TooltipAction dismiss={dismiss}/>}</>,
}}/>;}`,
    extra,
    wrapper: true,
  };
}
