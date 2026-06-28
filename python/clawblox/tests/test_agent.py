from pathlib import Path

from clawblox import Agent


def test_agent_paths_are_explicit() -> None:
    agent = Agent(
        agent="claude",
        name="agent-0",
        dir=Path("runs/g000/agent-0"),
        model="claude-opus-4-7",
        sandbox=True,
    )

    assert agent.agent == "claude"
    assert agent.name == "agent-0"
    assert agent.dir == Path("runs/g000/agent-0")
    assert agent.workspace_dir == Path("runs/g000/agent-0/workspace")
    assert agent.runtime_dir == Path("runs/g000/agent-0/runtime")
    assert agent.log_dir == Path("runs/g000/agent-0/logs")
    assert agent.events_file == Path("runs/g000/agent-0/events.jsonl")
    assert agent.visible_workspace_dir == "/workspace"
    assert hasattr(agent, "start")
    assert hasattr(agent, "send")
    assert hasattr(agent, "stop")


def test_agent_accepts_explicit_binary() -> None:
    agent = Agent(
        agent="codex",
        name="agent-0",
        dir=Path("runs/g000/agent-0"),
        model="gpt-5.5",
        binary=Path("/tmp/codex-wrapper"),
    )

    assert agent.agent == "codex"
