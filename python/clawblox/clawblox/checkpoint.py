"""Checkpoint archives: portable save/load of world and agent state.

A checkpoint is a single relocatable zip with three tiers:

- world tier:     ``world.snapshot`` — opaque state document produced by the
  world's ``GET /snapshot`` endpoint (see ``World.save``).
- workspace tier: ``agents/<name>/workspace/**`` — agent-authored files.
  Backend-agnostic and safe to share by construction.
- session tier:   ``agents/<name>/session/**`` — provider continuation state
  (conversation transcripts), selected by a per-backend allowlist. Needed only
  to resume the exact conversation; omitted with ``workspace_only=True``.

Policy and credentials (settings, permission grants, token files) are never
written to an archive and never extracted from one: state loads, policy
doesn't. A secret scan runs over every file at save time and refuses to
produce an archive containing credential-shaped content.
"""

from __future__ import annotations

import fnmatch
import json
import re
import shutil
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

CHECKPOINT_SCHEMA_VERSION = 1

# Provider continuation state, selected per backend. Allowlists fail closed:
# anything not matched here stays out of the archive.
SESSION_ALLOWLISTS: dict[str, dict[str, Any]] = {
    "claude": {
        "format": "claude-code-v1",
        "include": [
            "claude_session_id.txt",
            "world_session.txt",
            "runtime/system_prompt.md",
            "sandbox-home/.claude/projects/**",
            "sandbox-home/.claude/history.jsonl",
            "sandbox-home/.claude/file-history/**",
            "sandbox-home/.claude/todos/**",
        ],
    },
    "codex": {
        "format": "codex-v1",
        "include": [
            "codex_session_id.txt",
            "world_session.txt",
            "runtime/system_prompt.md",
            "sandbox-home/.codex/sessions/**",
            "sandbox-home/.codex/history.jsonl",
        ],
    },
}

# Reconstructable artifacts excluded from the workspace tier.
WORKSPACE_EXCLUDE_NAMES = {".venv", "__pycache__", ".pytest_cache", "node_modules"}
WORKSPACE_EXCLUDE_PATTERNS = ["*.pyc"]

# Capability-granting files: never archived, never extracted, in any tier.
POLICY_BASENAMES = {
    ".credentials.json",
    ".claude.json",
    "settings.json",
    "settings.local.json",
    "policy-limits.json",
    "remote-settings.json",
}

