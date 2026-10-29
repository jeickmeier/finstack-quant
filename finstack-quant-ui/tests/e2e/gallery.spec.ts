import { test, expect, type Page } from "playwright/test";
import { fileURLToPath } from "node:url";
import inventory from "../../../docs-site/src/components/registry-gallery/inventory.json" with { type: "json" };
const axe = fileURLToPath(
  new URL("../../node_modules/axe-core/axe.min.js", import.meta.url),
);
const pageErrors = new WeakMap<Page, string[]>();
test.beforeEach(({ page }) => {
  const errors: string[] = [];
  pageErrors.set(page, errors);
  page.on("pageerror", (error) => errors.push(error.message));
});
test.afterEach(({ page }) => {
  expect(pageErrors.get(page)).toEqual([]);
});
async function open(
  page: Page,
  name: string,
  theme = "light",
  density = "compact",
  extra: Record<string, string> = {},
) {
  await page.goto(
    `/registry-gallery?${new URLSearchParams({ item: name, theme, density, ...extra })}`,
  );
  await page.waitForLoadState("networkidle");
  const item = page.locator(`[data-item="${name}"]`);
  await expect(item).toBeVisible();
  if (await item.locator("[data-worker-state]").count()) {
    await expect(item.locator("[data-worker-state]")).toHaveAttribute(
      "data-worker-state",
      "ready",
    );
    await expect(item.locator("[data-fetching]")).toHaveAttribute(
      "data-fetching",
      "0",
    );
  }
  await expect(
    item.getByRole("status").filter({
      hasText: /Loading|Evaluating|Calibrating|Waiting|queued|Pricing…/,
    }),
  ).toHaveCount(0);
  await expect(item.locator('button[type="submit"]:disabled')).toHaveCount(0);
  if (name === "pricing-workbench")
    await expect(item.getByText("Priced", { exact: true })).toBeVisible();
  if (["explanation-trace", "covenant-report"].includes(name))
    await item.locator("summary").click();
  await page.evaluate(() => document.fonts.ready);
  expect(pageErrors.get(page)).toEqual([]);
  return item;
}
for (const item of inventory.visual)
  for (const theme of ["light", "dark"])
    for (const density of ["compact", "comfortable"]) {
      test(`${item.name} ${theme} ${density}`, async ({ page }) => {
        const preview = await open(page, item.name, theme, density);
        await page.addScriptTag({ path: axe });
        const violations = await page.evaluate(async (name) => {
          const axe = (window as any).axe;
          const report = await axe.run(
            document.querySelector(`[data-item="${name}"]`),
          );
          return report.violations
            .filter((v: any) => ["serious", "critical"].includes(v.impact))
            .map((v: any) => ({
              id: v.id,
              nodes: v.nodes.map((n: any) => ({
                target: n.target,
                message: n.failureSummary,
              })),
            }));
        }, item.name);
        expect(violations).toEqual([]);
        await expect(preview).toHaveScreenshot(
          `${item.name}-${theme}-${density}.png`,
          {
            mask: [
              preview
                .locator('dl[aria-label="Calculation metadata"] > div')
                .filter({ has: page.getByText("Timestamp:", { exact: true }) }),
            ],
          },
        );
      });
    }
for (const width of ["narrow", "wide"])
  test(`publication figure ${width}`, async ({ page }) => {
    const item = await open(page, "finstack-chart", "light", "comfortable", {
      variant: "publication",
      width,
    });
    const visibleText = (await item.locator("svg text").allTextContents())
      .join(" ")
      .replace(/\s+/g, " ");
    for (const text of [
      "Supplied observations with an explicit reference range",
      "Registry illustration",
      "Figure note",
    ])
      expect(visibleText).toContain(text);
    await expect(item).toHaveScreenshot(`publication-${width}.png`);
  });
