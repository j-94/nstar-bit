#!/usr/bin/env python3
"""Build README_PATH_REPO_SUMMARIES.md from indexed README path lists.

This generator reads README paths listed in:
  - DESKTOP_READMES_RECURSIVE.md
  - README_CORPUS_SYNOPSIS.md

It then produces one concise summary per README path using the README's
own content (no parent fallback summaries).
"""

from __future__ import annotations

import argparse
import re
from dataclasses import dataclass
from pathlib import Path


PATH_LINE_RE = re.compile(r"^- `(/Users/jobs/.+/(?:README|Readme|readme)\.md)`\s*$")
HEADING_RE = re.compile(r"^#{1,6}\s+")
LIST_RE = re.compile(r"^\s*(?:[-*+]|\d+\.)\s+")
TABLE_RULE_RE = re.compile(r"^[:\-\| ]+$")
SENTENCE_SPLIT_RE = re.compile(r"(?<=[.!?])\s+")


@dataclass
class Block:
    kind: str
    heading: str
    text: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Repository root containing source index files.",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Output markdown path (defaults to ROOT/README_PATH_REPO_SUMMARIES.md).",
    )
    return parser.parse_args()


def normalize_space(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def clean_inline(text: str) -> str:
    text = text.strip()
    if not text:
        return ""

    # Drop pure URLs and badges/images.
    if re.fullmatch(r"https?://\S+", text):
        return ""
    if text.startswith("![") or text.startswith("[![") or text.startswith("<img") or text.startswith("<picture"):
        return ""

    text = re.sub(r"!\[[^\]]*\]\([^\)]*\)", "", text)  # images
    text = re.sub(r"\[([^\]]+)\]\([^\)]+\)", r"\1", text)  # links -> label
    text = text.replace("`", "")
    text = text.replace("**", "")
    text = text.replace("__", "")
    text = re.sub(r"[*_]{1,2}", "", text)
    text = re.sub(r"<[^>]+>", " ", text)
    return normalize_space(text)


def extract_paths(source_files: list[Path]) -> list[Path]:
    paths: list[Path] = []
    seen: set[str] = set()
    for source in source_files:
        if not source.exists():
            continue
        for line in source.read_text(encoding="utf-8", errors="ignore").splitlines():
            match = PATH_LINE_RE.match(line)
            if not match:
                continue
            path_str = match.group(1)
            if path_str in seen:
                continue
            seen.add(path_str)
            paths.append(Path(path_str))
    return paths


def detect_repo_root(readme_path: Path) -> Path:
    current = readme_path.parent
    stop = Path("/Users/jobs")
    while True:
        if (current / ".git").exists():
            return current
        if current == stop or current.parent == current:
            return readme_path.parent
        current = current.parent


def markdown_blocks(markdown: str) -> tuple[str, list[Block]]:
    title = ""
    active_heading = ""
    blocks: list[Block] = []

    in_code = False
    current_kind = ""
    current_heading = ""
    current_lines: list[str] = []

    def flush() -> None:
        nonlocal current_kind, current_heading, current_lines
        if not current_kind or not current_lines:
            current_kind = ""
            current_heading = ""
            current_lines = []
            return
        text = normalize_space(" ".join(current_lines))
        if text:
            blocks.append(Block(kind=current_kind, heading=current_heading, text=text))
        current_kind = ""
        current_heading = ""
        current_lines = []

    for raw in markdown.splitlines():
        line = raw.rstrip("\n")
        stripped = line.strip()

        if stripped.startswith("```"):
            flush()
            in_code = not in_code
            continue
        if in_code:
            continue

        if not stripped:
            flush()
            continue

        if HEADING_RE.match(stripped):
            flush()
            heading_text = clean_inline(re.sub(r"^#{1,6}\s+", "", stripped))
            if heading_text:
                if not title:
                    title = heading_text
                active_heading = heading_text
                blocks.append(Block(kind="heading", heading=active_heading, text=heading_text))
            continue

        if stripped.startswith("|") or TABLE_RULE_RE.fullmatch(stripped):
            flush()
            continue

        kind = "paragraph"
        content = stripped
        if LIST_RE.match(stripped):
            kind = "list"
            content = LIST_RE.sub("", stripped, count=1)

        content = clean_inline(content)
        if not content:
            continue

        if current_kind == kind and current_heading == active_heading:
            current_lines.append(content)
        else:
            flush()
            current_kind = kind
            current_heading = active_heading
            current_lines = [content]

    flush()
    return title, blocks


def command_density(text: str) -> float:
    tokens = text.split()
    if not tokens:
        return 1.0
    noisy = 0
    for token in tokens:
        t = token.strip(".,:;()[]{}")
        if not t:
            continue
        if t.startswith("--"):
            noisy += 1
            continue
        if "/" in t or "\\" in t:
            noisy += 1
            continue
        if re.fullmatch(r"[A-Z0-9_]{4,}", t):
            noisy += 1
            continue
        if re.fullmatch(r"https?://\S+", t):
            noisy += 1
            continue
    return noisy / max(len(tokens), 1)


def sentence_score(sentence: str) -> int:
    score = 0
    lower = sentence.lower()
    if re.search(r"\b(is|are|provides?|builds?|implements?|enables?|allows?|offers?|contains?|focuses?|aims?)\b", lower):
        score += 3
    if re.search(r"\b(project|system|engine|framework|tool|platform|library|repo|repository)\b", lower):
        score += 2
    if 55 <= len(sentence) <= 190:
        score += 2
    if command_density(sentence) < 0.16:
        score += 2
    if lower.startswith(("run ", "clone ", "install ", "usage", "quick start", "api endpoints")):
        score -= 4
    if "http://" in lower or "https://" in lower:
        score -= 3
    return score


def best_sentence(block_text: str) -> str:
    pieces = SENTENCE_SPLIT_RE.split(block_text)
    candidates: list[str] = []
    for piece in pieces:
        s = normalize_space(piece.strip(" ."))
        if len(s) < 35:
            continue
        if sum(ch.isalpha() for ch in s) < 24:
            continue
        if command_density(s) > 0.45:
            continue
        candidates.append(s)

    if not candidates:
        # Fallback: split by colon and keep the first substantive phrase.
        colon_parts = [normalize_space(x) for x in block_text.split(":")]
        for part in colon_parts:
            if len(part) >= 35 and command_density(part) < 0.4:
                return part
        return normalize_space(block_text)[:220]

    ranked = sorted(candidates, key=lambda s: (sentence_score(s), len(s)), reverse=True)
    top = ranked[0]
    if len(top) < 90 and len(ranked) > 1:
        second = ranked[1]
        if second != top and len(normalize_space(f"{top}. {second}")) <= 230:
            top = normalize_space(f"{top}. {second}")
    return top


def select_summary_block(blocks: list[Block]) -> Block | None:
    if not blocks:
        return None

    heading_priority = ("overview", "about", "introduction", "summary", "purpose", "what", "why")
    candidates: list[tuple[int, Block]] = []
    for block in blocks:
        if block.kind not in {"paragraph", "list"}:
            continue
        if len(block.text) < 20:
            continue

        score = 0
        if block.kind == "paragraph":
            score += 5
        else:
            score += 1

        heading_lower = block.heading.lower()
        if any(key in heading_lower for key in heading_priority):
            score += 5

        if 60 <= len(block.text) <= 300:
            score += 3
        if command_density(block.text) < 0.20:
            score += 3
        if command_density(block.text) > 0.35:
            score -= 5
        if re.search(r"\b(install|usage|quick start|api endpoint|command|run)\b", block.text, re.IGNORECASE):
            score -= 2

        candidates.append((score, block))

    if not candidates:
        return None
    candidates.sort(key=lambda item: (item[0], len(item[1].text)), reverse=True)
    return candidates[0][1]


def summarize_readme(path: Path) -> str:
    try:
        raw = path.read_text(encoding="utf-8", errors="ignore")
    except Exception:
        return "README could not be read."

    title, blocks = markdown_blocks(raw)
    selected = select_summary_block(blocks)

    if selected is None:
        if title:
            return f"{title}: README has minimal descriptive prose."
        return "README has minimal descriptive prose."

    core = best_sentence(selected.text)
    core = normalize_space(core).rstrip(".")
    if not core:
        if title:
            return f"{title}: README has minimal descriptive prose."
        return "README has minimal descriptive prose."

    if title and title.lower() not in core.lower() and title.lower() not in {"readme", "overview"}:
        summary = f"{title}: {core}."
    else:
        summary = f"{core}."

    if len(summary) > 260:
        summary = summary[:257].rstrip() + "..."
    return summary


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    out = args.out.resolve() if args.out else (root / "README_PATH_REPO_SUMMARIES.md")

    sources = [
        root / "DESKTOP_READMES_RECURSIVE.md",
        root / "README_CORPUS_SYNOPSIS.md",
    ]
    paths = extract_paths(sources)

    entries = []
    missing = 0
    repo_roots: set[str] = set()
    for readme_path in paths:
        exists = readme_path.exists()
        if not exists:
            missing += 1
        repo_root = detect_repo_root(readme_path) if exists else readme_path.parent
        repo_roots.add(str(repo_root))
        summary = summarize_readme(readme_path) if exists else "Path missing at generation time."
        entries.append((str(readme_path), str(repo_root), summary, exists))

    lines: list[str] = []
    lines.append("# README Path -> Repo Summary Map")
    lines.append("")
    lines.append("This file is generated from actual README contents (path-by-path), not parent fallback summaries.")
    lines.append("")
    lines.append("Generated from:")
    for source in sources:
        lines.append(f"- `{source}`")
    lines.append("")
    lines.append(f"- Unique README paths indexed: `{len(entries)}`")
    lines.append(f"- Distinct detected repo roots: `{len(repo_roots)}`")
    lines.append(f"- Missing paths at generation time: `{missing}`")
    lines.append("")

    for path_str, repo_root, summary, exists in entries:
        lines.append(f"- `{path_str}`")
        lines.append(f"  Repo root: `{repo_root}`")
        lines.append(f"  Repo summary: {summary}")
        if not exists:
            lines.append("  Note: path missing at generation time.")
        lines.append("")

    out.write_text("\n".join(lines), encoding="utf-8")
    print(f"Wrote {out}")
    print(f"Entries: {len(entries)} | Repo roots: {len(repo_roots)} | Missing: {missing}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
