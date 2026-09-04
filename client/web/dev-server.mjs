// A Vite-free dev server for the web UI, kept as evidence rather than as the
// default: esbuild for TS+JSX+bundling, and a raw-socket proxy for the two
// WebSocket endpoints. Same contract as vite.config.ts -- serve index.html and
// the bundle, forward /ws and /events -- in about forty lines.
//
// It exists because the console asked whether every braemons UI must adopt this
// one's build (dev/design/WEB_BUILD_TOOLING.md). It must not: the whole
// Playwright suite passes against this server, unmodified.
//
//     VSTIMD_BIN=target/release/vstimd \
//       npx playwright test -c playwright.novite.config.ts
//
// The recommendation is still to keep Vite -- vitest depends on it either way --
// so this is the thing to re-run if the question comes back, not the thing to
// develop against.
import http from "node:http";
import net from "node:net";
import { readFileSync } from "node:fs";
import * as esbuild from "esbuild";

const ROOT = process.env.WEB_ROOT;
const PORT = Number(process.env.UI_PORT ?? 4174);
const BACKEND = new URL(process.env.VSTIMD_WEB ?? "http://127.0.0.1:8080");

const ctx = await esbuild.context({
  entryPoints: [`${ROOT}/src/app/main.tsx`],
  bundle: true, format: "esm", target: "es2022", jsx: "automatic",
  sourcemap: true, outfile: "app.js", write: false,
  define: { "process.env.NODE_ENV": '"development"' },
});
let bundle = "";
const rebuild = async () => {
  const out = (await ctx.rebuild()).outputFiles;
  bundle = out.find((f) => f.path.endsWith(".js")).text;
};
await rebuild();

const html = readFileSync(`${ROOT}/index.html`, "utf8")
  .replace('src="/src/app/main.tsx"', 'src="/app.js"');

const server = http.createServer((req, res) => {
  if (req.url === "/app.js") { res.writeHead(200, { "content-type": "text/javascript" }); res.end(bundle); return; }
  res.writeHead(200, { "content-type": "text/html" }); res.end(html);
});

// WebSocket upgrade: hand the raw socket to the backend and pipe both ways.
server.on("upgrade", (req, socket, head) => {
  const up = net.connect(Number(BACKEND.port), BACKEND.hostname, () => {
    up.write(`GET ${req.url} HTTP/1.1\r\n`);
    for (const [k, v] of Object.entries(req.headers)) up.write(`${k}: ${v}\r\n`);
    up.write("\r\n");
    if (head?.length) up.write(head);
    up.pipe(socket); socket.pipe(up);
  });
  up.on("error", () => socket.destroy());
  socket.on("error", () => up.destroy());
});

server.listen(PORT, "127.0.0.1", () => console.log(`ui on http://127.0.0.1:${PORT}`));
