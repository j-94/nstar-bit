"""
N★ Bit Python SDK

    from nstar_bit import NstarClient, Signal, delusion_delta

    async with NstarClient() as client:
        receipt = await client.turn(Signal(text="something happened"))
        belief = await client.belief(relation_id=receipt.new_concepts[0],
                                     declared_confidence=0.8)
        print(f"delusion_delta={belief.delusion_delta:.3f}")
"""
from .client import NstarClient, NstarError, delusion_delta
from .monitor import Monitor
from .types import (
    Belief,
    EvidenceDelta,
    EvidenceResponse,
    GraphHealth,
    Manifest,
    ManifestSummary,
    Pattern,
    Receipt,
    Signal,
    SynthesisRequest,
    SynthesisResponse,
)

__all__ = [
    # Client
    "NstarClient",
    "NstarError",
    "delusion_delta",
    # Monitor
    "Monitor",
    # Types (primitives)
    "Signal",
    "Receipt",
    "Pattern",
    "Belief",
    "Manifest",
    "ManifestSummary",
    "GraphHealth",
    # Request/Response
    "EvidenceDelta",
    "EvidenceResponse",
    "SynthesisRequest",
    "SynthesisResponse",
]
