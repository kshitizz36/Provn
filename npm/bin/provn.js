#!/usr/bin/env node
/**
 * Thin wrapper — calls the native provn binary with all CLI args forwarded.
 */

const { execFileSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

const isWindows = os.platform() === "win32";
const binary = path.join(__dirname, isWindows ? "provn.exe" : "provn");

if (!fs.existsSync(binary)) {
  console.error("Provn binary is missing from the npm install.");
  console.error("Reinstall after a release is published, or build from source:");
  console.error("  https://github.com/kshitizz36/Provn#install");
  process.exit(1);
}

try {
  execFileSync(binary, process.argv.slice(2), { stdio: "inherit" });
} catch (err) {
  process.exit(err.status ?? 1);
}