test("shifted cube", async ({ page }) => {
  const item = await open(page, "vol-cube-explorer", "dark", "compact", {
    variant: "shifted",
  });
  await expect(item).toHaveScreenshot("shifted-cube.png");
});
test("fx strike grid", async ({ page }) => {
  const item = await open(page, "fx-surface-chart", "light", "comfortable");
  // The four item baselines stop at the preview's scroll height, above the grid.
  const grid = item.getByRole("region", {
    name: "EURUSD strike grid",
    exact: true,
  });
  await expect(
    grid.getByRole("region", { name: "EURUSD strike grid evaluated surface" }),
  ).toBeVisible();
  await expect(grid).toHaveScreenshot("fx-strike-grid.png");
});
test("cube node smile", async ({ page }) => {
  const item = await open(page, "vol-cube-slice", "light", "comfortable");
  // The four item baselines stop at the preview's scroll height, above the smile.
  const smile = item.getByRole("region", {
    name: "USD-SWAPTION-NORMAL-VOL node smile",
    exact: true,
  });
  await expect(
    smile.getByRole("table", { name: "Node smile values" }),
  ).toBeVisible();
  await expect(smile).toHaveScreenshot("cube-node-smile.png");
});
test("performance-report monthly returns", async ({ page }) => {
  const item = await open(page, "performance-report", "light", "comfortable");
  // The four item baselines stop at the preview's scroll height, above the heatmap.
  const monthly = item.getByRole("region", {
    name: "Global Macro Composite monthly returns",
    exact: true,
  });
  await expect(
    monthly.getByRole("img", {
      name: "Global Macro Composite monthly returns heatmap",
    }),
  ).toBeVisible();
  await expect(monthly).toHaveScreenshot("performance-report-monthly.png");
});
test("positions-table selected swap", async ({ page }) => {
  const item = await open(page, "positions-table", "light", "comfortable");
  // The four item baselines stop inside the first position's measures.
  await item.getByRole("row", { name: /IRS-1/ }).click();
  const swap = item.getByRole("region", {
    name: "Position IRS-1",
    exact: true,
  });
  await expect(swap.getByText("IRS-5Y-USD-STD").first()).toBeVisible();
  await expect(swap).toHaveScreenshot("positions-table-swap.png");
});
test("portfolio-metrics by position", async ({ page }) => {
  const item = await open(page, "portfolio-metrics", "light", "comfortable");
  // The four item baselines stop inside the aggregated table, above this section.
  const byPosition = item.getByRole("region", {
    name: "Metrics by position",
    exact: true,
  });
  await expect(
    byPosition.getByRole("table", { name: "Metrics of position BOND-1" }),
  ).toBeVisible();
  await expect(byPosition).toHaveScreenshot(
    "portfolio-metrics-by-position.png",
  );
});
test("pnl-attribution-report waterfall", async ({ page }) => {
  const item = await open(
    page,
    "pnl-attribution-report",
    "light",
    "comfortable",
  );
  // The four item baselines stop above the waterfall drawn from Rust's steps.
  const waterfall = item.getByRole("region", {
    name: "Waterfall",
    exact: true,
  });
  await expect(
    waterfall.getByRole("table", { name: "Waterfall steps" }),
  ).toBeVisible();
  await expect(waterfall).toHaveScreenshot(
    "pnl-attribution-report-waterfall.png",
  );
});
test("horizon-return-report attribution", async ({ page }) => {
  const item = await open(
    page,
    "horizon-return-report",
    "light",
    "comfortable",
  );
  // The four item baselines stop above the embedded factor attribution.
  const attribution = item.getByRole("region", {
    name: "Attribution",
    exact: true,
  });
  await expect(
    attribution.getByRole("table", { name: "P&L by factor" }),
  ).toBeVisible();
  await expect(attribution).toHaveScreenshot(
    "horizon-return-report-attribution.png",
  );
});
test("benchmark-report multi-factor regression", async ({ page }) => {
  const item = await open(page, "benchmark-report", "light", "comfortable");
  // The four item baselines stop at the preview's scroll height, above this table.
  const multi = item.getByRole("region", {
    name: "Global Macro Composite multi-factor regression",
    exact: true,
  });
  await expect(multi.getByRole("table")).toBeVisible();
  await expect(multi).toHaveScreenshot("benchmark-report-multi-factor.png");
});
test("merton-calculator model parameters", async ({ page }) => {
  const item = await open(page, "merton-calculator", "light", "comfortable");
  // The four item baselines stop inside the term structure, above this table.
  const model = item.getByRole("region", { name: "Model", exact: true });
  await expect(model.getByRole("table")).toBeVisible();
  await expect(model).toHaveScreenshot("merton-calculator-model.png");
});
test("portfolio-margin-report selected netting set", async ({ page }) => {
  const item = await open(
    page,
    "portfolio-margin-report",
    "light",
    "comfortable",
  );
  // The four item baselines stop inside the first set's sensitivities.
  await item
    .getByRole("row", { name: /bilateral · CSA USD-REGULATORY-CSA/ })
    .click();
  const set = item.getByRole("region", {
    name: "Netting set USD-REGULATORY-CSA · CSA USD-REGULATORY-CSA",
    exact: true,
  });
  await expect(set.getByRole("table", { name: "IR delta" })).toBeVisible();
  await expect(set).toHaveScreenshot("portfolio-margin-report-netting-set.png");
});
test("csa-editor loads a Rust preset and its initial margin terms", async ({
  page,
}) => {
  const item = await open(page, "csa-editor", "light", "comfortable");
  await item.getByRole("button", { name: "EUR regulatory" }).click();
  await expect(item.getByLabel("Id", { exact: true })).toHaveValue(
    "EUR-REGULATORY-CSA",
  );
  // The four item baselines stop inside the eligible collateral, above these terms.
  const im = item.getByRole("group", { name: "Im params" });
  await expect(im).toBeVisible();
  await expect(im).toHaveScreenshot("csa-editor-im-params.png");
});
test("credit-assessment-report P&L summary", async ({ page }) => {
  const item = await open(
    page,
    "credit-assessment-report",
    "light",
    "comfortable",
  );
  // The four item baselines stop inside the native P&L summary table.
  const summary = item.getByRole("region", {
    name: "P&L summary",
    exact: true,
  });
  await expect(summary.getByRole("table")).toBeVisible();
  await expect(summary).toHaveScreenshot(
    "credit-assessment-report-pl-summary.png",
  );
});
test("dcf-assumptions-form options", async ({ page }) => {
  const item = await open(page, "dcf-assumptions-form", "light", "comfortable");
  // The four item baselines show the options disclosure collapsed.
  await item.getByText("DCF options", { exact: true }).click();
  const options = item.locator("details").filter({ hasText: "DCF options" });
  await expect(
    options.getByRole("button", { name: "Add exit multiple bump" }),
  ).toBeVisible();
  await options
    .getByRole("button", { name: "Add exit multiple metric node" })
    .click();
  await expect(
    options.getByRole("combobox", { name: "Exit multiple metric node" }),
  ).toBeVisible();
  await expect(options).toHaveScreenshot("dcf-assumptions-form-options.png");
});
test("lbo-report returns and checks", async ({ page }) => {
  const item = await open(page, "lbo-report", "light", "comfortable");
  // The four item baselines stop inside the entry and exit list, above the checks.
  const returns = item.getByRole("region", {
    name: "Entry and exit",
    exact: true,
  });
  await expect(returns.getByText("MOIC")).toBeVisible();
  await expect(returns).toHaveScreenshot("lbo-report-entry-exit.png");
  const checks = item.getByRole("region", { name: "LBO checks", exact: true });
  await expect(checks).toBeVisible();
  await expect(checks).toHaveScreenshot("lbo-report-checks.png");
});
test("portfolio-risk-report risk budget", async ({ page }) => {
  const item = await open(
    page,
    "portfolio-risk-report",
    "light",
    "comfortable",
  );
  // The four item baselines stop above the budget table.
  const budget = item.getByRole("region", { name: "Risk budget", exact: true });
  await expect(budget.getByRole("table")).toBeVisible();
  await expect(budget).toHaveScreenshot("portfolio-risk-report-budget.png");
});
test("portfolio-risk-report factor contributions", async ({ page }) => {
  const item = await open(
    page,
    "portfolio-risk-report",
    "light",
    "comfortable",
  );
  // Below the preview cut: the host-parity book's factor decomposition.
  const factors = item.getByRole("region", {
    name: "Factor contributions",
    exact: true,
  });
  await expect(factors.getByRole("table").first()).toBeVisible();
  await expect(factors).toHaveScreenshot("portfolio-risk-report-factors.png");
});
test("portfolio-risk-report historical stress attribution", async ({
  page,
}) => {
  const item = await open(
    page,
    "portfolio-risk-report",
    "light",
    "comfortable",
    {
      variant: "historical",
    },
  );
  const stress = item.getByRole("region", {
    name: "Stress attribution",
    exact: true,
  });
  await expect(stress.getByRole("table").first()).toBeVisible();
  await expect(stress).toHaveScreenshot("portfolio-risk-report-stress.png");
});
test("transition-matrix-heatmap cumulative default probability", async ({
  page,
}) => {
  const item = await open(
    page,
    "transition-matrix-heatmap",
    "light",
    "comfortable",
  );
  // The four item baselines stop inside the generator heatmap, above this section.
  const cumulative = item.getByRole("region", {
    name: "Cumulative default probability",
    exact: true,
  });
  await expect(
    cumulative.getByRole("img", {
      name: "Cumulative default probability by horizon",
    }),
  ).toBeVisible();
  await expect(cumulative).toHaveScreenshot(
    "transition-matrix-heatmap-cumulative.png",
  );
});
test("simm-calculator validated set and initial margin", async ({ page }) => {
  const item = await open(page, "simm-calculator", "light", "comfortable");
  // The four item baselines stop inside the sensitivity form, above these sections.
  const sensitivities = item.getByRole("region", {
    name: "SIMM sensitivities",
    exact: true,
  });
  await expect(
    sensitivities.getByRole("region", { name: "Interest rate" }),
  ).toBeVisible();
  await expect(sensitivities).toHaveScreenshot(
    "simm-calculator-sensitivities.png",
  );
  const report = item.getByRole("region", {
    name: "SIMM initial margin",
    exact: true,
  });
  await expect(report.getByLabel("Initial margin")).toHaveText(
    "USD 13,958,205.61533609",
  );
  await expect(report).toHaveScreenshot("simm-calculator-report.png");
});
test("every nonvisual item imports without initializing WASM", async ({
  page,
}) => {
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await page.goto("/registry-gallery?item=nonvisual");
  await page
    .getByRole("button", { name: "Import all nonvisual items" })
    .click();
  await expect(page.getByLabel("Imported item count")).toHaveText(
    String(inventory.nonvisual.length),
  );
  await expect(page.locator('[data-imported="true"]')).toHaveCount(
    inventory.nonvisual.length,
  );
  expect(requests.filter((url) => url.endsWith(".wasm"))).toEqual([]);
});
test("linked table and chart preserve accepted state, nested controls and focus", async ({
  page,
}) => {
  const item = await open(page, "curve-link-example"),
    status = item.locator("[data-link-status]");
  await item.getByRole("gridcell", { name: "OVERLAY", exact: true }).click();
  await expect(status).toContainText(
    'Accepted: ["series","OVERLAY"]; proposals: 1; activations: 1',
  );
  const value = item.getByRole("gridcell", {
    name: "0.9512345678901234",
    exact: true,
  });
  await value.click();
  await expect(value).toHaveAttribute("aria-selected", "true");
  await expect(status).toContainText("proposals: 2; activations: 2");
  await item
    .getByRole("button", { name: "Inspect OVERLAY", exact: true })
    .click();
  await expect(status).toContainText(
    "proposals: 2; activations: 2; detail: Inspect OVERLAY",
  );
  await item.getByRole("button", { name: "Clear", exact: true }).click();
  const chart = item.locator(
    '[aria-label="Linked stored curves stored curves"][tabindex]',
  );
  await chart.focus();
  await chart.press("End");
  await chart.press("Enter");
  await expect(status).toContainText(
    'Accepted: ["point","USD-OIS",40]; proposals: 4; activations: 3',
  );
  await expect(item.locator('td[aria-selected="true"]')).toHaveCount(1);
  await chart.press("Escape");
  const select = item.getByRole("button", { name: "Select overlay point" });
  await select.click();
  await expect(select).toBeFocused();
  await expect(value).toHaveAttribute("aria-selected", "true");
  await item.getByRole("button", { name: "Reverse rows" }).click();
  await expect(value).toHaveAttribute("aria-selected", "true");
  await item.getByRole("button", { name: "Clear", exact: true }).click();
  await expect(item.locator('td[aria-selected="true"]')).toHaveCount(0);
});
test("custom tooltip pinning, keyboard action and dismissal notify once", async ({
  page,
}) => {
  const item = await open(page, "figure-example", "light", "compact", {
    variant: "interactive",
  });
  const chart = item.locator(
    '[aria-label="Interactive observations"][tabindex]',
  );
  await chart.focus();
  await chart.press("Home");
  await chart.press("Enter");
  await expect(item.locator("[data-interaction-status]")).toContainText(
    "Activations: 1; proposals: 1",
  );
  const action = page.getByRole("button", { name: "Open observation details" });
  await expect(action).toBeVisible();
  await chart.press("Tab");
  await expect(action).toBeFocused();
  await action.press("Enter");
  await expect(item.locator("[data-selection-status]")).toContainText(
    "detail: a",
  );
  await expect(item.locator("[data-interaction-status]")).toContainText(
    "Activations: 1; proposals: 1",
  );
  await chart.focus();
  await chart.press("End");
  await chart.press("Enter");
  await page.keyboard.press("Escape");
  await expect(action).toHaveCount(0);
});
test("keyboard traverses every visible bond control, Enter submits and Escape dismisses", async ({
  page,
}) => {
  test.setTimeout(180000);
  const item = await open(page, "instrument-form");
  const count = await item.evaluate((root) => {
    const controls = [
      ...root.querySelectorAll<HTMLElement>(
        "a[href],button,input,textarea,select,summary,[tabindex]",
      ),
    ].filter(
      (node) =>
        node.tabIndex >= 0 &&
        !node.hasAttribute("disabled") &&
        node.getAttribute("aria-disabled") !== "true" &&
        !node.closest("[inert]") &&
        [...root.querySelectorAll("details:not([open])")].every(
          (details) =>
            !details.contains(node) ||
            details.querySelector(":scope > summary")?.contains(node),
        ) &&
        node.getClientRects().length > 0 &&
        getComputedStyle(node).visibility !== "hidden",
    );
    controls.forEach(
      (node, index) => (node.dataset.galleryControl = String(index)),
    );
    return controls.length;
  });
  expect(count).toBeGreaterThan(20);
  await item.locator('[data-gallery-control="0"]').focus();
  const seen = new Set<string>();
  for (let i = 0; i < count + 10; i++) {
    const id = await page.evaluate(
      () => (document.activeElement as HTMLElement)?.dataset.galleryControl,
    );
    if (id !== undefined) seen.add(id);
    if (seen.size === count) break;
    await page.keyboard.press("Tab");
  }
  const missing = await item
    .locator("[data-gallery-control]")
    .evaluateAll(
      (nodes, visited) =>
        nodes
          .filter(
            (n) =>
              !visited.includes((n as HTMLElement).dataset.galleryControl!),
          )
          .map((n) => n.outerHTML),
      [...seen],
    );
  expect(missing).toEqual([]);
  const calendar = item.getByRole("button", { name: / calendar$/i }).first();
  await calendar.focus();
  await calendar.press("Enter");
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(calendar).toBeFocused();
  await item
    .getByRole("textbox", { name: "Amount", exact: true })
    .press("Enter");
  await expect(item.getByLabel("Submission count")).toHaveText("1");
});
