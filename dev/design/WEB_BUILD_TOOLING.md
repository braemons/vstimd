# Does the web UI need Vite?

**Short answer: no, and that matters less than it sounds.** Nothing in
`client/web` is load-bearing on Vite — the whole UI builds and passes its browser
suite on plain esbuild plus a 40-line dev server, and the evidence is in this
branch. But removing Vite from the build does *not* remove it from the tree,
because `vitest` depends on it, so the saving is one config file rather than a
dependency. The reason to have asked is the console, and there the answer is
sharper: see §5.

Explored on `explore/web-build-tooling` for the braemons console
(`braemons/console`), whose `experiments/01-no-vite/` records this.

## 1. What Vite is actually doing here

Three things, and only three:

| | |
|---|---|
| TS + JSX transform | esbuild, which is what Vite calls |
| bundling bare specifiers | `react`, `react-dom`, `@bufbuild/protobuf` — a rig box has no CDN, so *something* must bundle |
| dev proxy | `/ws` and `/events` to the running server, so the dev page can use same-origin URLs |

Everything else Vite is good at, this app does not use. It has **no CSS files**
(every style is an inline `style={{}}`), no asset imports, no `import.meta.env`,
no plugins beyond `@vitejs/plugin-react`, and no HMR-dependent workflow. The
config is 17 lines and 12 of them are the proxy.

## 2. The measurement

Same source, same entry point, this machine:

| | Vite 5.4 | esbuild 0.21 alone |
|---|---|---|
| production build | **776 ms** | **24 ms** |
| bundle | 273.4 kB (83.0 kB gzip) | 266.1 kB minified |
| config | `vite.config.ts` + plugin | 11 lines of flags |
| dev server | built in, with HMR | `client/web/dev-server.mjs`, 40 lines, no HMR |

`dev-server.mjs` in this branch is the replacement: esbuild in watch mode for the
bundle, and a raw-socket `upgrade` handler for the two WebSocket endpoints. The
proxy is ~15 lines because a WebSocket proxy that does not inspect frames is just
`socket.pipe(upstream)` in both directions.

**The whole Playwright suite passes against it**, unmodified:

```console
$ VSTIMD_BIN=target/release/vstimd npx playwright test -c playwright.novite.config.ts
  8 passed (3.8s)
```

That is the claim worth having: not that a hello-world builds without Vite, but
that the map drag, the VTL bit grid, the animation arm and the scene-config
round-trip all do.

## 3. What it costs

Three things, and the third is the one that decides it.

- **No HMR.** esbuild has watch-and-rebuild, not module replacement, so a save is
  a full page reload. At 24 ms plus reload that is fast enough to be unnoticeable
  here, and it would not be on a UI with a form somebody is halfway through
  filling in.
- **No CSS pipeline.** Free today (there are no stylesheets) and not free the
  first time somebody wants one. Inline styles do not scale to a themeable UI,
  and §5 has a reason the console will eventually want one.
- **`vitest` depends on `vite`.** This is the finding that changes the answer.
  The node WebSocket e2e (`tests/e2e.test.ts`, 19 `it`s, 43 assertions) runs
  under vitest, so vite stays installed whatever the build does. Porting it to
  `node --test` is not free either: the suite uses NodeNext-style `.js`
  specifiers for `.ts` files throughout, and **Node's native type stripping does
  not rewrite `.js` → `.ts`** — verified on Node 24.20, `ERR_MODULE_NOT_FOUND`.
  So that port is "touch every import in the client" plus rewriting 43 `expect`s
  as asserts, to remove a dependency that is a test runner.

## 4. Recommendation for vstimd

**Keep Vite, and stop treating it as a decision.** It is 776 ms and one config
file; the dependency it would save is retained by vitest anyway; and the two
things it does badly here (no CSS story, inline styles everywhere) are not fixed
by removing it.

What this exploration was actually worth is the negative result: **nothing in the
UI depends on Vite**, so it is not a constraint on anything downstream. When the
console asked "must every braemons UI adopt vstimd's build?", the answer is no,
and now there is a passing test suite behind that instead of an opinion.

Keep `dev-server.mjs` and `playwright.novite.config.ts` in the tree as the
evidence. They cost nothing and they are the thing to re-run if the question
comes back.

## 5. The console, which is where this actually bites

The console (`braemons/console`) is a shell with **no domain logic**: every panel
is a custom element served by the daemon that owns the hardware it is about —
statemachined's contract, `dev/DAEMON.md` §5, which names `braemons-console` as
its consumer. A shell like that imports modules over HTTP and bundles nothing, so
it has **no npm dependencies at all**, and the question of its bundler does not
arise. It should never acquire one.

The build question therefore belongs to each daemon separately, and **vstimd is
the only one that has it** — statemachined and triald are vanilla by rule. Which
means the honest framing is not "does the console need Vite" but "vstimd's build
is vstimd's business, as long as it can emit custom elements."

It can. `braemons/console`'s experiment 02 wraps five existing vstimd panels as
`vstimd-map`, `vstimd-stimuli`, `vstimd-lines`, `vstimd-animations`,
`vstimd-system` in about forty lines, and runs them on one page beside
statemachined's, against a live board. Nothing in the panels changed — they were
already functions of `(conn, snapshot)` with no global state, and their inline
styles are element-local and therefore already shadow-DOM-safe.

What vstimd owes the console, none of which is a build decision:

1. **an `/elements/vstimd.js` entry point** — a second esbuild/Vite entry beside
   `main.tsx`, exporting nothing and registering the tags;
2. **CORS on `/elements/`** — `server/src/web/mod.rs` sets no CORS headers today,
   and a cross-origin module script requires them. (The two WebSockets do not:
   `/ws` and `/events` already work cross-origin.)
3. **an mDNS record a console can use** — see `braemons/console` `docs/PLAN.md`
   §4. Today `packaging/avahi/vstimd.service.tmpl` advertises **port 5555, the
   ZMQ port**, with `id=` and nothing else. A console cannot find the web port,
   the API path or the elements URL from it, and `id=vstimd-XXXXXX` cannot be
   matched against statemachined's `sha256("statemachined:" + machine-id)` to
   decide the two are one rig.
