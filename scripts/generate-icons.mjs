import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const outDir = fileURLToPath(new URL("../src-tauri/icons/", import.meta.url));
const sourceIcon = join(outDir, "icon.ico");
const tmpDir = mkdtempSync(join(tmpdir(), "wordsnap-icons-"));

try {
  const sourcePng = join(tmpDir, "icon-source.png");
  run("sips", ["-s", "format", "png", sourceIcon, "--out", sourcePng]);

  resize(sourcePng, "32x32.png", 32);
  resize(sourcePng, "128x128.png", 128);
  resize(sourcePng, "128x128@2x.png", 256);
  resize(sourcePng, "icon.png", 512);
  resize(sourcePng, "tray.png", 64);
  resize(sourcePng, "tray-template.png", 32);

  const iconset = join(tmpDir, "WordSnap.iconset");
  mkdirSync(iconset);
  resizeTo(sourcePng, join(iconset, "icon_16x16.png"), 16);
  resizeTo(sourcePng, join(iconset, "icon_16x16@2x.png"), 32);
  resizeTo(sourcePng, join(iconset, "icon_32x32.png"), 32);
  resizeTo(sourcePng, join(iconset, "icon_32x32@2x.png"), 64);
  resizeTo(sourcePng, join(iconset, "icon_128x128.png"), 128);
  resizeTo(sourcePng, join(iconset, "icon_128x128@2x.png"), 256);
  resizeTo(sourcePng, join(iconset, "icon_256x256.png"), 256);
  resizeTo(sourcePng, join(iconset, "icon_256x256@2x.png"), 512);
  resizeTo(sourcePng, join(iconset, "icon_512x512.png"), 512);
  resizeTo(sourcePng, join(iconset, "icon_512x512@2x.png"), 1024);

  run("iconutil", ["-c", "icns", iconset, "-o", join(outDir, "icon.icns")]);
} finally {
  rmSync(tmpDir, { recursive: true, force: true });
}

function resize(source, name, size) {
  resizeTo(source, join(outDir, name), size);
}

function resizeTo(source, destination, size) {
  run("sips", ["-z", String(size), String(size), source, "--out", destination]);
}

function run(command, args) {
  execFileSync(command, args, { stdio: "ignore" });
}
