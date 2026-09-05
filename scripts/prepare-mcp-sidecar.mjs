import { copyFileSync, chmodSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const rustcVersion = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
const hostLine = rustcVersion.split("\n").find((line) => line.startsWith("host: "));

if (!hostLine) {
  throw new Error("Could not determine Rust host target triple from rustc -vV.");
}

const targetTriple = hostLine.slice("host: ".length).trim();
const isWindows = targetTriple.includes("windows");
const extension = isWindows ? ".exe" : "";
const source = join(root, "src-tauri", "target", "release", `mcp-server${extension}`);
const destination = join(
  root,
  "src-tauri",
  "bin",
  `mcp-server-${targetTriple}${extension}`,
);

if (!existsSync(source)) {
  throw new Error(`MCP server release binary not found at ${source}. Run pnpm build:mcp first.`);
}

mkdirSync(dirname(destination), { recursive: true });
copyFileSync(source, destination);

if (!isWindows) {
  chmodSync(destination, 0o755);
}

console.log(`Prepared MCP sidecar: ${destination}`);
