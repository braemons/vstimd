// Virtual trigger line list + level indicators, mirroring the egui VTL overlay.
// Reads live state from snapshot.vtlLines and drives conn.vtl.

import type { Connection, SceneSnapshot } from "../index.js";

interface Props {
  conn: Connection | null;
  snapshot: SceneSnapshot | null;
}

export function VtlPanel({ conn, snapshot }: Props) {
  const lines = snapshot?.vtlLines ?? [];

  return (
    <div style={{ minWidth: 260 }}>
      <h3>Trigger Lines</h3>
      {lines.length === 0 ? (
        <p style={{ color: "#666", fontSize: 13 }}>No lines registered.</p>
      ) : (
        <table style={{ width: "100%", fontSize: 13, borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", color: "#888" }}>
              <th>Line</th><th>Dir</th><th>Level</th><th></th>
            </tr>
          </thead>
          <tbody>
            {lines.map((l) => (
              <tr key={`${l.bank}:${l.bit}`}>
                <td>{l.name || <span style={{ color: "#888" }}>{l.bank}:{l.bit}</span>}</td>
                <td style={{ color: "#888" }}>{l.direction}</td>
                <td>
                  <span
                    title={l.high ? "high" : "low"}
                    style={{
                      display: "inline-block", width: 10, height: 10, borderRadius: "50%",
                      background: l.high ? "#4c8" : "#444",
                      boxShadow: l.high ? "0 0 6px #4c8" : "none",
                    }}
                  />
                </td>
                <td>
                  {l.direction === "input" && (
                    <button
                      disabled={!conn}
                      onClick={() => conn?.vtl.toggleInput({ bank: l.bank, bit: l.bit })}
                    >
                      toggle
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
