#!/usr/bin/env node
/**
 * postinstall.js
 * Downloads the correct pre-built Provn binary from GitHub Releases
 * and places it in the package's bin/ directory.
 */

const https = require("https");
const fs = require("fs");
const path = require("path");
const os = require("os");
const { execSync } = require("child_process");

const REPO = "kshitizz36/Provn";
const BIN_DIR = path.join(__dirname, "bin");

function getArtifact() {
  const platform = os.platform();
  const arch = os.arch();

  if (platform === "darwin") {
    if (arch === "arm64") return "provn-aarch64-apple-darwin.tar.gz";
    if (arch === "x64") return "provn-x86_64-apple-darwin.tar.gz";
  }
  if (platform === "linux") {
    if (arch === "x64") return "provn-x86_64-linux.tar.gz";
    if (arch === "arm64") return "provn-aarch64-linux.tar.gz";
  }
  if (platform === "win32" && arch === "x64") {
    return "provn-x86_64-windows.zip";
  }

  throw new Error(`Unsupported platform: ${platform} ${arch}`);
}

function fetchJson(url) {
  return new Promise((resolve, reject) => {
    const opts = {
      headers: {
        "User-Agent": "provn-npm-installer",
        Accept: "application/vnd.github+json",
      },
    };
    https
      .get(url, opts, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          return fetchJson(res.headers.location).then(resolve).catch(reject);
        }
        let data = "";
        res.on("data", (d) => (data += d));
        res.on("end", () => {
          try {
            resolve(JSON.parse(data));
          } catch (err) {
            reject(new Error(`Bad JSON from ${url}: ${err.message}`));
          }
        });
      })
      .on("error", reject);
  });
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const opts = { headers: { "User-Agent": "provn-npm-installer" } };
    https
      .get(url, opts, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          return download(res.headers.location, dest).then(resolve).catch(reject);
        }
        const file = fs.createWriteStream(dest);
        res.pipe(file);
        file.on("finish", () => file.close(resolve));
      })
      .on("error", reject);
  });
}

async function main() {
  const artifact = getArtifact();
  const isWindows = artifact.endsWith(".zip");
  const packageVersion = require("./package.json").version;
  const tag = `v${packageVersion}`;

  const release = await fetchJson(`https://api.github.com/repos/${REPO}/releases/tags/${tag}`);
  const asset = release.assets?.find((entry) => entry.name === artifact);

  if (!asset) {
    throw new Error(
      `Binary not found in release ${tag}. ` +
        `Falling back to build from source: https://github.com/${REPO}#install`
    );
  }

  if (!fs.existsSync(BIN_DIR)) fs.mkdirSync(BIN_DIR, { recursive: true });
  const tmpFile = path.join(os.tmpdir(), artifact);
  process.stdout.write(`Downloading Provn ${tag} for ${os.platform()}/${os.arch()}...`);
  await download(asset.browser_download_url, tmpFile);
  process.stdout.write(" done\n");

  const binPath = path.join(BIN_DIR, isWindows ? "provn.exe" : "provn");
  if (isWindows) {
    execSync(`powershell -Command "Expand-Archive -Force '${tmpFile}' '${BIN_DIR}'"`, {
      stdio: "inherit",
    });
  } else {
    execSync(`tar xzf "${tmpFile}" -C "${BIN_DIR}"`, { stdio: "inherit" });
    fs.chmodSync(binPath, 0o755);
  }

  fs.unlinkSync(tmpFile);
  console.log(`  ✓  provn installed → ${binPath}`);
}

main().catch((err) => {
  console.error(`\n  provn install failed: ${err.message}`);
  console.error(`  Install manually: https://github.com/${REPO}#install`);
});
