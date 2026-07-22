// Client side of the relay. Connects over WebSocket, sends/receives encrypted
// envelopes, and reconnects on drop. Content is always ciphertext here.
import { API, getToken } from "./api";

export type Incoming =
  | {
      type: "msg";
      id: number;
      from: number;
      nonce: string;
      ciphertext: string;
      ts: number;
    }
  | { type: "receipt"; from: number; receipt: "delivered" | "read"; mid?: string }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  | { type: "signal"; from: number; signal: any };

let ws: WebSocket | null = null;
let handler: ((e: Incoming) => void) | null = null;
let closed = false;
const outbox: string[] = [];

function open() {
  const token = getToken();
  if (!token || closed) return;
  if (ws && ws.readyState !== WebSocket.CLOSED) return; // one socket only
  const base = API || location.origin; // same-origin when API is ""
  const url = base.replace(/^http/, "ws") + "/ws?token=" + encodeURIComponent(token);
  ws = new WebSocket(url);
  ws.onopen = () => {
    while (outbox.length && ws && ws.readyState === WebSocket.OPEN) {
      ws.send(outbox.shift()!);
    }
  };
  ws.onmessage = (ev) => {
    try {
      const e = JSON.parse(ev.data);
      if (
        e &&
        (e.type === "msg" || e.type === "receipt" || e.type === "signal") &&
        handler
      ) {
        handler(e as Incoming);
      }
    } catch {
      /* ignore */
    }
  };
  ws.onclose = () => {
    ws = null;
    if (!closed) setTimeout(open, 1500);
  };
  ws.onerror = () => {
    try {
      ws?.close();
    } catch {
      /* ignore */
    }
  };
}

export function connectWs(onMessage: (e: Envelope) => void) {
  handler = onMessage;
  closed = false;
  open();
}

function raw(payload: string) {
  if (ws && ws.readyState === WebSocket.OPEN) ws.send(payload);
  else outbox.push(payload); // flushed on (re)connect
}

export function sendMsg(to: number, nonce: string, ciphertext: string, ts: number) {
  raw(JSON.stringify({ to, type: "msg", nonce, ciphertext, ts }));
}

export function sendReceipt(
  to: number,
  receipt: "delivered" | "read",
  mid?: string,
) {
  raw(JSON.stringify({ to, type: "receipt", receipt, mid }));
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function sendSignal(to: number, signal: any) {
  raw(JSON.stringify({ to, type: "signal", signal }));
}

export function disconnectWs() {
  closed = true;
  if (ws) {
    // Detach handlers so a socket still closing can't deliver stray messages.
    ws.onmessage = null;
    ws.onclose = null;
    ws.onerror = null;
    ws.onopen = null;
    try {
      ws.close();
    } catch {
      /* ignore */
    }
    ws = null;
  }
}
