import { createChartRuntime, type ChartValue } from "@tanstack/charts";
import { serializeChartSvg, renderChartImage } from "@tanstack/charts/export";
import { composeFigure, type FigureSpec } from "./figure";
import { readPresentation } from "./presentation";
export interface FigureExportOptions {
  width: number;
  height: number;
  /** Raster multiplier; use physical inches × PPI / layout pixels for a print target. */
  scale?: number;
  theme?: "light" | "dark";
  background?: string;
}
const fontData = new Map<string, Promise<string>>();
async function dataUrl(url: string) {
  let pending = fontData.get(url);
  if (!pending) {
    pending = fetch(url).then(async (response) => {
      if (!response.ok)
        throw new Error(`Figure font could not be loaded: ${response.status}`);
      const blob = await response.blob();
      return new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result));
        reader.onerror = () => reject(reader.error);
        reader.readAsDataURL(blob);
      });
    });
    fontData.set(url, pending);
    void pending.catch(() => fontData.delete(url));
  }
  return pending;
}
/** Embed the loaded theme's readable FontFace rules so raster images use the measured fonts. */
async function embeddedFonts(document: Document, family: string) {
  const primary = family.split(",")[0].replaceAll(/["']/g, "").trim();
  const faces: { css: string; base: string }[] = [];
  function visit(rules: CSSRuleList, base: string) {
    for (const rule of rules) {
      if (
        rule instanceof CSSFontFaceRule &&
        rule.style.fontFamily.replaceAll(/["']/g, "").trim() === primary
      )
        faces.push({ css: rule.cssText, base });
      if (rule instanceof CSSImportRule && rule.styleSheet)
        visit(rule.styleSheet.cssRules, rule.href);
      else if ("cssRules" in rule)
        visit((rule as CSSGroupingRule).cssRules, base);
    }
  }
  for (const sheet of document.styleSheets) {
    try {
      visit(sheet.cssRules, sheet.href ?? document.baseURI);
    } catch (error) {
      if (!(error instanceof DOMException && error.name === "SecurityError"))
        throw error;
    }
  }
  if (!faces.length)
    throw new Error(
      `Expose a same-origin font stylesheet for figure export: ${primary}`,
    );
  return (
    await Promise.all(
      faces.map(async ({ css, base }) => {
        for (const match of [...css.matchAll(/url\(["']?([^"')]+)["']?\)/g)])
          css = css.replace(
            match[0],
            `url("${await dataUrl(new URL(match[1], base).href)}")`,
          );
        return css;
      }),
    )
  ).join("\n");
}
/** Relayout and export the complete figure; native export owns SVG styles and PNG rasterization. */
export async function exportFigure<
  T,
  X extends ChartValue,
  Y extends ChartValue,
>(
  source: HTMLElement,
  props: FigureSpec<T, X, Y>,
  options: FigureExportOptions,
  format: "svg" | "png",
): Promise<Blob> {
  if (
    ![options.width, options.height, options.scale ?? 2].every(
      (value) => Number.isFinite(value) && value > 0,
    )
  )
    throw new Error(
      "Export dimensions and raster scale must be finite and positive",
    );
  const document = source.ownerDocument;
  await document.fonts.ready;
  const host = document.createElement("div");
  host.className = "font-sans text-sm";
  host.style.cssText =
    "position:fixed;left:-100000px;top:0;pointer-events:none;";
  host.style.fontFamily = getComputedStyle(source).fontFamily;
  host.dataset.theme =
    options.theme ??
    source.closest<HTMLElement>("[data-theme]")?.dataset.theme ??
    "light";
  source.append(host);
  const runtime = createChartRuntime<T, X, Y>();
  try {
    const presentation = readPresentation(host);
    if (options.background) presentation.theme.background = options.background;
    const figure = composeFigure(props, presentation);
    const scene = runtime.render(
      figure.definition,
      { width: options.width, height: options.height },
      {
        measureText: presentation.measureText,
        typography: { fontFamily: presentation.fontFamily },
      },
    );
    host.innerHTML = figure.renderSvg(scene, {
      ariaLabel: props.ariaLabel,
      ariaDescription: props.ariaDescription,
      idPrefix: "figure-export",
    });
    const svg = host.querySelector("svg")!;
    const style = document.createElementNS(
      "http://www.w3.org/2000/svg",
      "style",
    );
    style.textContent = await embeddedFonts(document, presentation.fontFamily);
    svg.prepend(style);
    if (format === "svg")
      return new Blob(
        [
          serializeChartSvg(svg, {
            width: options.width,
            height: options.height,
          }),
        ],
        { type: "image/svg+xml" },
      );
    return await renderChartImage(svg, {
      width: options.width,
      height: options.height,
      scale: options.scale ?? 2,
      type: "image/png",
    });
  } finally {
    runtime.destroy();
    host.remove();
  }
}
