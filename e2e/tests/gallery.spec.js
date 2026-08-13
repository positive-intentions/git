import { expect, test } from "@playwright/test";

test.describe("git gallery", () => {
  test("home loads with Git story group", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("heading", { name: /git gallery/i })).toBeVisible({
      timeout: 30_000,
    });
    await expect(page.getByText("Git", { exact: true }).first()).toBeVisible();
  });

  test("Init story creates a workdir", async ({ page }) => {
    await page.goto("/demo/gui/git/init");
    await expect(page.getByRole("heading", { name: "Init", level: 1 })).toBeVisible({
      timeout: 30_000,
    });
    await page.getByRole("button", { name: /^Init$/i }).click();
    await expect(page.getByText(/Initialized at/i)).toBeVisible({ timeout: 30_000 });
    await expect(page.getByText("Workdir")).toBeVisible();
  });

  test("Files story write list read", async ({ page }) => {
    await page.goto("/demo/gui/git/files");
    await expect(page.getByRole("heading", { name: "Files", level: 1 })).toBeVisible({
      timeout: 30_000,
    });
    await page.getByRole("button", { name: /^Init$/i }).click();
    await expect(page.getByText(/Ready at/i)).toBeVisible({ timeout: 30_000 });
    await page.getByRole("button", { name: /^Write$/i }).click();
    await expect(page.getByText(/Wrote/i)).toBeVisible({ timeout: 30_000 });
    await page.getByRole("button", { name: /^List$/i }).click();
    await expect(page.getByText(/hello\.txt|notes/i).first()).toBeVisible({
      timeout: 30_000,
    });
    await page.getByRole("button", { name: /^Read$/i }).click();
    await expect(page.getByText(/hello from git-gallery/i)).toBeVisible({
      timeout: 30_000,
    });
  });

  test("Status story mutation then status", async ({ page }) => {
    await page.goto("/demo/gui/git/status");
    await expect(page.getByRole("heading", { name: "Status", level: 1 })).toBeVisible({
      timeout: 30_000,
    });
    await page.getByRole("button", { name: /^Init$/i }).click();
    await expect(page.getByText(/Ready at/i)).toBeVisible({ timeout: 30_000 });
    await page.getByRole("button", { name: /Write sample/i }).click();
    await expect(page.getByText(/Wrote \+ staged sample\.txt/i)).toBeVisible({
      timeout: 30_000,
    });
    await page.getByRole("button", { name: /^Status$/i }).click();
    await expect(page.getByText(/sample\.txt/i).first()).toBeVisible({
      timeout: 30_000,
    });
  });

  test("coverage page shows report or missing instructions", async ({ page }) => {
    await page.goto("/coverage");
    await expect(page.getByRole("link", { name: /open in new tab/i })).toBeVisible({
      timeout: 30_000,
    });
  });

  test("unknown route shows not found content", async ({ page }) => {
    await page.goto("/this/route/does-not-exist");
    await expect(page.getByText(/not found/i).first()).toBeVisible({
      timeout: 30_000,
    });
  });

  // Does not perform a network clone — only checks the story chrome loads.
  test("Clone story loads connect form and controls", async ({ page }) => {
    await page.goto("/demo/gui/git/clone");
    await expect(page.getByRole("heading", { name: "Clone", level: 1 })).toBeVisible({
      timeout: 30_000,
    });
    await expect(page.getByText("Remote connection")).toBeVisible();
    await expect(page.getByText("URL", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("Username", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("Access token", { exact: true }).first()).toBeVisible();
    await expect(page.getByRole("button", { name: /Clone/i })).toBeVisible();
    await expect(page.getByRole("button", { name: /Fetch/i })).toBeVisible();
    await expect(page.getByRole("button", { name: /Push/i })).toBeVisible();
    await expect(page.getByText("Workspace")).toBeVisible();
    // Monaco mount point (web gallery debugger) must have a real height.
    const monaco = page.locator("#git-gallery-monaco");
    await expect(monaco).toBeVisible({ timeout: 30_000 });
    await expect
      .poll(async () => monaco.evaluate((el) => el.clientHeight), { timeout: 30_000 })
      .toBeGreaterThanOrEqual(240);
  });
});
