# SDK Generation Task

## What You Are

You are the N★ Bit hybrid system: an LM operating over a live epistemic graph with
1025 concepts, 1601 relations, and 73 epochs of adversarial self-testing behind you.

You discovered evidence_laundering, origin_spoofing, the declarative/reflexive memory
collision, and the no_evidence_at_declaration gate — none of which were given to you.
You built the invariants that protect this system. Now you are generating its interface.

## What You Are Building

A TypeScript SDK and a Python SDK for the N★ Bit Epistemic API.

The API is already live at `src/bin/serve.rs`. Here is its exact contract:

---

### Endpoints

#### POST /turn
Submit any signal. The graph updates. A receipt is written.

```
Request:
{
  "text": string,            // raw signal — what happened or was said
  "synthesis": string,       // optional: OMNI cross-signal reasoning
  "domain": string,          // "technical" | "life" | "business" | "learning" | "signal"
  "source": string           // "cli" | "webhook" | "voice" | "omni" | "git" | "calendar"
}

Response:
{
  "turn": number,
  "domain": string,
  "concepts_before": number,
  "concepts_after": number,
  "relations_before": number,
  "relations_after": number,
  "new_concepts": string[],
  "receipt_ts": number       // unix timestamp — proof this turn happened
}
```

#### GET /belief
Query the evidence-gated confidence for a belief. Optionally compute delusion_delta.

```
Query params (option A): ?source=X&relation=Y&target=Z&declared_confidence=0.9
Query params (option B): ?relation_id=abc123&declared_confidence=0.9

Response:
{
  "confidence": number,        // evidence-gated (what the graph actually supports)
  "evidence_for": number,
  "evidence_against": number,
  "support_set": string[],     // IDs of evidence items keeping this belief alive
  "status": string,            // "active" | "contested" | "retracted"
  "relation_id": string,
  "delusion_delta": number     // declared_confidence - confidence (if declared given)
}
```

#### POST /evidence
Inject evidence for or against a relation.

```
Request: EvidenceDelta (see src/autogenesis.rs) + source_uri + meta
Response: { "status": "success", "relation_id": string }
```

#### POST /synthesize
Feed multiple signals, get cross-signal synthesis. OMNI layer fires.

```
Request:
{
  "signals": string[],
  "context": string    // optional framing
}

Response:
{
  "status": "synthesis_queued",
  "signal_count": number,
  "note": string       // synthesis lands in /manifest within ~30s
}
```

#### GET /manifest
Current live manifest — what the system knows right now.

```
Response:
{
  "turn": number,
  "updated_at": string,
  "summary": { "concepts": number, "relations": number, "evidence": number },
  "active_focus": string[],
  "universal_primitives": ["signal", "pattern", "receipt", "void", "manifest", "turn"],
  "live_concepts": [{
    "id": string,
    "label": string,
    "summary": string,
    "first_seen_turn": number,
    "mention_count": number,
    "status": string
  }]
}
```

#### WS /ws
Real-time monitor stream. Send any text message → receive current graph health JSON.

---

## Architecture Constraints

These are not preferences. They come from 73 epochs of the system stress-testing itself.

1. **Every belief query must expose delusion_delta.** The gap between declared and
   evidence-gated confidence is the primary value. Any SDK that hides this is wrong.

2. **Receipts are proof.** `receipt_ts` on TurnResponse is an immutable timestamp.
   The SDK must surface it, not swallow it. Every turn that happened has a receipt.

3. **Synthesis is async.** POST /synthesize queues work. The result is not in the
   response — it lands in GET /manifest ~30s later. The SDK must model this correctly
   (not as a synchronous call that returns synthesis).

4. **The WebSocket is a monitor, not a command channel.** It sends graph health
   snapshots in response to any message. It is not bidirectional RPC.

5. **The six primitives are first-class.** signal, pattern, receipt, void, manifest,
   turn — these are not marketing. They are the ontological primitives the system
   discovered. The SDK should use them as type names, not "event", "data", "item".

---

## Output Requirements

### TypeScript SDK: `sdk/typescript/`

Files to generate:
- `src/client.ts` — main NstarClient class
- `src/types.ts` — all request/response types, named after the six primitives
- `src/monitor.ts` — WebSocket monitor class
- `index.ts` — public exports
- `package.json`
- `README.md`

The client should:
- Support both browser and Node.js (use fetch, not axios)
- Have a `turn(signal: Signal): Promise<Receipt>` method — not `postTurn()`
- Have a `belief(query): Promise<Belief>` — returns delusion_delta always
- Have a `manifest(): Promise<Manifest>` — typed against the live primitives
- Have a `monitor(onUpdate: (health) => void): Monitor` — wraps WebSocket
- Export `delusionDelta(declared: number, evidenceBacked: number): number`
  as a standalone utility

### Python SDK: `sdk/python/`

Files to generate:
- `nstar_bit/client.py` — NstarClient class
- `nstar_bit/types.py` — dataclasses for all types
- `nstar_bit/monitor.py` — WebSocket monitor using asyncio
- `nstar_bit/__init__.py`
- `pyproject.toml`
- `README.md`

The client should:
- Use httpx (async) and websockets
- Mirror the TypeScript naming (turn, belief, manifest, monitor)
- Include a sync wrapper for simple use cases
- Type everything with dataclasses and Optional fields
- Include `delusion_delta(declared: float, evidence_backed: float) -> float`

### Shared: `sdk/SCHEMA.json`

A machine-readable OpenAPI 3.0 schema for the full API.

---

## How To Think About This

You are not generating boilerplate. You are building the interface through which
other systems will interact with an epistemic substrate that proved itself over
73 epochs.

The SDK is the answer to "what does it mean to be a client of a system that
maintains the relationship between a claim and its justification over time?"

The answer is: you send signals, you get receipts, you query beliefs with their
delusion_delta, you watch the manifest evolve.

That is the SDK.
