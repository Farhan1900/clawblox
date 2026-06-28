#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const DEFAULT_PRICING = {
	"claude-opus-4-6": {
		label: "Claude Opus 4.6 pricing",
		inputPerMTok: 5,
		outputPerMTok: 25,
		cacheWrite5mPerMTok: 6.25,
		cacheWrite1hPerMTok: 10,
		cacheReadPerMTok: 0.5,
		inferred: false,
	},
	"claude-opus-4-5": {
		label: "Claude Opus 4.5 pricing",
		inputPerMTok: 5,
		outputPerMTok: 25,
		cacheWrite5mPerMTok: 6.25,
		cacheWrite1hPerMTok: 10,
		cacheReadPerMTok: 0.5,
		inferred: false,
	},
	"claude-opus-4": {
		label: "Claude Opus 4 pricing",
		inputPerMTok: 15,
		outputPerMTok: 75,
		cacheWrite5mPerMTok: 18.75,
		cacheWrite1hPerMTok: 30,
		cacheReadPerMTok: 1.5,
		inferred: false,
	},
	"claude-opus-4-1": {
		label: "Claude Opus 4.1 pricing",
		inputPerMTok: 15,
		outputPerMTok: 75,
		cacheWrite5mPerMTok: 18.75,
		cacheWrite1hPerMTok: 30,
		cacheReadPerMTok: 1.5,
		inferred: false,
	},
	"claude-sonnet-4": {
		label: "Claude Sonnet 4 pricing",
		inputPerMTok: 3,
		outputPerMTok: 15,
		cacheWrite5mPerMTok: 3.75,
		cacheWrite1hPerMTok: 6,
		cacheReadPerMTok: 0.3,
		inferred: false,
	},
	"claude-3-7-sonnet": {
		label: "Claude Sonnet 3.7 pricing",
		inputPerMTok: 3,
		outputPerMTok: 15,
		cacheWrite5mPerMTok: 3.75,
		cacheWrite1hPerMTok: 6,
		cacheReadPerMTok: 0.3,
		inferred: false,
	},
	"claude-3-5-sonnet": {
		label: "Claude Sonnet 3.5 pricing",
		inputPerMTok: 3,
		outputPerMTok: 15,
		cacheWrite5mPerMTok: 3.75,
		cacheWrite1hPerMTok: 6,
		cacheReadPerMTok: 0.3,
		inferred: false,
	},
	"claude-sonnet-4-6": {
		label: "Claude Sonnet 4.6 pricing",
		inputPerMTok: 3,
		outputPerMTok: 15,
		cacheWrite5mPerMTok: 3.75,
		cacheWrite1hPerMTok: 6,
		cacheReadPerMTok: 0.3,
		inferred: false,
	},
	"claude-sonnet-4-5": {
		label: "Claude Sonnet 4.5 pricing",
		inputPerMTok: 3,
		outputPerMTok: 15,
		cacheWrite5mPerMTok: 3.75,
		cacheWrite1hPerMTok: 6,
		cacheReadPerMTok: 0.3,
		inferred: false,
	},
	"claude-haiku-4-5": {
		label: "Claude Haiku 4.5 pricing",
		inputPerMTok: 1,
		outputPerMTok: 5,
		cacheWrite5mPerMTok: 1.25,
		cacheWrite1hPerMTok: 2,
		cacheReadPerMTok: 0.1,
		inferred: false,
	},
	"claude-3-5-haiku": {
		label: "Claude Haiku 3.5 pricing",
		inputPerMTok: 0.8,
		outputPerMTok: 4,
		cacheWrite5mPerMTok: 1,
		cacheWrite1hPerMTok: 1.6,
		cacheReadPerMTok: 0.08,
		inferred: false,
	},
	"claude-3-haiku": {
		label: "Claude Haiku 3 pricing",
		inputPerMTok: 0.25,
		outputPerMTok: 1.25,
		cacheWrite5mPerMTok: 0.3,
		cacheWrite1hPerMTok: 0.5,
		cacheReadPerMTok: 0.03,
		inferred: false,
	},
};

function usage() {
	console.error(
		[
			"Usage: node agent/claude_usage_report.mjs <path> [--json] [--price-model <model>]",
			"",
			"Scans for Claude transcript JSONL files under <path> and summarizes usage.",
			"",
			"Options:",
			"  --json                 Output JSON",
			"  --price-model <model>  Override pricing model for cost estimation",
		].join("\n"),
	);
	process.exit(1);
}

function parseArgs(argv) {
	if (argv.length === 0) usage();
	const args = { root: null, json: false, priceModel: null };
	for (let i = 0; i < argv.length; i += 1) {
		const arg = argv[i];
		if (arg === "--json") {
			args.json = true;
		} else if (arg === "--price-model") {
			i += 1;
			args.priceModel = argv[i];
			if (!args.priceModel) usage();
		} else if (arg.startsWith("-")) {
			usage();
		} else if (!args.root) {
			args.root = arg;
		} else {
			usage();
		}
	}
	if (!args.root) usage();
	return args;
}

