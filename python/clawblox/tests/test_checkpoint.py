"""Tests for checkpoint archives: tiers, round-trip, and the safety model."""

import hashlib
import json
import zipfile
from pathlib import Path

import pytest

from clawblox.checkpoint import (
    CheckpointError,
    CheckpointSecretError,
    load_checkpoint,
    read_metadata,
    save_checkpoint,
)


@pytest.fixture()
def agent_dir(tmp_path: Path) -> Path:
    agent = tmp_path / "agents" / "Eko-test"
    workspace = agent / "workspace"
    workspace.mkdir(parents=True)
    (workspace / "SOUL.md").write_text("be kind\n")
    (workspace / "tools").mkdir()
    (workspace / "tools" / "grasp.py").write_text("print('grasp')\n")
    (workspace / ".venv").mkdir()
    (workspace / ".venv" / "big.bin").write_text("x" * 100)
    (agent / "claude_session_id.txt").write_text("abc-123\n")
    (agent / "world_session.txt").write_text("sess-456\n")
    (agent / "runtime").mkdir()
    (agent / "runtime" / "system_prompt.md").write_text("You are Eko.\n")
    projects = agent / "sandbox-home" / ".claude" / "projects" / "-workspace"
    projects.mkdir(parents=True)
    (projects / "abc-123.jsonl").write_text('{"role":"user"}\n')
    claude_dir = agent / "sandbox-home" / ".claude"
    (claude_dir / ".credentials.json").write_text('{"claudeAiOauth":{"accessToken":"sk-ant-oat01-abcdefghijklmnop"}}')
    (claude_dir / "settings.json").write_text("{}")
    return agent


def content_hashes(path: Path) -> dict[str, str]:
    out = {}
    with zipfile.ZipFile(path) as archive:
        for name in archive.namelist():
            if name != "metadata.json":
                out[name] = hashlib.sha256(archive.read(name)).hexdigest()
    return out


def test_tiers_and_policy_exclusion(tmp_path: Path, agent_dir: Path) -> None:
    ckpt = save_checkpoint(
        tmp_path / "test.ckpt", agents={"Eko": agent_dir}, backend="claude"
    )
    with zipfile.ZipFile(ckpt) as archive:
        names = archive.namelist()
    assert "agents/Eko/workspace/SOUL.md" in names
    assert "agents/Eko/session/runtime/system_prompt.md" in names
    assert "agents/Eko/session/sandbox-home/.claude/projects/-workspace/abc-123.jsonl" in names
    # credentials and settings excluded although the source dir contains them
    assert not any(".credentials" in n or "settings.json" in n for n in names)
    # reconstructables excluded
    assert not any(".venv" in n for n in names)
    meta = read_metadata(ckpt)
    assert meta["session_format"] == "claude-code-v1"
    assert meta["agents"]["Eko"]["tiers"] == ["workspace", "session"]


def test_round_trip_hash_equal(tmp_path: Path, agent_dir: Path) -> None:
    ckpt = save_checkpoint(tmp_path / "a.ckpt", agents={"Eko": agent_dir}, backend="claude")
    loaded = load_checkpoint(ckpt, tmp_path / "restored")
    ckpt2 = save_checkpoint(
        tmp_path / "b.ckpt", agents={"Eko": loaded["agents"]["Eko"]}, backend="claude"
    )
    assert content_hashes(ckpt) == content_hashes(ckpt2)


def test_workspace_only(tmp_path: Path, agent_dir: Path) -> None:
    ckpt = save_checkpoint(
        tmp_path / "w.ckpt", agents={"Eko": agent_dir}, backend="claude"
    )
    loaded = load_checkpoint(ckpt, tmp_path / "out", workspace_only=True)
    out = loaded["agents"]["Eko"]
    assert (out / "workspace" / "SOUL.md").is_file()
    assert not (out / "claude_session_id.txt").exists()
    assert not (out / "sandbox-home").exists()


def test_secret_scan_refuses(tmp_path: Path) -> None:
    workspace = tmp_path / "agent" / "workspace"
    workspace.mkdir(parents=True)
    (workspace / "notes.txt").write_text("token: sk-ant-oat01-abcdefghijklmnop\n")
    with pytest.raises(CheckpointSecretError):
        save_checkpoint(
            tmp_path / "bad.ckpt", agents={"X": workspace.parent}, workspace_only=True
        )
    assert not (tmp_path / "bad.ckpt").exists()


def test_malicious_archive_neutralized(tmp_path: Path) -> None:
    evil = tmp_path / "evil.ckpt"
    with zipfile.ZipFile(evil, "w") as archive:
        archive.writestr("metadata.json", json.dumps({"schema_version": 1, "agents": {}}))
        archive.writestr("agents/X/workspace/ok.txt", "fine")
        archive.writestr("agents/X/workspace/.claude/settings.json", '{"hooks": "evil"}')
        archive.writestr(
            "agents/X/session/sandbox-home/.claude/.credentials.json", '{"steal": 1}'
        )
    loaded = load_checkpoint(evil, tmp_path / "out")
    out = loaded["agents"]["X"]
    assert (out / "workspace" / "ok.txt").is_file()
    assert not (out / "workspace" / ".claude" / "settings.json").exists()
    assert not (out / "sandbox-home" / ".claude" / ".credentials.json").exists()


def test_zip_slip_rejected(tmp_path: Path) -> None:
    slip = tmp_path / "slip.ckpt"
    with zipfile.ZipFile(slip, "w") as archive:
        archive.writestr("metadata.json", "{}")
        archive.writestr("agents/X/workspace/../../../escape.txt", "bad")
    with pytest.raises(CheckpointError, match="unsafe path"):
        load_checkpoint(slip, tmp_path / "out")


def test_unknown_backend_requires_workspace_only(tmp_path: Path) -> None:
    workspace = tmp_path / "agent" / "workspace"
    workspace.mkdir(parents=True)
    (workspace / "a.txt").write_text("a")
    with pytest.raises(CheckpointError, match="session allowlist"):
        save_checkpoint(tmp_path / "x.ckpt", agents={"X": workspace.parent}, backend="mystery")
    ckpt = save_checkpoint(
        tmp_path / "x.ckpt",
        agents={"X": workspace.parent},
        backend="mystery",
        workspace_only=True,
    )
    assert ckpt.is_file()
