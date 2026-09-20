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