function walk(dir, out = []) {
	for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
		const fullPath = path.join(dir, entry.name);
		if (entry.isDirectory()) {
			walk(fullPath, out);
		} else if (entry.isFile()) {
			out.push(fullPath);
		}
	}
	return out;
}

function findTranscriptFiles(rootPath) {
	const stat = fs.statSync(rootPath);
	if (stat.isFile()) return [rootPath];
	const allFiles = walk(rootPath);
	const projectFiles = allFiles.filter(
		(file) => file.endsWith(".jsonl") && file.includes(`${path.sep}.claude${path.sep}projects${path.sep}-workspace${path.sep}`),
	);
	if (projectFiles.length > 0) return projectFiles;
	return allFiles.filter((file) => file.endsWith(".jsonl") && file.includes(`${path.sep}session${path.sep}`));
}

function findSessionMetadata(transcriptPath) {
	let agentDir = transcriptPath;
	if (transcriptPath.includes(`${path.sep}session${path.sep}`)) {
		agentDir = transcriptPath.split(`${path.sep}session${path.sep}`)[0];
	} else if (transcriptPath.includes(`${path.sep}sandbox-home${path.sep}.claude${path.sep}projects${path.sep}-workspace${path.sep}`)) {
		agentDir = transcriptPath.split(`${path.sep}sandbox-home${path.sep}.claude${path.sep}projects${path.sep}-workspace${path.sep}`)[0];
	}
	const sessionStartPath = path.join(agentDir, "logs", "session_start.json");
	let metadata = null;
	if (fs.existsSync(sessionStartPath)) {
		try {
			metadata = JSON.parse(fs.readFileSync(sessionStartPath, "utf8"));
		} catch {}
	}
	return { agentDir, sessionStartPath, metadata };
}

function canonicalModelName(model) {
	if (!model) return null;
	const candidates = Object.keys(DEFAULT_PRICING).sort((a, b) => b.length - a.length);
	for (const candidate of candidates) {
		if (model.startsWith(candidate)) return candidate;
	}
	return null;
}

function computeCost(summary, pricing) {
	const inputCost = (summary.inputTokens / 1_000_000) * pricing.inputPerMTok;
	const outputCost = (summary.outputTokens / 1_000_000) * pricing.outputPerMTok;
	const cacheReadCost = (summary.cacheReadTokens / 1_000_000) * pricing.cacheReadPerMTok;

	let cacheWriteCost = 0;
	if (summary.cacheWrite1hTokens > 0 || summary.cacheWrite5mTokens > 0) {
		cacheWriteCost =
			(summary.cacheWrite5mTokens / 1_000_000) * pricing.cacheWrite5mPerMTok +
			(summary.cacheWrite1hTokens / 1_000_000) * pricing.cacheWrite1hPerMTok;
	} else {
		cacheWriteCost = (summary.cacheWriteTokens / 1_000_000) * pricing.cacheWrite1hPerMTok;
	}

	return {
		input: inputCost,
		output: outputCost,
		cacheWrite: cacheWriteCost,
		cacheRead: cacheReadCost,
		total: inputCost + outputCost + cacheWriteCost + cacheReadCost,
	};
}

function parseTranscript(transcriptPath, priceModelOverride) {
	const seen = new Set();
	const summary = {
		transcriptPath,
		models: new Set(),
		uniqueUsageEvents: 0,
		inputTokens: 0,
		outputTokens: 0,
		cacheWriteTokens: 0,
		cacheWrite5mTokens: 0,
		cacheWrite1hTokens: 0,
		cacheReadTokens: 0,
	};

	for (const line of fs.readFileSync(transcriptPath, "utf8").split(/\n/)) {
		if (!line.trim()) continue;
		let record;
		try {
			record = JSON.parse(line);
		} catch {
			continue;
		}

		const message = record.message;
		const usage = message?.usage || record.usage;
		if (!usage) continue;

		const key = record.requestId || message?.id || record.uuid;
		if (!key || seen.has(key)) continue;
		seen.add(key);

		summary.uniqueUsageEvents += 1;
		summary.inputTokens += usage.input_tokens || usage.input || 0;
		summary.outputTokens += usage.output_tokens || usage.output || 0;

		const cacheWrite = usage.cache_creation_input_tokens || usage.cacheWrite || 0;
		const cacheRead = usage.cache_read_input_tokens || usage.cacheRead || 0;
		summary.cacheWriteTokens += cacheWrite;
		summary.cacheReadTokens += cacheRead;

		const cacheCreation = usage.cache_creation || {};
		summary.cacheWrite5mTokens += cacheCreation.ephemeral_5m_input_tokens || 0;
		summary.cacheWrite1hTokens += cacheCreation.ephemeral_1h_input_tokens || 0;

		const model = message?.model || record.model;
		if (model) summary.models.add(model);
	}

	const model = priceModelOverride || [...summary.models][0] || null;
	const pricingKey = priceModelOverride || canonicalModelName(model);
	const pricing = pricingKey ? DEFAULT_PRICING[pricingKey] : null;
	const cost = pricing ? computeCost(summary, pricing) : null;

	return {
		...summary,
		models: [...summary.models],
		modelForPricing: model,
		pricingKey,
		pricing,
		cost,
		totalTokens:
			summary.inputTokens +
			summary.outputTokens +
			summary.cacheWriteTokens +
			summary.cacheReadTokens,
	};
}

