import type { ExtensionAPI, ExtensionContext } from "@mariozechner/pi-coding-agent";
import { Type } from "@sinclair/typebox";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

const AUTO_LOOP_STEER = "continue";
const PROJECT_CONTEXT_SECTION = /(?:^|\n)# Project Context\b[\s\S]*$/m;
const CHECKPOINT_PROMPT = (process.env.AGENT_CHECKPOINT_PROMPT ?? "").trim();
const CHECKPOINT_AT_EPOCH = parseEpochSeconds(process.env.AGENT_CHECKPOINT_AT_EPOCH);
const CHECKPOINT_TRIGGER_TURN = process.env.AGENT_CHECKPOINT_TRIGGER_TURN !== "0";
const CHECKPOINT_DISABLE_AUTO_LOOP = process.env.AGENT_CHECKPOINT_DISABLE_AUTO_LOOP === "1";
const WORLD_BASE_URL = (process.env.WORLD_BASE_URL ?? "").trim();
const CHAT_POLL_INTERVAL_MS = 2500;
const CHAT_MESSAGES_LIMIT = 20;

type ChatMessage = {
  id?: string;
  agent_name?: string;
  content?: string;
  created_at?: string;
};

function writeSystemPromptSnapshot(prompt: string) {
  const outputPath = process.env.PI_SYSTEM_PROMPT_SNAPSHOT_FILE;
  if (!outputPath || !prompt.trim()) return;
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, prompt.endsWith("\n") ? prompt : `${prompt}\n`, "utf8");
}

function stripProjectContext(prompt: string): string {
  return prompt.replace(PROJECT_CONTEXT_SECTION, "").trimEnd();
}

function parseEpochSeconds(value: string | undefined): number | null {
  if (!value) return null;
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) return null;
  return Math.floor(parsed);
}

function normalizeBaseUrl(url: string): string {
  return url.replace(/\/+$/, "");
}

function formatChatNotification(message: ChatMessage): string | null {
  const content = String(message.content ?? "").trim();
  if (!content) return null;
  const sender = String(message.agent_name ?? "Someone").trim() || "Someone";
  return `New chat message from ${sender}: ${content}`;
}

