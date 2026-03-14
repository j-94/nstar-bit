import type {
  Signal,
  Receipt,
  Belief,
  BeliefQuery,
  EvidenceDelta,
  EvidenceResponse,
  SynthesisRequest,
  SynthesisResponse,
  Manifest,
  GraphHealth,
  NstarClientOptions,
} from "./types.js";
import { Monitor } from "./monitor.js";

/**
 * N★ Bit Epistemic API Client
 *
 * Core contract:
 *   client.turn(signal)    → Receipt  (immutable proof the turn happened)
 *   client.belief(query)   → Belief   (evidence-gated confidence + delusion_delta)
 *   client.manifest()      → Manifest (what the system knows right now)
 *   client.monitor(fn)     → Monitor  (real-time graph health stream)
 */
export class NstarClient {
  private baseUrl: string;
  private timeout: number;

  constructor(options: NstarClientOptions = {}) {
    this.baseUrl = (options.baseUrl ?? "http://localhost:8080").replace(/\/$/, "");
    this.timeout = options.timeout ?? 30_000;
  }

  /** Submit a signal. The graph updates. Returns a Receipt. */
  async turn(signal: Signal): Promise<Receipt> {
    return this._post<Receipt>("/turn", signal);
  }

  /**
   * Query evidence-gated confidence for a belief.
   *
   * Always returns delusion_delta — the gap between declared and actual
   * confidence. This is the primary value: knowing how wrong your claim is.
   *
   * If declared_confidence is not provided, delusion_delta defaults to 0.
   */
  async belief(query: BeliefQuery): Promise<Belief> {
    const params = new URLSearchParams();
    if (query.relation_id) {
      params.set("relation_id", query.relation_id);
    } else {
      if (query.source) params.set("source", query.source);
      if (query.relation) params.set("relation", query.relation);
      if (query.target) params.set("target", query.target);
    }
    if (query.declared_confidence !== undefined) {
      params.set("declared_confidence", String(query.declared_confidence));
    }
    return this._get<Belief>(`/belief?${params}`);
  }

  /** Inject evidence for or against a relation. */
  async evidence(delta: EvidenceDelta): Promise<EvidenceResponse> {
    return this._post<EvidenceResponse>("/evidence", delta);
  }

  /**
   * Feed multiple signals for cross-signal synthesis.
   *
   * Synthesis is async — the result lands in /manifest within ~30s.
   * This call returns immediately with status="synthesis_queued".
   */
  async synthesize(request: SynthesisRequest): Promise<SynthesisResponse> {
    return this._post<SynthesisResponse>("/synthesize", request);
  }

  /** Get the current live manifest — what the system knows right now. */
  async manifest(): Promise<Manifest> {
    return this._get<Manifest>("/manifest");
  }

  /**
   * Open a real-time monitor stream.
   *
   * The WebSocket is a monitor, not a command channel. It sends graph
   * health snapshots in response to any message. Use the returned Monitor
   * to start, stop, and receive health updates.
   */
  monitor(onUpdate: (health: unknown) => void): Monitor {
    const wsUrl = this.baseUrl.replace(/^http/, "ws") + "/ws";
    return new Monitor(wsUrl, onUpdate);
  }

  // ── Private ───────────────────────────────────────────────────────────────

  private async _get<T>(path: string): Promise<T> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeout);
    try {
      const res = await fetch(`${this.baseUrl}${path}`, {
        signal: controller.signal,
      });
      if (!res.ok) {
        throw new NstarError(res.status, await res.text());
      }
      return res.json() as Promise<T>;
    } finally {
      clearTimeout(timer);
    }
  }

  private async _post<T>(path: string, body: unknown): Promise<T> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeout);
    try {
      const res = await fetch(`${this.baseUrl}${path}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
        signal: controller.signal,
      });
      if (!res.ok) {
        throw new NstarError(res.status, await res.text());
      }
      return res.json() as Promise<T>;
    } finally {
      clearTimeout(timer);
    }
  }
}

export class NstarError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: string,
  ) {
    super(`N★ API error ${status}: ${body}`);
    this.name = "NstarError";
  }
}

/**
 * Standalone utility: compute delusion_delta.
 *
 * The gap between what you declared and what the evidence supports.
 * Positive = overconfident. Negative = underconfident.
 */
export function delusionDelta(declared: number, evidenceBacked: number): number {
  return declared - evidenceBacked;
}
