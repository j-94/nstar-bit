"""
N★ Bit Epistemic API — Core Types

Named after the six primitives the system discovered:
signal, pattern, receipt, void, manifest, turn
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Optional


# ── Primitives ────────────────────────────────────────────────────────────────

@dataclass
class Signal:
    """A raw signal submitted to the graph. What happened or was said."""
    text: str
    synthesis: Optional[str] = None
    domain: Optional[str] = None   # "technical" | "life" | "business" | "learning" | "signal"
    source: Optional[str] = None   # "cli" | "webhook" | "voice" | "omni" | "git" | "calendar"

    def to_dict(self) -> dict:
        d: dict = {"text": self.text}
        if self.synthesis is not None:
            d["synthesis"] = self.synthesis
        if self.domain is not None:
            d["domain"] = self.domain
        if self.source is not None:
            d["source"] = self.source
        return d


@dataclass
class Receipt:
    """Immutable proof that a turn happened. receipt_ts is the anchor."""
    turn: int
    domain: str
    concepts_before: int
    concepts_after: int
    relations_before: int
    relations_after: int
    new_concepts: List[str]
    receipt_ts: int  # unix timestamp

    @classmethod
    def from_dict(cls, d: dict) -> "Receipt":
        return cls(
            turn=d["turn"],
            domain=d["domain"],
            concepts_before=d["concepts_before"],
            concepts_after=d["concepts_after"],
            relations_before=d["relations_before"],
            relations_after=d["relations_after"],
            new_concepts=d.get("new_concepts", []),
            receipt_ts=d["receipt_ts"],
        )


@dataclass
class Pattern:
    """A live concept in the epistemic graph."""
    id: str
    label: str
    summary: str
    first_seen_turn: int
    mention_count: int
    status: str  # "active" | "candidate" | "retracted"

    @classmethod
    def from_dict(cls, d: dict) -> "Pattern":
        return cls(
            id=d["id"],
            label=d["label"],
            summary=d.get("summary", ""),
            first_seen_turn=d.get("first_seen_turn", 0),
            mention_count=d.get("mention_count", 0),
            status=d.get("status", "active"),
        )


@dataclass
class Belief:
    """
    Evidence-gated confidence for a belief.

    Always includes delusion_delta — the gap between declared and actual
    confidence. Positive = overconfident. This is the primary value.
    """
    confidence: float
    evidence_for: int
    evidence_against: int
    support_set: List[str]
    status: str  # "active" | "contested" | "retracted"
    relation_id: str
    delusion_delta: float  # declared_confidence - confidence

    @classmethod
    def from_dict(cls, d: dict) -> "Belief":
        return cls(
            confidence=d["confidence"],
            evidence_for=d.get("evidence_for", 0),
            evidence_against=d.get("evidence_against", 0),
            support_set=d.get("support_set", []),
            status=d.get("status", "active"),
            relation_id=d["relation_id"],
            delusion_delta=d.get("delusion_delta", 0.0),
        )


@dataclass
class ManifestSummary:
    concepts: int
    relations: int
    evidence: int


@dataclass
class Manifest:
    """The current live state of the epistemic graph."""
    turn: int
    updated_at: str
    summary: ManifestSummary
    active_focus: List[str]
    universal_primitives: List[str]
    live_concepts: List[Pattern]

    @classmethod
    def from_dict(cls, d: dict) -> "Manifest":
        s = d.get("summary", {})
        return cls(
            turn=d["turn"],
            updated_at=d.get("updated_at", ""),
            summary=ManifestSummary(
                concepts=s.get("concepts", 0),
                relations=s.get("relations", 0),
                evidence=s.get("evidence", 0),
            ),
            active_focus=d.get("active_focus", []),
            universal_primitives=d.get("universal_primitives", []),
            live_concepts=[Pattern.from_dict(c) for c in d.get("live_concepts", [])],
        )


@dataclass
class EvidenceDelta:
    """Evidence injection request."""
    relation_id: str
    direction: str  # "for" | "against"
    weight: float
    source_uri: str
    meta: Optional[dict] = None

    def to_dict(self) -> dict:
        d: dict = {
            "relation_id": self.relation_id,
            "direction": self.direction,
            "weight": self.weight,
            "source_uri": self.source_uri,
        }
        if self.meta:
            d["meta"] = self.meta
        return d


@dataclass
class EvidenceResponse:
    status: str
    relation_id: str

    @classmethod
    def from_dict(cls, d: dict) -> "EvidenceResponse":
        return cls(status=d["status"], relation_id=d["relation_id"])


@dataclass
class SynthesisRequest:
    signals: List[str]
    context: Optional[str] = None

    def to_dict(self) -> dict:
        d: dict = {"signals": self.signals}
        if self.context is not None:
            d["context"] = self.context
        return d


@dataclass
class SynthesisResponse:
    """Synthesis is async — result lands in /manifest within ~30s."""
    status: str
    signal_count: int
    note: str

    @classmethod
    def from_dict(cls, d: dict) -> "SynthesisResponse":
        return cls(
            status=d["status"],
            signal_count=d["signal_count"],
            note=d.get("note", ""),
        )


@dataclass
class GraphHealth:
    """WebSocket health snapshot from the monitor stream."""
    turn: int
    concepts: int
    relations: int
    evidence: int
    violations: int
    inflation_score: float
    timestamp: int

    @classmethod
    def from_dict(cls, d: dict) -> "GraphHealth":
        return cls(
            turn=d.get("turn", 0),
            concepts=d.get("concepts", 0),
            relations=d.get("relations", 0),
            evidence=d.get("evidence", 0),
            violations=d.get("violations", 0),
            inflation_score=d.get("inflation_score", 0.0),
            timestamp=d.get("timestamp", 0),
        )
