import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  use: {
    baseURL: "http://127.0.0.1:5173",
  },
  webServer: [
    {
      command: "cargo run -p axionomy-studio-server --bin axionomy-studio",
      cwd: "..",
      url: "http://127.0.0.1:3000/api/problems",
      reuseExistingServer: true,
      timeout: 120_000,
    },
    {
      command: "pnpm dev --host 127.0.0.1",
      url: "http://127.0.0.1:5173",
      reuseExistingServer: true,
      timeout: 120_000,
    }
  ]
});
