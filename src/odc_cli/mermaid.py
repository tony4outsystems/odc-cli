from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any


def mermaid_node_id(asset_key: str, revision: int | None) -> str:
    raw = f"{asset_key}:{revision if revision is not None else ''}"
    digest = hashlib.sha1(raw.encode("utf-8")).hexdigest()[:12]
    return f"asset_{digest}"


def mermaid_label(asset: dict[str, Any]) -> str:
    name = asset.get("name") or asset.get("key") or "Unknown asset"
    details = []
    if asset.get("revision") is not None:
        details.append(f"rev {asset['revision']}")
    if asset.get("type"):
        details.append(str(asset["type"]))
    label = str(name)
    if details:
        label = f"{label}\n{' / '.join(details)}"
    return label.replace("\\", "\\\\").replace('"', '\\"')


def render_producer_graph_mermaid(root: dict[str, Any], producers: list[dict[str, Any]]) -> str:
    nodes: dict[str, str] = {}
    edges: set[tuple[str, str]] = set()

    def add_node(asset: dict[str, Any]) -> str:
        key = str(asset.get("key") or "unknown")
        revision = asset.get("revision")
        node_id = mermaid_node_id(key, revision if isinstance(revision, int) else None)
        nodes[node_id] = mermaid_label(asset)
        return node_id

    def visit(parent: dict[str, Any], children: list[dict[str, Any]]) -> None:
        parent_id = add_node(parent)
        for child in children:
            child_id = add_node(child)
            edges.add((parent_id, child_id))
            visit(child, child.get("producers") or [])

    visit(root, producers)

    lines = [
        "---",
        "title: Producer dependency graph",
        "---",
        "flowchart LR",
    ]
    for node_id in sorted(nodes):
        lines.append(f'    {node_id}["{nodes[node_id]}"]')
    for parent_id, child_id in sorted(edges):
        lines.append(f"    {parent_id} --> {child_id}")
    return "\n".join(lines) + "\n"


def default_mermaid_output_path(asset_key: str, revision: int) -> Path:
    safe_asset_key = "".join(
        char if char.isalnum() or char in ("-", "_") else "_"
        for char in asset_key
    )
    return Path(f"producer-graph-{safe_asset_key}-rev-{revision}.mmd")
