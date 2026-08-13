import { defineConfig } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const candidates = [
  process.env.GALLERY_DIST,
  path.join(root, "git-gallery/target/dx/git-gallery/release/web/public"),
  path.join(root, "git-gallery/target/dx/git-gallery/debug/web/public"),
].filter(Boolean);

const serveDir = candidates.find((dir) => fs.existsSync(dir));
if (!serveDir) {
  throw new Error(
    "Gallery dist not found. Run: cd git-gallery && dx build --platform web --bin git-gallery",
  );
}

export default defineConfig({
  testDir: "./tests",
  timeout: 60_000,
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  use: {
    baseURL: "http://127.0.0.1:4173",
    trace: "on-first-retry",
  },
  webServer: {
    command: `npx --yes serve@14 "${serveDir}" -l 4173 -s`,
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
