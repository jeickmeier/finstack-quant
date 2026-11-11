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
  // Focus mode shows the whole item, uncropped.
  await page.goto(
    `/registry-gallery?${new URLSearchParams({ item: name, theme, density, focus: "1", ...extra })}`,
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
// Axe runs in all four modes; these two cover both themes and both densities in pixels.
const screenshotModes = ["light-compact", "dark-comfortable"];
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
        if (!screenshotModes.includes(`${theme}-${density}`)) return;
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
test("positions-table selected swap", async ({ page }) => {
  const item = await open(page, "positions-table", "light", "comfortable");
  await item.getByRole("row", { name: /IRS-1/ }).click();
  const swap = item.getByRole("region", {
    name: "Position IRS-1",
    exact: true,
  });
  await expect(swap.getByText("IRS-5Y-USD-STD").first()).toBeVisible();
  await expect(swap).toHaveScreenshot("positions-table-swap.png");
});
test("curve-query selected surface", async ({ page }) => {
  const item = await open(page, "curve-query", "light", "comfortable");
  await item.getByRole("button", { name: /^Inspect EQ-VOL, / }).click();
  const lookup = item.getByRole("region", {
    name: "Point lookup for EQ-VOL",
  });
  await expect(lookup.getByRole("table")).toBeVisible();
  await expect(lookup).toHaveScreenshot("curve-query-surface.png");
});
test("waterfall-report period detail", async ({ page }) => {
  const item = await open(page, "waterfall-report", "light", "comfortable");
  const report = item.getByRole("region", { name: "Waterfall report" });
  await report.getByRole("button", { expanded: false }).first().click();
  await expect(
    report.getByRole("table", { name: "Payment records" }),
  ).toBeVisible();
  await expect(report).toHaveScreenshot("waterfall-report-period-detail.png");
});
test("coverage-test-chart a trigger with no ratio by date", async ({
  page,
}) => {
  const item = await open(page, "coverage-test-chart", "light", "comfortable");
  const report = item.getByRole("region", { name: "Coverage tests" });
  await report.getByRole("combobox", { name: "Test" }).click();
  await page.getByRole("option", { name: "OC_EQUITY_trigger" }).click();
  await expect(
    report.getByText(/^Recorded on 0 of \d+ dates\.$/),
  ).toBeVisible();
  await expect(
    report.getByText("First breach 2026-07-01", { exact: false }),
  ).toBeVisible();
});
test("relative-value-table regression and a narrower peer filter", async ({
  page,
}) => {
  const item = await open(page, "relative-value-table", "light", "comfortable");
  const regression = item.getByRole("region", { name: "Spread vs leverage" });
  await expect(
    regression.getByRole("img", { name: /OAS bp on Leverage/ }),
  ).toBeVisible();
  // Screen on the utilities instead: two peers, too few for the regression.
  const sector = item.getByRole("textbox", { name: "Gics sectors 1" });
  await sector.fill("Utilities");
  await item.getByRole("button", { name: "Compare" }).click();
  await expect(
    item
      .getByRole("table", { name: "Comparable companies, LTM" })
      .getByRole("rowheader"),
  ).toHaveText(["NORTHWINDSubject", "DOGWOOD", "IRONWOOD"]);
  await expect(
    regression.getByText(/Excluded from the composite: Regression unusable/),
  ).toBeVisible();
});
test("portfolio-margin-report selected netting set", async ({ page }) => {
  const item = await open(
    page,
    "portfolio-margin-report",
    "light",
    "comfortable",
  );
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
  const im = item.getByRole("group", { name: "Im params" });
  await expect(im).toBeVisible();
  await expect(im).toHaveScreenshot("csa-editor-im-params.png");
});
test("dcf-assumptions-form options", async ({ page }) => {
  const item = await open(page, "dcf-assumptions-form", "light", "comfortable");
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
test("statement-monte-carlo results", async ({ page }) => {
  const item = await open(
    page,
    "statement-monte-carlo",
    "light",
    "comfortable",
  );
  const results = item.getByRole("group", {
    name: "Returned Monte Carlo results",
  });
  await expect(
    results.getByRole("table", {
      name: "Leverage percentiles by forecast period",
    }),
  ).toBeVisible();
  await expect(
    results.getByRole("region", { name: "Breach probability" }),
  ).toContainText("14%");
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
test("simm-calculator validated set and initial margin", async ({ page }) => {
  const item = await open(page, "simm-calculator", "light", "comfortable");
  const sensitivities = item.getByRole("region", {
    name: "SIMM sensitivities",
    exact: true,
  });
  await expect(
    sensitivities.getByRole("region", { name: "Interest rate" }),
  ).toBeVisible();
  const report = item.getByRole("region", {
    name: "SIMM initial margin",
    exact: true,
  });
  await expect(report.getByLabel("Initial margin")).toHaveText(
    "USD 13,958,205.61533609",
  );
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
