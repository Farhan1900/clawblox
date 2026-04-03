#!/usr/bin/env node
import process from "node:process";
import { getModel, complete } from "@mariozechner/pi-ai";

async function main() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  const input = JSON.parse(Buffer.concat(chunks).toString("utf8"));

  const model = getModel("anthropic", input.model);
  const response = await complete(
    model,
    {
      systemPrompt: input.systemPrompt,
      messages: [
        {
          role: "user",
          content: input.userText,
          timestamp: Date.now(),
        },
      ],
    },
    {
      maxTokens: input.maxTokens,
      apiKey: process.env.ANTHROPIC_API_KEY,
    },
  );

  if (response.stopReason === "error" || response.errorMessage) {
    process.stderr.write(
      `pi-ai provider error: ${response.errorMessage || JSON.stringify(response, null, 2)}\n`,
    );
    process.exit(1);
  }

  const text = (response.content || [])
    .filter((item) => item.type === "text")
    .map((item) => item.text)
    .join("\n\n")
    .trim();

  if (!text) {
    process.stderr.write(`pi-ai returned no text content: ${JSON.stringify(response, null, 2)}\n`);
    process.exit(1);
  }

  process.stdout.write(text);
}

main().catch((err) => {
  const message = err instanceof Error ? err.stack || err.message : String(err);
  process.stderr.write(message + "\n");
  process.exit(1);
});
