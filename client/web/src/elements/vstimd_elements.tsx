// SPDX-License-Identifier: AGPL-3.0-or-later
// The `/elements/` contract, for vstimd. Served at `/elements/vstimd.js` --
// see `server/src/web/mod.rs` (CORS, and the embedded-asset route) and
// `vite.config.ts` (the second build entry that produces this bundle).
//
// statemachined already publishes this contract (dev/DAEMON.md §5) and names
// `braemons-console` as its consumer. This is what lets the console hold no
// domain logic at all: every panel is served by the daemon that owns the
// hardware it is about, at that daemon's own version, and the console never
// bundles a copy of it.
//
// The awkward part was that vstimd's UI is React and the contract is custom
// elements. That turns out to cost about forty lines, because the panels were
// already written as functions of (conn, snapshot) with no global state and no
// stylesheet -- every style in them is an inline `style={{}}`, which is
// element-local and therefore already shadow-DOM-safe. Nothing in the panels
// below changed to get here; this file only wraps them.

import { StrictMode, useEffect, useState } from "react";
import { createRoot, type Root } from "react-dom/client";

import { Connection, type SceneSnapshot } from "../index.js";
import { StimuliPanel } from "../app/StimuliPanel.js";
import { StimulusMap } from "../app/StimulusMap.js";
import { VtlPanel } from "../app/VtlPanel.js";
import { AnimationsPanel } from "../app/AnimationsPanel.js";
import { SystemPanel } from "../app/SystemPanel.js";

/** What every panel here is a function of. */
interface PanelProps {
  conn: Connection | null;
  snapshot: SceneSnapshot | null;
}

/**
 * Three states, not two. "connecting" and "unreachable" have to be different
 * words on the screen: a panel that only knows connected/not sits on
 * "connecting…" indefinitely when its daemon is down, which reads as *slow*
 * when it means *absent* -- and a person deciding whether to walk to the
 * booth is being told the wrong thing by a spinner that never resolves.
 */
type LinkState = "connecting" | "connected" | "unreachable";

/**
 * One connection per element.
 *
 * Deliberately *not* one shared connection per `base`: an element must be
 * usable on its own, and a console that shows only the map should not keep a
 * command channel open for panels nobody asked for. vstimd's two sockets are
 * cheap and the daemon is on the same rig. If a console ever puts six of these
 * on one page this is the thing to revisit -- it is a cache, not a rewrite.
 */
function useConnection(base: string): PanelProps & { link: LinkState } {
  const [conn, setConn] = useState<Connection | null>(null);
  const [snapshot, setSnapshot] = useState<SceneSnapshot | null>(null);
  const [link, setLink] = useState<LinkState>("connecting");

  useEffect(() => {
    let closed = false;
    let sub: { close(): void } | undefined;
    let live: Connection | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let attempt = 0;

    // Retry, because a console is usually left open across the thing it is
    // watching: a rig restarted between blocks, a daemon upgraded, a cable.
    // A panel that needs a page reload to notice its daemon came back is a
    // panel somebody stops trusting. Backoff caps at 10s so a rig that was off
    // all night is picked up within ten seconds of coming up, without a browser
    // tab hammering a closed port for hours in between.
    const attach = async () => {
      try {
        const c = await Connection.connect(base);
        if (closed) return c.close();
        live = c;
        attempt = 0;
        setConn(c);
        setLink("connected");
        sub = await c.events(setSnapshot);
      } catch {
        if (closed) return;
        setLink("unreachable");
        setConn(null);
        setSnapshot(null);
        timer = setTimeout(attach, Math.min(1000 * 2 ** attempt++, 10_000));
      }
    };
    attach();

    return () => {
      closed = true;
      clearTimeout(timer);
      sub?.close();
      // The connection this effect opened -- not one from a previous render,
      // which is what reading `conn` here would sometimes get.
      live?.close();
    };
  }, [base]);

  return { conn, snapshot, link };
}

/**
 * The `base` attribute, as a WebSocket origin.
 *
 * `base` is an attribute and not an assumption because a console is not served
 * from the rig -- the same reason statemachined's and triald's elements take
 * one. An http(s) base is accepted and converted, since that is what an mDNS
 * record and a person both produce.
 */
function websocketBase(base: string | null): string {
  if (!base) return `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}`;
  return base.replace(/^http:/, "ws:").replace(/^https:/, "wss:").replace(/\/+$/, "");
}

/** Wrap one panel as a custom element with a shadow root. */
function definePanel(tag: string, Panel: (p: PanelProps) => JSX.Element, label: string) {
  class PanelElement extends HTMLElement {
    static observedAttributes = ["base"];
    #root: Root | null = null;

    connectedCallback() {
      if (!this.#root) this.#root = createRoot(this.attachShadow({ mode: "open" }));
      this.#render();
    }

    attributeChangedCallback() {
      if (this.#root) this.#render();
    }

    disconnectedCallback() {
      // Unmount on the next tick: React refuses to unmount during a render, and
      // a console that moves a panel between containers disconnects and
      // reconnects it in the same task.
      const root = this.#root;
      this.#root = null;
      queueMicrotask(() => root?.unmount());
    }

    #render() {
      const base = websocketBase(this.getAttribute("base"));
      this.#root!.render(
        <StrictMode>
          <Frame base={base} label={label} Panel={Panel} />
        </StrictMode>,
      );
    }
  }
  customElements.define(tag, PanelElement);
}

/**
 * The sentence, the connection state, and the panel.
 *
 * The sentence is statemachined's rule and it applies for the same reason: a
 * console embeds these and has no tabs of its own, so a panel that does not say
 * what it is says nothing. The connection state is here rather than in the
 * console because only the element knows whether *its* daemon answered -- a
 * console showing one rig-wide "connected" light would be lying about the other
 * daemon the moment one of them goes down.
 */
function Frame({ base, label, Panel }: { base: string; label: string; Panel: (p: PanelProps) => JSX.Element }) {
  const { conn, snapshot, link } = useConnection(base);
  const colour = { connecting: "#c84", connected: "#4c8", unreachable: "#c44" }[link];
  const words = { connecting: "connecting…", connected: "connected", unreachable: `unreachable — retrying` }[link];
  return (
    <div style={{ fontFamily: "system-ui, sans-serif", color: "#ddd", background: "#1a1a1a", padding: 12, borderRadius: 6 }}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 8, marginBottom: 8 }}>
        <strong style={{ fontSize: 13 }}>vstimd</strong>
        <span style={{ color: "#888", fontSize: 12 }}>{label}</span>
        <span style={{ marginLeft: "auto", fontSize: 12, color: colour }} title={base}>
          {words}
        </span>
      </div>
      <Panel conn={conn} snapshot={snapshot} />
    </div>
  );
}

definePanel("vstimd-map", StimulusMap, "the screen, to scale — drag to map a receptive field");
definePanel("vstimd-stimuli", StimuliPanel, "what is on the screen, and what each one is");
definePanel("vstimd-lines", VtlPanel, "the virtual trigger lines, as the server sees them now");
definePanel("vstimd-animations", AnimationsPanel, "what is armed, and what it is waiting for");
definePanel("vstimd-system", SystemPanel, "background, photodiode, and the whole-scene switches");

/** So a console can iterate the panels rather than hard-code a list that goes stale. */
export const VSTIMD_ELEMENT_NAMES = [
  "vstimd-map",
  "vstimd-stimuli",
  "vstimd-lines",
  "vstimd-animations",
  "vstimd-system",
];
