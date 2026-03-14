# nstar-bit TypeScript SDK

TypeScript/JavaScript client for the N★ Bit Epistemic API.

Works in both browser and Node.js (uses `fetch`).

## Install

```bash
npm install nstar-bit
```

## Usage

```typescript
import { NstarClient, Signal, delusionDelta } from "nstar-bit";

const client = new NstarClient({ baseUrl: "http://localhost:8080" });

// Submit a signal — get back a Receipt (immutable proof it happened)
const receipt = await client.turn({
  text: "deployed new auth middleware to production",
  domain: "technical",
  source: "git",
});
console.log(`turn=${receipt.turn} ts=${receipt.receipt_ts}`);

// Query evidence-gated confidence — always includes delusion_delta
const belief = await client.belief({
  source: "auth_middleware",
  relation: "improves",
  target: "security",
  declared_confidence: 0.9,
});
console.log(`confidence=${belief.confidence} delusion_delta=${belief.delusion_delta}`);
// delusion_delta > 0 means you were overconfident

// Standalone utility
const delta = delusionDelta(0.9, belief.confidence);

// Get the live manifest — what the system knows right now
const manifest = await client.manifest();
console.log(`${manifest.summary.concepts} concepts, ${manifest.summary.relations} relations`);

// Real-time health monitor
const monitor = client.monitor((health) => {
  console.log(`turn=${health.turn} violations=${health.violations}`);
});
monitor.start();
// ... later ...
monitor.stop();
```

## The six primitives

The API is built on six primitives the system discovered through 86+ epochs of adversarial testing:

| Primitive | Type | Meaning |
|-----------|------|---------|
| `Signal` | Request | What you send — raw signal text |
| `Receipt` | Response | Immutable proof the turn happened |
| `Pattern` | Graph node | A concept that survived evidence gating |
| `Belief` | Query result | Evidence-gated confidence + delusion_delta |
| `Manifest` | State | What the system knows right now |
| `Void` | Absence | A belief with no evidence support |

## delusion_delta

Every `Belief` exposes `delusion_delta = declared_confidence - evidence_backed_confidence`.

This is the primary value the API delivers. Positive means you were overconfident. The system maintains the relationship between your claim and its justification over time.

## Synthesis is async

`POST /synthesize` queues work. The result is **not** in the response — it lands in `GET /manifest` within ~30s. The SDK models this correctly:

```typescript
const queued = await client.synthesize({ signals: ["A", "B", "C"] });
// queued.status === "synthesis_queued"
// ... wait ~30s ...
const manifest = await client.manifest(); // synthesis result is here
```
