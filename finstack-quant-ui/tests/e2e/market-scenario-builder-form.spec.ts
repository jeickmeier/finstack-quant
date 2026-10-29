import { test, expect } from "playwright/test";

// The gallery preview stops near 1100px, above the generated operation form;
// this screenshots that form on its own.
test("market-scenario-builder operations form with id comboboxes", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=market-scenario-builder&theme=light&density=compact&focus=1",
  );
  const builder = page.getByRole("region", {
    name: "Market scenario builder",
    exact: true,
  });
  // The example starts from the remapped rates component composed with equity.
  await expect(
    builder.getByRole("region", { name: "Scenario spec" }),
  ).toContainText("rate_shock_2022_rates+rate_shock_2022_equity");
  const form = builder.locator("form.finstack-term-sheet");
  await expect(form.getByRole("combobox", { name: "Curve id" })).toHaveValue(
    "USD-OIS",
  );
  await expect(
    form.getByRole("combobox", { name: "Ids 1" }).first(),
  ).toHaveValue("SPX");
  await page.evaluate(() => document.fonts.ready);
  await expect(form).toHaveScreenshot("market-scenario-builder-form.png");
  expect(errors).toEqual([]);
});