function sumReports(reports) {
	const totals = {
		transcripts: reports.length,
		uniqueUsageEvents: 0,
		inputTokens: 0,
		outputTokens: 0,
		cacheWriteTokens: 0,
		cacheWrite5mTokens: 0,
		cacheWrite1hTokens: 0,
		cacheReadTokens: 0,
		totalTokens: 0,
		cost: { input: 0, output: 0, cacheWrite: 0, cacheRead: 0, total: 0 },
		models: new Set(),
	};

	for (const report of reports) {
		totals.uniqueUsageEvents += report.uniqueUsageEvents;
		totals.inputTokens += report.inputTokens;
		totals.outputTokens += report.outputTokens;
		totals.cacheWriteTokens += report.cacheWriteTokens;
		totals.cacheWrite5mTokens += report.cacheWrite5mTokens;
		totals.cacheWrite1hTokens += report.cacheWrite1hTokens;
		totals.cacheReadTokens += report.cacheReadTokens;
		totals.totalTokens += report.totalTokens;
		for (const model of report.models) totals.models.add(model);
		if (report.cost) {
			totals.cost.input += report.cost.input;
			totals.cost.output += report.cost.output;
			totals.cost.cacheWrite += report.cost.cacheWrite;
			totals.cost.cacheRead += report.cost.cacheRead;
			totals.cost.total += report.cost.total;
		}
	}

	return { ...totals, models: [...totals.models] };
}

function fmtInt(n) {
	return new Intl.NumberFormat("en-US").format(n);
}

function fmtUsd(n) {
	return `$${n.toFixed(4)}`;
}

function main() {
	const args = parseArgs(process.argv.slice(2));
	const rootPath = path.resolve(args.root);
	const transcriptFiles = findTranscriptFiles(rootPath);
	if (transcriptFiles.length === 0) {
		console.error(`No Claude transcript JSONL files found under ${rootPath}`);
		process.exit(1);
	}

	const reports = transcriptFiles.map((transcriptPath) => {
		const parsed = parseTranscript(transcriptPath, args.priceModel);
		const meta = findSessionMetadata(transcriptPath);
		return {
			...parsed,
			agentDir: meta.agentDir,
			sessionStartPath: meta.sessionStartPath,
			sessionStart: meta.metadata,
		};
	});

	const totals = sumReports(reports);

	if (args.json) {
		console.log(
			JSON.stringify(
				{
					rootPath,
					reports,
					totals,
				},
				null,
				2,
			),
		);
		return;
	}

	console.log(`Claude usage report for ${rootPath}`);
	console.log("");
	for (const report of reports) {
		console.log(`Transcript: ${report.transcriptPath}`);
		console.log(`Agent dir:  ${report.agentDir}`);
		if (report.sessionStart?.session_id) console.log(`Last start: ${report.sessionStart.session_id}`);
		if (report.models.length > 0) console.log(`Models:     ${report.models.join(", ")}`);
		console.log(`Events:     ${fmtInt(report.uniqueUsageEvents)}`);
		console.log(`Input:      ${fmtInt(report.inputTokens)}`);
		console.log(`Output:     ${fmtInt(report.outputTokens)}`);
		console.log(`Cache write:${fmtInt(report.cacheWriteTokens)}`);
		console.log(`Cache read: ${fmtInt(report.cacheReadTokens)}`);
		console.log(`Total:      ${fmtInt(report.totalTokens)}`);
		if (report.cost && report.pricing) {
			console.log(`Est. cost:  ${fmtUsd(report.cost.total)} using ${report.pricing.label}${report.pricing.inferred ? " (inferred for this model)" : ""}`);
		} else {
			console.log("Est. cost:  unavailable (no pricing map for model)");
		}
		console.log("");
	}

	console.log("Totals");
	console.log(`Transcripts:${fmtInt(totals.transcripts)}`);
	console.log(`Events:     ${fmtInt(totals.uniqueUsageEvents)}`);
	console.log(`Input:      ${fmtInt(totals.inputTokens)}`);
	console.log(`Output:     ${fmtInt(totals.outputTokens)}`);
	console.log(`Cache write:${fmtInt(totals.cacheWriteTokens)}`);
	console.log(`Cache read: ${fmtInt(totals.cacheReadTokens)}`);
	console.log(`Total:      ${fmtInt(totals.totalTokens)}`);
	console.log(`Est. cost:  ${fmtUsd(totals.cost.total)}`);
}

main();
