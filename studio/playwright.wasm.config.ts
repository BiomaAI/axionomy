import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  testMatch: "wasm.spec.ts",
  timeout: 60_000,
  use: {
    baseURL: "http://127.0.0.1:5174",
  },
  webServer: {
    command: "VITE_AXIONOMY_ENGINE=browser pnpm dev --host 127.0.0.1 --port 5174",
    url: "http://127.0.0.1:5174",
    reuseExistingServer: false,
    timeout: 180_000,
  },
});
