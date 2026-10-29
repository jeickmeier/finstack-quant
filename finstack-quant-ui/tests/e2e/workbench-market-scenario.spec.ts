import { test, expect } from "playwright/test";

test("pricing-workbench market-scenario applied beside the base valuation", async ({
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
  await expect(results.getByText("Comparison valuation")).toHaveCount(0);
  await workbench
    .getByRole("tablist", { name: "Pricing inputs" })
    .getByRole("tab", { name: "Market scenario" })
    .click();
  // The gallery seeds a 50bp USD-OIS shift; Rust builds its spec before it can apply.
  const apply = workbench.getByRole("button", { name: "Apply scenario" });
  await expect(apply).toBeEnabled();
  await apply.click();
  await expect(results.getByText("Comparison valuation")).toBeVisible();
  await expect(results.getByTitle("995488.150413341").first()).toBeVisible();
  await workbench
    .getByRole("tablist", { name: "Result views" })
    .getByRole("tab", { name: "Market scenario" })
    .click();
  await expect(
    results.getByRole("table", { name: "Applied shocks" }),
  ).toContainText("USD-OIS");
  // Rust's horizon return of the same request under the applied scenario.
  const horizon = results.getByRole("region", {
    name: "Horizon return report",
    exact: true,
  });
  await expect(horizon.getByText("Total return (decimal)")).toBeVisible();
  await expect(
    horizon.getByText("Annualized return (decimal)").locator(".."),
  ).toContainText("not defined");
  await page.evaluate(() => document.fonts.ready);
  await expect(results).toHaveScreenshot("workbench-market-scenario.png");
  expect(errors).toEqual([]);
});
