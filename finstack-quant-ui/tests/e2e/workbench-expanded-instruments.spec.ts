import { test, expect } from "playwright/test";

const instruments = [
  ["cds_index", "CDX-IG-42", "hazard_rate"],
  ["cds_option", "CDSOPT-CALL-CORP-5Y", "bloomberg_cdso"],
  ["cds_tranche", "TRANCHE-AUDIT", "hazard_rate"],
  ["swaption", "SWPN-1Yx5Y-USD", "black76"],
  ["bermudan_swaption", "SWPN-1Yx5Y-USD", "hull_white_1f"],
  ["cap_floor", "IRCAP-USD-5Y-3PCT", "black76"],
  ["term_loan", "TERM-LOAN-USD-5Y", "discounting"],
  ["revolving_credit", "RCF-USD-3Y", "discounting"],
  ["convertible_bond", "CONVERTIBLE-AUDIT", "tree"],
] as const;

for (const [type, id, model] of instruments)
  test(`${type} gallery request prices`, async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(
      `/registry-gallery/?item=pricing-workbench&focus=1&instrument=${type}`,
    );
    const workbench = page.getByRole("region", {
      name: "Pricing workbench",
      exact: true,
    });
    await expect(workbench.locator(".finstack-workbench__id")).toHaveText(id);
    await expect(
      workbench.getByRole("combobox", { name: "Pricing model", exact: true }),
    ).toContainText(model);
    await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
      "Priced",
    );
    await expect(workbench).not.toHaveAttribute("data-stale");
    await expect(
      workbench.getByRole("region", { name: "Results", exact: true }),
    ).toContainText("Present value");
    expect(errors).toEqual([]);
  });

test("switching from a rates option loads a credit market and request", async ({
  page,
}) => {
  await page.goto(
    "/registry-gallery/?item=pricing-workbench&focus=1&instrument=swaption",
  );
  const workbench = page.getByRole("region", {
    name: "Pricing workbench",
    exact: true,
  });
  await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
    "Priced",
  );
  await workbench
    .getByRole("combobox", { name: "Instrument type", exact: true })
    .click();
  await page.getByRole("option", { name: "cds_tranche" }).click();
  await expect(page).toHaveURL(/instrument=cds_tranche/);
  await expect(workbench.locator(".finstack-workbench__id")).toHaveText(
    "TRANCHE-AUDIT",
  );
  await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
    "Priced",
  );
  await expect(workbench).not.toHaveAttribute("data-stale");
});
