const os = require("os");
const fs = require("fs");
const path = require("path");
const https = require("https");
const { execSync } = require("child_process");

const VERSION = "0.1.2";
const BIN_DIR = path.join(__dirname, "bin");

function getBinaryName() {
  const platform = os.platform();
  const arch = os.arch();

  const map = {
    "linux-x64": "wakawiki-linux-x64",
    "darwin-x64": "wakawiki-macos-x64",
    "darwin-arm64": "wakawiki-macos-arm64",
    "win32-x64": "wakawiki-windows-x64.exe",
  };

  const key = `${platform}-${arch}`;
  const name = map[key];
  if (!name) {
    console.error(`Unsupported platform: ${platform}-${arch}`);
    process.exit(1);
  }
  return name;
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);
    https
      .get(url, (response) => {
        if (response.statusCode === 302 || response.statusCode === 301) {
          https.get(response.headers.location, (res) => {
            res.pipe(file);
            file.on("finish", () => {
              file.close();
              resolve();
            });
          });
        } else {
          response.pipe(file);
          file.on("finish", () => {
            file.close();
            resolve();
          });
        }
      })
      .on("error", (err) => {
        fs.unlink(dest, () => {});
        reject(err);
      });
  });
}

async function main() {
  const binaryName = getBinaryName();
  const dest = path.join(BIN_DIR, os.platform() === "win32" ? "wakawiki.exe" : "wakawiki");
  const url = `https://github.com/sonyarianto/wakawiki/releases/download/v${VERSION}/${binaryName}`;

  if (!fs.existsSync(BIN_DIR)) {
    fs.mkdirSync(BIN_DIR, { recursive: true });
  }

  console.log(`Downloading wakawiki v${VERSION} for ${os.platform()}-${os.arch()}...`);

  try {
    await download(url, dest);
    fs.chmodSync(dest, "755");
    console.log("wakawiki installed successfully");
  } catch (e) {
    console.error(`Failed to download binary: ${e.message}`);
    console.error(`URL: ${url}`);
    process.exit(1);
  }
}

main();
