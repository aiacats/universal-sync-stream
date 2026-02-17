import { execSync } from "child_process";
import { copyFileSync, mkdirSync, existsSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";
import { homedir } from "os";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..", "..", "..");

// Ensure Rust toolchain is in PATH
const cargoHome = process.env.CARGO_HOME || join(homedir(), ".cargo");
const cargoBin = join(cargoHome, "bin");
const pathSep = process.platform === "win32" ? ";" : ":";
const env = { ...process.env, PATH: `${cargoBin}${pathSep}${process.env.PATH}` };

// Get target triple from rustc
let triple;
try {
  const rustcOutput = execSync("rustc -vV", { encoding: "utf-8", env });
  const tripleMatch = rustcOutput.match(/host: (.+)/);
  if (!tripleMatch) {
    console.error("Failed to detect Rust target triple from rustc -vV");
    process.exit(1);
  }
  triple = tripleMatch[1].trim();
} catch (e) {
  console.error("Failed to run rustc. Is Rust installed?");
  console.error(e.message);
  process.exit(1);
}

const ext = process.platform === "win32" ? ".exe" : "";
const srcBin = join(root, "target", "debug", `ussp-server${ext}`);
const destDir = join(__dirname, "..", "src-tauri", "binaries");
const destBin = join(destDir, `ussp-server-${triple}${ext}`);

if (!existsSync(srcBin)) {
  console.error(`Source binary not found: ${srcBin}`);
  console.error("Make sure 'cargo build -p ussp-server' completed successfully.");
  process.exit(1);
}

if (!existsSync(destDir)) {
  mkdirSync(destDir, { recursive: true });
}

copyFileSync(srcBin, destBin);
console.log(`Sidecar binary copied: ${destBin}`);
