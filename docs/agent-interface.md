---
title: Agent interface
description: How Clawblox starts and resumes an agent process.
---

An agent lets Clawblox start and resume an AI process, such as Claude or Codex.

Clawblox gives the agent paths and prompts. The agent starts or continues its own process and writes state back to the paths it was given.

## What Clawblox Gives You

Your agent receives paths like these:

```text
workspace_dir       where the agent works
prompt_file         system/developer prompt
session_id_file     where to store the agent session id
transcript_dir      where to put useful session files
runtime_dir         scratch/config files
agent_home_dir      agent home/config directory
```

And settings like:

```text
model
binary
permission_mode
initial_prompt
lifecycle_prompt
auto_continue_prompt
extra_args
env
sandbox_enabled
```

Use `binary` when a run must pin an exact CLI version, for example a script-owned
wrapper or a path to a versioned install. Clawblox should not silently choose a
different agent binary for repeatable runs.

## What To Implement

An agent needs to support:

1. **Check setup**
   Confirm the CLI is installed and logged in.

2. **Start**
   Start a new session in `workspace_dir`.

3. **Continue**
   Continue the saved session with a new prompt.

4. **Remember**
   Write the current session id to `session_id_file`.

5. **Keep transcripts**
   Put useful session files in `transcript_dir`.

6. **Auto-continue**
   Configure the agent to continue instead of stopping early. Use `auto_continue_prompt` as the message.

7. **Support sandboxing**
   List the files, directories, env vars, hooks, and network access the agent needs when `--sandbox` is enabled.

## Ready Checklist

An agent is ready for repeatable runs when it can:

- start successfully
- write a session id
- continue from that session id
- handle a later prompt in the same session
- auto-continue when the agent tries to stop early
- preserve workspace changes
- leave useful session files in `transcript_dir`
- write a rerunnable command file
- write operation events that explain what happened

Agents that cannot continue sessions or auto-continue are not ready for Clawblox-managed repeatable runs.

## Debugging Contract

Agent operations must be inspectable. A failed run should tell the user where to look.

Each operation should write:

```text
runtime/start.sh       rerunnable command file
events.jsonl           structured operation log
logs/agent.log         agent/runtime log when available
```

Errors should include the agent directory, command file, event log, and tmux attach command when available.
