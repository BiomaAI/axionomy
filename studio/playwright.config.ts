import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  use: {
    baseURL: "http://127.0.0.1:5173",
  },
  webServer: [
    {
      command: "AXIONOMY_STUDIO_BIND=127.0.0.1:3001 cargo run -p axionomy-studio-server --bin axionomy-studio",
      cwd: "..",
      url: "http://127.0.0.1:3001/api/problems",
      reuseExistingServer: false,
      timeout: 120_000,
    },
    {
      command: "VITE_AXIONOMY_DEV_SERVER=http://127.0.0.1:3001 pnpm dev --host 127.0.0.1",
      url: "http://127.0.0.1:5173",
      reuseExistingServer: false,
      timeout: 120_000,
    }
  ]
});
