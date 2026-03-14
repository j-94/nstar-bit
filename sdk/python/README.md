# nstar-bit Python SDK

Python client for the N★ Bit Epistemic API.

Requires Python 3.9+. Uses `httpx` (async) and `websockets`.

## Install

```bash
pip install nstar-bit
```

## Usage

```python
import asyncio
from nstar_bit import NstarClient, Signal, delusion_delta

async def main():
    async with NstarClient(base_url="http://localhost:8080") as client:

        # Submit a signal — get back a Receipt (immutable proof it happened)
        receipt = await client.turn(Signal(
            text="deployed new auth middleware to production",
            domain="technical",
            source="git",
        ))
        print(f"turn={receipt.turn} ts={receipt.receipt_ts}")

        # Query evidence-gated confidence — always includes delusion_delta
        belief = await client.belief(
            source="auth_middleware",
            relation="improves",
            target="security",
            declared_confidence=0.9,
        )
        print(f"confidence={belief.confidence:.3f} delusion_delta={belief.delusion_delta:.3f}")
        # delusion_delta > 0 means you were overconfident

        # Standalone utility
        delta = delusion_delta(0.9, belief.confidence)

        # Get the live manifest
        manifest = await client.manifest()
        print(f"{manifest.summary.concepts} concepts, {manifest.summary.relations} relations")

asyncio.run(main())
```

### Sync wrappers

For simple scripts without async:

```python
from nstar_bit import NstarClient, Signal

client = NstarClient()
receipt = client.turn_sync(Signal(text="something happened"))
belief = client.belief_sync(relation_id=receipt.new_concepts[0], declared_confidence=0.8)
print(f"delusion_delta={belief.delusion_delta:.3f}")
```

### Real-time monitor

```python
from nstar_bit import NstarClient, GraphHealth

async def main():
    client = NstarClient()

    def on_health(health: GraphHealth):
        print(f"turn={health.turn} violations={health.violations}")

    mon = client.monitor(on_health)
    await mon.start()
    await asyncio.sleep(30)
    await mon.stop()

asyncio.run(main())
```

## The six primitives

| Primitive | Class | Meaning |
|-----------|-------|---------|
| `Signal` | Request dataclass | What you send — raw signal text |
| `Receipt` | Response dataclass | Immutable proof the turn happened |
| `Pattern` | Graph node | A concept that survived evidence gating |
| `Belief` | Query result | Evidence-gated confidence + delusion_delta |
| `Manifest` | State | What the system knows right now |
| `GraphHealth` | Monitor | Real-time health snapshot |

## delusion_delta

Every `Belief` exposes `delusion_delta = declared_confidence - evidence_backed_confidence`.

Positive means overconfident. The system maintains the relationship between your claim and its justification over time.
