// Experiment: the exact same browser suite, served by esbuild + a raw-socket
// WS proxy instead of Vite. Only the UI webServer command differs.
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { defineConfig } from "@playwright/test";

const VSTIMD_WEB_PORT = 8139;
const VSTIMD_ZMQ_PORT = 5567;
const UI_PORT = 4174;
const REPO_ROOT = new URL("../..", import.meta.url).pathname;
const STORAGE_DIR = mkdtempSync(join(tmpdir(), "vstimd-novite-"));

export default defineConfig({
  testDir: "./playwright",
  timeout: 30_000,
  expect: { timeout: 10_000 },
  use: { baseURL: `http://127.0.0.1:${UI_PORT}` },
  webServer: [
    {
      command: `${process.env.VSTIMD_BIN} --null --web-port ${VSTIMD_WEB_PORT} --zmq-port ${VSTIMD_ZMQ_PORT} --storage-dir ${STORAGE_DIR}`,
      cwd: REPO_ROOT,
      url: `http://127.0.0.1:${VSTIMD_WEB_PORT}/`,
      reuseExistingServer: false,
      timeout: 180_000,
    },
    {
      command: `node ./dev-server.mjs`,
      env: { ...process.env, WEB_ROOT: process.cwd(), UI_PORT: String(UI_PORT), VSTIMD_WEB: `http://127.0.0.1:${VSTIMD_WEB_PORT}` },
      url: `http://127.0.0.1:${UI_PORT}`,
      reuseExistingServer: false,
      timeout: 60_000,
    },
  ],
});
