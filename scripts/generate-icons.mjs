import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = fileURLToPath(new URL("../", import.meta.url));
const outDir = join(rootDir, "src-tauri", "icons");
const sourceIcon = join(outDir, "icon-source.png");
const traySource = join(outDir, "tray-source.svg");
const tauriBin = join(
  rootDir,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "tauri.cmd" : "tauri",
);
const tmpDir = mkdtempSync(join(tmpdir(), "wordsnap-icons-"));

if (!existsSync(tauriBin)) {
  throw new Error("Tauri CLI is missing. Run npm ci before generating icons.");
}

try {
  const appDir = join(tmpDir, "app");
  const trayDir = join(tmpDir, "tray");
  mkdirSync(appDir);
  mkdirSync(trayDir);

  generate(sourceIcon, appDir);
  generate(traySource, trayDir);

  for (const name of [
    "32x32.png",
    "128x128.png",
    "128x128@2x.png",
    "icon.png",
    "icon.icns",
    "icon.ico",
  ]) {
    copyFileSync(join(appDir, name), join(outDir, name));
  }

  copyFileSync(join(trayDir, "64x64.png"), join(outDir, "tray.png"));
  copyFileSync(join(trayDir, "32x32.png"), join(outDir, "tray-template.png"));
} finally {
  rmSync(tmpDir, { recursive: true, force: true });
}

function generate(source, destination) {
  execFileSync(tauriBin, ["icon", source, "--output", destination], {
    stdio: "ignore",
  });
}
