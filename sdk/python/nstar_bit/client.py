"""
N★ Bit Epistemic API Client

Core contract:
    await client.turn(signal)       → Receipt  (immutable proof the turn happened)
    await client.belief(query)      → Belief   (evidence-gated confidence + delusion_delta)
    await client.manifest()         → Manifest (what the system knows right now)
    client.monitor(on_update)       → Monitor  (real-time graph health stream)

Sync wrappers:
    client.turn_sync(signal)
    client.belief_sync(query)
    client.manifest_sync()
"""
from __future__ import annotations

import asyncio
from typing import Callable, Optional

import httpx

from .types import (
    Belief,
    EvidenceDelta,
    EvidenceResponse,
    Manifest,
    Receipt,
    Signal,
    SynthesisRequest,
    SynthesisResponse,
)
from .monitor import Monitor


class NstarClient:
    """
    Async-first client for the N★ Bit Epistemic API.

    Use as an async context manager for connection pooling:

        async with NstarClient() as client:
            receipt = await client.turn(Signal(text="something happened"))

    Or use the sync wrappers for simple scripting:

        client = NstarClient()
        receipt = client.turn_sync(Signal(text="something happened"))
    """

    def __init__(
        self,
        base_url: str = "http://localhost:8080",
        timeout: float = 30.0,
    ) -> None:
        self._base_url = base_url.rstrip("/")
        self._timeout = timeout
        self._client: Optional[httpx.AsyncClient] = None

    async def __aenter__(self) -> "NstarClient":
        self._client = httpx.AsyncClient(
            base_url=self._base_url,
            timeout=self._timeout,
        )
        return self

    async def __aexit__(self, *_: object) -> None:
        if self._client:
            await self._client.aclose()
            self._client = None

    def _get_client(self) -> httpx.AsyncClient:
        if self._client is None:
            # Auto-create for one-off calls (not recommended for high throughput)
            self._client = httpx.AsyncClient(
                base_url=self._base_url,
                timeout=self._timeout,
            )
        return self._client

    # ── Async API ────────────────────────────────────────────────────────────

    async def turn(self, signal: Signal) -> Receipt:
        """Submit a signal. The graph updates. Returns a Receipt."""
        client = self._get_client()
        res = await client.post("/turn", json=signal.to_dict())
        _raise_for_status(res)
        return Receipt.from_dict(res.json())

    async def belief(
        self,
        *,
        relation_id: Optional[str] = None,
        source: Optional[str] = None,
        relation: Optional[str] = None,
        target: Optional[str] = None,
        declared_confidence: Optional[float] = None,
    ) -> Belief:
        """
        Query evidence-gated confidence for a belief.

        Always returns delusion_delta — the gap between declared and actual
        confidence. If declared_confidence is not provided, delusion_delta = 0.

        Usage:
            # By relation_id
            b = await client.belief(relation_id="abc123", declared_confidence=0.9)
            print(b.delusion_delta)  # how wrong you were

            # By source / relation / target
            b = await client.belief(source="A", relation="causes", target="B")
        """
        params: dict = {}
        if relation_id:
            params["relation_id"] = relation_id
        else:
            if source:
                params["source"] = source
            if relation:
                params["relation"] = relation
            if target:
                params["target"] = target
        if declared_confidence is not None:
            params["declared_confidence"] = str(declared_confidence)

        client = self._get_client()
        res = await client.get("/belief", params=params)
        _raise_for_status(res)
        return Belief.from_dict(res.json())

    async def evidence(self, delta: EvidenceDelta) -> EvidenceResponse:
        """Inject evidence for or against a relation."""
        client = self._get_client()
        res = await client.post("/evidence", json=delta.to_dict())
        _raise_for_status(res)
        return EvidenceResponse.from_dict(res.json())

    async def synthesize(self, request: SynthesisRequest) -> SynthesisResponse:
        """
        Feed multiple signals for cross-signal synthesis.

        Synthesis is async — result lands in /manifest within ~30s.
        Returns immediately with status="synthesis_queued".
        """
        client = self._get_client()
        res = await client.post("/synthesize", json=request.to_dict())
        _raise_for_status(res)
        return SynthesisResponse.from_dict(res.json())

    async def manifest(self) -> Manifest:
        """Get the current live manifest — what the system knows right now."""
        client = self._get_client()
        res = await client.get("/manifest")
        _raise_for_status(res)
        return Manifest.from_dict(res.json())

    def monitor(
        self,
        on_update: Callable,
        ping_interval: float = 5.0,
    ) -> Monitor:
        """
        Open a real-time monitor stream.

        The WebSocket sends graph health snapshots in response to any message.
        The monitor pings every `ping_interval` seconds to keep receiving updates.

        Usage:
            def on_health(health):
                print(f"turn={health.turn} concepts={health.concepts}")

            mon = client.monitor(on_health)
            await mon.start()
            # ... do work ...
            await mon.stop()
        """
        ws_url = self._base_url.replace("http://", "ws://").replace("https://", "wss://") + "/ws"
        return Monitor(ws_url, on_update, ping_interval=ping_interval)

    # ── Sync wrappers ─────────────────────────────────────────────────────────

    def turn_sync(self, signal: Signal) -> Receipt:
        return asyncio.run(self.turn(signal))

    def belief_sync(
        self,
        *,
        relation_id: Optional[str] = None,
        source: Optional[str] = None,
        relation: Optional[str] = None,
        target: Optional[str] = None,
        declared_confidence: Optional[float] = None,
    ) -> Belief:
        return asyncio.run(
            self.belief(
                relation_id=relation_id,
                source=source,
                relation=relation,
                target=target,
                declared_confidence=declared_confidence,
            )
        )

    def manifest_sync(self) -> Manifest:
        return asyncio.run(self.manifest())


class NstarError(Exception):
    def __init__(self, status: int, body: str) -> None:
        super().__init__(f"N★ API error {status}: {body}")
        self.status = status
        self.body = body


def _raise_for_status(res: httpx.Response) -> None:
    if res.is_error:
        raise NstarError(res.status_code, res.text)


def delusion_delta(declared: float, evidence_backed: float) -> float:
    """
    Compute delusion_delta.

    The gap between what you declared and what the evidence supports.
    Positive = overconfident. Negative = underconfident.
    """
    return declared - evidence_backed