SECRET_PATTERNS = [
    re.compile(rb"sk-ant-[A-Za-z0-9_-]{8,}"),
    re.compile(rb"sk-[A-Za-z0-9]{20,}"),
    re.compile(rb'"(?:accessToken|refreshToken)"\s*:\s*"[^"]{8,}"'),
    re.compile(rb"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    re.compile(rb"(?:CLAUDE_CODE_OAUTH_TOKEN|ANTHROPIC_API_KEY|ANTHROPIC_OAUTH_TOKEN)\s*="),
]
SECRET_SCAN_MAX_BYTES = 16 * 1024 * 1024


class CheckpointError(RuntimeError):
    pass


class CheckpointSecretError(CheckpointError):
    def __init__(self, findings: list[str]) -> None:
        self.findings = findings
        listing = "\n".join(f"  - {item}" for item in findings)
        super().__init__(
            "refusing to write checkpoint: credential-shaped content found in\n"
            f"{listing}\nRemove the secrets (or the files) and retry."
        )


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def scan_for_secrets(path: Path, archive_name: str, findings: list[str]) -> None:
    try:
        size = path.stat().st_size
    except OSError:
        return
    if size == 0 or size > SECRET_SCAN_MAX_BYTES:
        return
    data = path.read_bytes()
    for pattern in SECRET_PATTERNS:
        if pattern.search(data):
            findings.append(f"{archive_name} (matches {pattern.pattern!r})")
            return


def _workspace_files(workspace: Path):
    for path in sorted(workspace.rglob("*")):
        if not path.is_file() or path.is_symlink():
            continue
        rel = path.relative_to(workspace)
        if any(part in WORKSPACE_EXCLUDE_NAMES for part in rel.parts):
            continue
        if any(fnmatch.fnmatch(rel.name, pattern) for pattern in WORKSPACE_EXCLUDE_PATTERNS):
            continue
        if rel.name in POLICY_BASENAMES:
            continue
        yield rel, path


def _session_files(agent_dir: Path, include: list[str]):
    for pattern in include:
        if pattern.endswith("/**"):
            base = agent_dir / pattern[: -len("/**")]
            if not base.is_dir():
                continue
            for path in sorted(base.rglob("*")):
                if path.is_file() and not path.is_symlink():
                    rel = path.relative_to(agent_dir)
                    if rel.name not in POLICY_BASENAMES:
                        yield rel, path
        else:
            path = agent_dir / pattern
            if path.is_file() and path.name not in POLICY_BASENAMES:
                yield Path(pattern), path


def save_checkpoint(
    out_path: Path | str,
    *,
    world_snapshot: Path | str | None = None,
    agents: dict[str, Path | str] | None = None,
    backend: str = "claude",
    workspace_only: bool = False,
    metadata: dict[str, Any] | None = None,
) -> Path:
    """Write a single-file checkpoint archive.

    ``agents`` maps agent name to its agent directory (the directory that
    contains ``workspace/``). ``world_snapshot`` is a snapshot file produced
    by ``World.save``. Extra ``metadata`` is merged into ``metadata.json``.
    """
    out_path = Path(out_path)
    if out_path.suffix != ".ckpt":
        out_path = out_path.with_suffix(out_path.suffix + ".ckpt")
    agents = {name: Path(path) for name, path in (agents or {}).items()}
    session_spec = SESSION_ALLOWLISTS.get(backend)
    if not workspace_only and session_spec is None:
        raise CheckpointError(
            f"no session allowlist for backend {backend!r}; "
            "use workspace_only=True or add one to SESSION_ALLOWLISTS"
        )

    entries: list[tuple[str, Path]] = []
    if world_snapshot is not None:
        snapshot_path = Path(world_snapshot)
        if not snapshot_path.is_file():
            raise CheckpointError(f"world snapshot not found: {snapshot_path}")
        entries.append(("world.snapshot", snapshot_path))

    agent_meta: dict[str, Any] = {}
    for name, agent_dir in agents.items():
        workspace = agent_dir / "workspace"
        if not workspace.is_dir():
            raise CheckpointError(f"agent workspace not found: {workspace}")
        tiers = ["workspace"]
        for rel, path in _workspace_files(workspace):
            entries.append((f"agents/{name}/workspace/{rel.as_posix()}", path))
        if not workspace_only:
            assert session_spec is not None
            session_entries = [
                (f"agents/{name}/session/{rel.as_posix()}", path)
                for rel, path in _session_files(agent_dir, session_spec["include"])
            ]
            if session_entries:
                tiers.append("session")
                entries.extend(session_entries)
        agent_meta[name] = {"backend": backend, "tiers": tiers}

    findings: list[str] = []
    for archive_name, path in entries:
        scan_for_secrets(path, archive_name, findings)
    if findings:
        raise CheckpointSecretError(findings)

    doc = {
        "schema_version": CHECKPOINT_SCHEMA_VERSION,
        "created_at": utc_stamp(),
        "workspace_only": workspace_only,
        "session_format": None if workspace_only else session_spec["format"],
        "agents": agent_meta,
        **(metadata or {}),
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = out_path.with_suffix(out_path.suffix + ".tmp")
    with zipfile.ZipFile(tmp_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("metadata.json", json.dumps(doc, indent=2) + "\n")
        for archive_name, path in entries:
            archive.write(path, archive_name)
    tmp_path.rename(out_path)
    return out_path


def read_metadata(ckpt_path: Path | str) -> dict[str, Any]:
    with zipfile.ZipFile(Path(ckpt_path)) as archive:
        return json.loads(archive.read("metadata.json"))


def _safe_relpath(name: str, prefix: str) -> Path | None:
    if not name.startswith(prefix):
        return None
    rel = Path(name[len(prefix):])
    if rel.is_absolute() or ".." in rel.parts or not rel.parts:
        raise CheckpointError(f"unsafe path in checkpoint archive: {name!r}")
    if rel.name in POLICY_BASENAMES:
        return None
    return rel


def load_checkpoint(
    ckpt_path: Path | str,
    dest_dir: Path | str,
    *,
    workspace_only: bool = False,
) -> dict[str, Any]:
    """Materialize a checkpoint archive into ``dest_dir``.

    Returns ``{"metadata", "world_snapshot", "agents": {name: agent_dir}}``.
    Policy files are never extracted, even if present in the archive.
    """
    ckpt_path = Path(ckpt_path)
    dest_dir = Path(dest_dir)
    dest_dir.mkdir(parents=True, exist_ok=True)
    result: dict[str, Any] = {"world_snapshot": None, "agents": {}}
    with zipfile.ZipFile(ckpt_path) as archive:
        metadata = json.loads(archive.read("metadata.json"))
        result["metadata"] = metadata
        for info in archive.infolist():
            if info.is_dir() or info.filename == "metadata.json":
                continue
            if info.filename == "world.snapshot":
                target = dest_dir / "world.snapshot"
                target.parent.mkdir(parents=True, exist_ok=True)
                with archive.open(info) as src, open(target, "wb") as dst:
                    shutil.copyfileobj(src, dst)
                result["world_snapshot"] = target
                continue
            rel = _safe_relpath(info.filename, "agents/")
            if rel is None:
                continue
            parts = rel.parts
            if len(parts) < 3 or parts[1] not in {"workspace", "session"}:
                continue
            name, tier = parts[0], parts[1]
            if tier == "session" and workspace_only:
                continue
            agent_dir = dest_dir / "agents" / name
            if tier == "workspace":
                target = agent_dir / "workspace" / Path(*parts[2:])
            else:
                target = agent_dir / Path(*parts[2:])
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(info) as src, open(target, "wb") as dst:
                shutil.copyfileobj(src, dst)
            result["agents"].setdefault(name, agent_dir)
    return result
