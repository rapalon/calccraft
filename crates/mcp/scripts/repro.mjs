// Runs the review's reproduction cases (docs/mcp-fixes.md) against a built calccraft-mcp over
// stdio, as an MCP client does, and prints one line per case with the time it took.
//
//   node crates/mcp/scripts/repro.mjs target/release/calccraft-mcp(.exe)
//
// It needs only Node.js: the protocol is newline-delimited JSON-RPC.

import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { pathToFileURL } from "node:url";

const server = spawn(process.argv[2] ?? "target/release/calccraft-mcp", [], {
  stdio: ["pipe", "pipe", "inherit"],
});
const lines = createInterface({ input: server.stdout });
const waiting = new Map();
const stray = [];
lines.on("line", (line) => {
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    stray.push(line);
    return;
  }
  const resolve = waiting.get(message.id);
  if (resolve) {
    waiting.delete(message.id);
    resolve(message);
  }
});

let nextId = 1;
function request(method, params, timeoutMs = 30000) {
  const id = nextId++;
  server.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  return new Promise((resolve) => {
    waiting.set(id, resolve);
    setTimeout(() => {
      if (waiting.delete(id)) resolve({ timeout: true });
    }, timeoutMs);
  });
}

function nested(levels) {
  return "(".repeat(levels) + "1+1)*2" + "+1)*2".repeat(levels - 1);
}

const nines = "9".repeat(44);
const defaultCases = [
  ["B1", "programmer", { expression: nines }],
  ["B1", "programmer", { expression: "340282366920938463463374607431768211456" }],
  ["B1", "programmer", { expression: "9223372036854775807" }],
  ["B1", "programmer", { expression: "9223372036854775808" }],
  ["B1", "programmer", { expression: "-9223372036854775808" }],
  ["B1", "programmer", { expression: "-9223372036854775809" }],
  ["B1", "programmer", { expression: "FFFFFFFFFFFFFFFF", radix: "hex" }],
  ["B1", "programmer", { expression: "10000000000000000", radix: "hex" }],
  ["B2", "convert_units", { category: "data", value: "1", from: "GB", to: "MiB" }],
  ["B2", "convert_units", { category: "data", value: "1", from: "GB", to: "MB" }],
  ["B2", "convert_units", { category: "data", value: "1", from: "KB", to: "B" }],
  ["B2", "convert_units", { category: "data", value: "1", from: "TB", to: "GB" }],
  ["B2", "convert_units", { category: "data", value: "1", from: "Gb", to: "MB" }],
  ["I1", "programmer", { expression: nested(24) }],
  ["I1", "programmer", { expression: nested(28) }],
  ["I1", "programmer", { expression: nested(40) }],
  ["I2", "calculate", { expression: "9^9^9" }],
  ["I2", "calculate", { expression: "(2^9999)!" }],
  ["I2", "calculate", { expression: "3248.5!" }],
  ["I2", "calculate", { expression: "(123456789/987654321)^9999" }],
  ["I2", "calculate", { expression: "1" + "+1".repeat(255) }],
  ["I2", "calculate", { expression: "1" + "+1".repeat(256) }],
  ["I2", "analyze_function", { equation: Array(150).fill("sin(1000x)").join("+") }],
  ["I2", "calculate", { expression: "1+1" }],
  ["I3", "programmer", { expression: "-1", word: "byte" }],
  ["I3", "calculate", { expression: "sin(90)", angles: "radians" }],
  ["I3", "calculate", { expression: "1+1", extra: true }],
  ["I4", "convert_units", { category: "volume", value: "1", from: "gallon", to: "liter" }],
  ["I4", "convert_units", { category: "volume", value: "1", from: "cup", to: "mL" }],
  ["I4", "convert_units", { category: "power", value: "1", from: "hp", to: "W" }],
  ["I4", "convert_units", { category: "length", value: "1", from: "parsec", to: "m" }],
  ["I4", "convert_units", { category: "length", value: "1", from: "kilometre", to: "mile" }],
  ["M1", "programmer", { expression: "-128", word_size: "byte" }],
  ["M1", "programmer", { expression: "-129", word_size: "byte" }],
  ["M2", "calculate", { expression: "12345678901234567 + 1", mode: "standard" }],
  ["M2", "calculate", { expression: "2 + 3 * 4 = =", mode: "standard" }],
  ["M5", "calculate", { expression: "200 % 15" }],
];
// Other cases: a module whose default export is a list like the one above.
const cases = process.argv[3]
  ? (await import(pathToFileURL(process.argv[3]))).default
  : defaultCases;

function show(response) {
  if (response.timeout) return "NO ANSWER (timeout)";
  if (response.error) return `JSON-RPC error ${response.error.code}: ${response.error.message}`;
  const result = response.result;
  const text = result.content?.[0]?.text ?? "";
  if (result.isError) return `isError: ${text}`;
  const value = result.structuredContent ?? {};
  const fields = ["result", "dec", "hex", "value", "from", "to", "domain"]
    .filter((key) => key in value)
    .map((key) => `${key}=${JSON.stringify(value[key])}`);
  return fields.join(" ") || text;
}

await request("initialize", {
  protocolVersion: "2025-06-18",
  capabilities: {},
  clientInfo: { name: "repro", version: "1" },
});
server.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");

for (const [finding, tool, args] of cases) {
  const start = Date.now();
  const response = await request("tools/call", { name: tool, arguments: args });
  const input = JSON.stringify(args);
  const shown = input.length > 90 ? input.slice(0, 87) + "..." : input;
  console.log(`${finding} | ${tool} ${shown} | ${show(response)} | ${Date.now() - start} ms`);
}

// M3: a line that isn't JSON.
server.stdin.write("this is not json\n");
const after = await request("tools/call", { name: "calculate", arguments: { expression: "1+1" } });
console.log(`M3 | "this is not json" then calculate 1+1 | ${show(after)}`);
if (stray.length) console.log(`stdout lines that aren't JSON: ${stray.length}`);
server.kill();
