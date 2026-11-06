import { test, expect } from "playwright/test";

test("pricing-workbench P&L explain from the pinned market to the applied scenario", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=pricing-workbench&theme=light&density=compact&focus=1",
  );
  const workbench = page.getByRole("region", {
    name: "Pricing workbench",
    exact: true,
  });
  await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
    "Priced",
  );
  const results = workbench.getByRole("region", {
    name: "Results",
    exact: true,
  });
  const resultViews = workbench.getByRole("tablist", { name: "Result views" });
  await resultViews.getByRole("tab", { name: "P&L explain" }).click();
  await results.getByRole("button", { name: "Pin as T0" }).click();
  await expect(results.getByText(/T0 pinned at/)).toBeVisible();
  // The gallery seeds a 50bp USD-OIS shift; applying it supplies T1.
  await workbench
    .getByRole("tablist", { name: "Pricing inputs" })
    .getByRole("tab", { name: "Market scenario" })
    .click();
  const apply = workbench.getByRole("button", { name: "Apply scenario" });
  await expect(apply).toBeEnabled();
  await apply.click();
  await expect(results.getByText("Comparison valuation")).toBeVisible();
  await resultViews.getByRole("tab", { name: "P&L explain" }).click();
  await results.getByRole("radio", { name: "Applied market scenario" }).click();
  await results.getByRole("radio", { name: "waterfall" }).click();
  const report = results.getByRole("region", {
    name: "P&L attribution",
    exact: true,
  });
  // Rust's all-rates P&L of the shift, chained over its nine waterfall steps.
  await expect(report.getByTitle("-4895.657693227").first()).toBeVisible();
  await expect(report.getByText("Residual within tolerance")).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  await expect(results).toHaveScreenshot("workbench-pnl-explain.png");
  expect(errors).toEqual([]);
});
