/**
 * N★ Bit Epistemic API — Core Types
 *
 * Named after the six primitives the system discovered:
 * signal, pattern, receipt, void, manifest, turn
 */

// ── Primitives ────────────────────────────────────────────────────────────────

/** A raw signal submitted to the graph. What happened or was said. */
export interface Signal {
  text: string;
  synthesis?: string;
  domain?: "technical" | "life" | "business" | "learning" | "signal";
  source?: "cli" | "webhook" | "voice" | "omni" | "git" | "calendar";
}

/** Immutable proof that a turn happened. receipt_ts is the anchor. */
export interface Receipt {
  turn: number;
  domain: string;
  concepts_before: number;
  concepts_after: number;
  relations_before: number;
  relations_after: number;
  new_concepts: string[];
  receipt_ts: number; // unix timestamp — proof of occurrence
}

/** A live concept in the epistemic graph. */
export interface Pattern {
  id: string;
  label: string;
  summary: string;
  first_seen_turn: number;
  mention_count: number;
  status: "active" | "candidate" | "retracted";
}

/** The evidence-gated confidence for a belief. Always includes delusion_delta. */
export interface Belief {
  confidence: number;        // what the graph actually supports
  evidence_for: number;
  evidence_against: number;
  support_set: string[];     // IDs of evidence items keeping this belief alive
  status: "active" | "contested" | "retracted";
  relation_id: string;
  delusion_delta: number;    // declared_confidence - confidence (gap between claim and evidence)
}

/** The current live state of the epistemic graph. */
export interface Manifest {
  turn: number;
  updated_at: string;
  summary: {
    concepts: number;
    relations: number;
    evidence: number;
  };
  active_focus: string[];
  universal_primitives: ["signal", "pattern", "receipt", "void", "manifest", "turn"];
  live_concepts: Pattern[];
}

/** Absence of evidence — returned when a belief has no support set. */
export interface Void {
  relation_id: string;
  reason: string;
  confidence: 0;
  delusion_delta: number;
}

/** WebSocket health snapshot from the monitor stream. */
export interface GraphHealth {
  turn: number;
  concepts: number;
  relations: number;
  evidence: number;
  violations: number;
  inflation_score: number;
  timestamp: number;
}

// ── Request / Response types ──────────────────────────────────────────────────

export interface BeliefQuery {
  /** Option A: source + relation + target */
  source?: string;
  relation?: string;
  target?: string;
  /** Option B: direct relation_id */
  relation_id?: string;
  /** The confidence you declared — required to compute delusion_delta */
  declared_confidence?: number;
}

export interface EvidenceDelta {
  relation_id: string;
  direction: "for" | "against";
  weight: number;
  source_uri: string;
  meta?: Record<string, unknown>;
}

export interface EvidenceResponse {
  status: "success";
  relation_id: string;
}

export interface SynthesisRequest {
  signals: string[];
  context?: string;
}

export interface SynthesisResponse {
  status: "synthesis_queued";
  signal_count: number;
  note: string; // synthesis lands in /manifest within ~30s
}

export interface NstarClientOptions {
  baseUrl?: string;
  timeout?: number;
}
