#!/usr/bin/env node
/**
 * Thin wrapper — calls the native provn binary with all CLI args forwarded.
 */

const { execFileSync } = require("child_process");
const path = require("path");
const os   = require("os");

const isWindows = os.platform() === "win32";
const binary    = path.join(__dirname, isWindows ? "provn.exe" : "provn");

try {
  execFileSync(binary, process.argv.slice(2), { stdio: "inherit" });
} catch (err) {
  process.exit(err.status ?? 1);
}
