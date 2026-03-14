import type { GraphHealth } from "./types.js";

/**
 * Real-time N★ Bit graph health monitor.
 *
 * The WebSocket endpoint sends graph health snapshots in response to any
 * message. The monitor pings every `pingInterval` ms to keep receiving updates.
 *
 * This is a monitor, not a command channel — it does not send instructions.
 */
export class Monitor {
  private ws: WebSocket | null = null;
  private pingTimer: ReturnType<typeof setInterval> | null = null;
  private stopped = false;

  constructor(
    private readonly wsUrl: string,
    private readonly onUpdate: (health: GraphHealth) => void,
    private readonly pingInterval = 5_000,
  ) {}

  start(): this {
    if (this.ws) return this;
    this._connect();
    return this;
  }

  stop(): void {
    this.stopped = true;
    if (this.pingTimer !== null) {
      clearInterval(this.pingTimer);
      this.pingTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  private _connect(): void {
    // Uses native WebSocket (browser, Node.js 22+, or Deno)
    this.ws = new WebSocket(this.wsUrl);

    const ws = this.ws;
    ws.onopen = () => {
      // Send an initial ping to receive first snapshot
      ws.send("ping");
      this.pingTimer = setInterval(() => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send("ping");
        }
      }, this.pingInterval);
    };

    this.ws.onmessage = (event) => {
      try {
        const health = JSON.parse(
          typeof event.data === "string" ? event.data : String(event.data),
        ) as GraphHealth;
        this.onUpdate(health);
      } catch {
        // Unparseable frame — ignore
      }
    };

    this.ws.onclose = () => {
      if (this.pingTimer !== null) {
        clearInterval(this.pingTimer);
        this.pingTimer = null;
      }
      // Reconnect unless explicitly stopped
      if (!this.stopped) {
        setTimeout(() => this._connect(), 2_000);
      }
    };

    this.ws.onerror = () => {
      // onclose fires after onerror — reconnect logic is there
    };
  }
}