export default function (pi: ExtensionAPI) {
  let autoLoopEnabled = true;
  let checkpointSent = false;
  let checkpointTimer: ReturnType<typeof setTimeout> | null = null;
  let chatPollTimer: ReturnType<typeof setInterval> | null = null;
  let chatPollInFlight = false;
  let chatAfter: string | null = null;
  let chatBaselineLoaded = false;
  let chatPollingEnabled = false;
  const seenChatIds = new Set<string>();
  let latestCtx: ExtensionContext | null = null;

  const sendCheckpointPrompt = () => {
    if (checkpointSent) return;
    if (!CHECKPOINT_PROMPT) return;
    checkpointSent = true;

    if (CHECKPOINT_DISABLE_AUTO_LOOP) {
      autoLoopEnabled = false;
      if (latestCtx?.hasUI) latestCtx.ui.setStatus("agent-loop", "⏸ agent");
    }

    pi.sendUserMessage(CHECKPOINT_PROMPT, {
      deliverAs: "steer",
      triggerTurn: CHECKPOINT_TRIGGER_TURN,
    });
  };

  const scheduleCheckpointPrompt = () => {
    if (!CHECKPOINT_AT_EPOCH || !CHECKPOINT_PROMPT) return;
    const delayMs = CHECKPOINT_AT_EPOCH * 1000 - Date.now();
    if (delayMs <= 0) {
      sendCheckpointPrompt();
      return;
    }
    checkpointTimer = setTimeout(sendCheckpointPrompt, delayMs);
  };

  const ingestChatMessages = (messages: ChatMessage[], notify: boolean) => {
    for (const message of [...messages].reverse()) {
      const id = String(message.id ?? "").trim();
      if (id) {
        if (seenChatIds.has(id)) continue;
        seenChatIds.add(id);
      }

      const createdAt = String(message.created_at ?? "").trim();
      if (createdAt && (!chatAfter || createdAt > chatAfter)) {
        chatAfter = createdAt;
      }

      if (!notify) continue;
      const text = formatChatNotification(message);
      if (!text) continue;
      pi.sendUserMessage(text, { deliverAs: "steer", triggerTurn: true });
    }
  };

  const pollChatMessages = async () => {
    if (!chatPollingEnabled || chatPollInFlight || !WORLD_BASE_URL) return;
    chatPollInFlight = true;
    try {
      const url = new URL(`${normalizeBaseUrl(WORLD_BASE_URL)}/chat/messages`);
      url.searchParams.set("limit", String(CHAT_MESSAGES_LIMIT));
      if (chatAfter) url.searchParams.set("after", chatAfter);

      const response = await fetch(url.toString());
      if (response.status === 404 || response.status === 405) {
        chatPollingEnabled = false;
        return;
      }
      if (!response.ok) return;

      const data = (await response.json()) as { messages?: ChatMessage[] };
      if (!Array.isArray(data.messages) || data.messages.length === 0) {
        chatBaselineLoaded = true;
        return;
      }

      ingestChatMessages(data.messages, chatBaselineLoaded);
      chatBaselineLoaded = true;
    } catch {
      // Ignore polling failures so the main loop remains unaffected.
    } finally {
      chatPollInFlight = false;
    }
  };

  const startChatPolling = () => {
    if (!WORLD_BASE_URL) return;
    chatPollingEnabled = true;
    void pollChatMessages();
    chatPollTimer = setInterval(() => {
      void pollChatMessages();
    }, CHAT_POLL_INTERVAL_MS);
  };

  pi.on("before_agent_start", async (event) => {
    const sanitizedPrompt = stripProjectContext(event.systemPrompt);
    writeSystemPromptSnapshot(sanitizedPrompt);
    scheduleCheckpointPrompt();
    startChatPolling();
    return { systemPrompt: sanitizedPrompt };
  });

  pi.registerCommand("agent-auto", {
    description: "Control the generic continue auto-loop: /agent-auto on|off|status",
    handler: async (args, ctx) => {
      latestCtx = ctx;
      const mode = (args || "").trim().toLowerCase();
      if (mode === "off") {
        autoLoopEnabled = false;
        if (ctx.hasUI) ctx.ui.setStatus("agent-loop", "⏸ agent");
        ctx.ui.notify("Agent auto-loop disabled", "info");
        return;
      }
      if (mode === "on" || mode === "") {
        autoLoopEnabled = true;
        if (ctx.hasUI) ctx.ui.setStatus("agent-loop", "♾ agent");
        ctx.ui.notify("Agent auto-loop enabled", "info");
        if (ctx.isIdle() && !ctx.hasPendingMessages()) {
          pi.sendUserMessage(AUTO_LOOP_STEER, { deliverAs: "steer" });
        }
        return;
      }
      if (mode === "status") {
        ctx.ui.notify(`Agent auto-loop is ${autoLoopEnabled ? "enabled" : "disabled"}`, "info");
        return;
      }
      ctx.ui.notify("Usage: /agent-auto on|off|status", "warning");
    },
  });

  pi.on("agent_end", async (_event, ctx) => {
    latestCtx = ctx;
    if (ctx.hasUI) ctx.ui.setStatus("agent-loop", autoLoopEnabled ? "♾ agent" : "⏸ agent");
    if (!autoLoopEnabled) return;
    if (!ctx.isIdle()) return;
    if (ctx.hasPendingMessages()) return;
    pi.sendUserMessage(AUTO_LOOP_STEER, { deliverAs: "steer" });
  });

  pi.registerCommand("reload-runtime", {
    description: "Reload extensions, skills, prompts, and themes",
    handler: async (_args, ctx) => {
      await ctx.reload();
      return;
    },
  });

  pi.registerTool({
    name: "reload_runtime",
    label: "Reload Runtime",
    description: "Reload extensions, skills, prompts, and themes",
    parameters: Type.Object({}),
    async execute() {
      pi.sendUserMessage("/reload-runtime", { deliverAs: "followUp" });
      return {
        content: [{ type: "text", text: "Queued /reload-runtime as a follow-up command." }],
        details: {},
      };
    },
  });

  pi.on("session_shutdown", async () => {
    if (checkpointTimer) {
      clearTimeout(checkpointTimer);
      checkpointTimer = null;
    }
    if (chatPollTimer) {
      clearInterval(chatPollTimer);
      chatPollTimer = null;
    }
  });
}
