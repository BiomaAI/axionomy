import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  // The Worker's own tests, and the packages workspace on the engine in the browser.
  testMatch: ["wasm.spec.ts", "packages.spec.ts"],
  timeout: 60_000,
  use: {
    baseURL: "http://127.0.0.1:5174",
  },
  webServer: {
    // Reuse the Wasm artifacts from `pnpm build`; optimization is not server startup.
    command: "VITE_AXIONOMY_ENGINE=browser pnpm exec vite --host 127.0.0.1 --port 5174",
    url: "http://127.0.0.1:5174",
    reuseExistingServer: false,
    timeout: 180_000,
  },
});
