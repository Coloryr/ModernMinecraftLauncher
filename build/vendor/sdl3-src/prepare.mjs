// 还原 build/vendor/sdl3-src 的补丁产物：把 crates.io 的 sdl3-src 固定版本取下来，应用
// patches/sdl-trim.patch，产物放到 target/vendor/sdl3-src（已被 git 忽略）。
//
// 与 mml-gui 的 tauri-plugin-decoration 同一套思路：仓库只存补丁，不存第三方源码。
//
// 取源顺序：
//   1. 本机 cargo registry 里已有 sdl3-src-<ver>（cargo fetch / 之前构建过）→ 直接复制；
//   2. 否则从 static.crates.io 下载 <ver>.crate，按 Cargo.lock 的 checksum 校验后解包。
//
// 幂等：补丁内容和版本没变时直接跳过（不会重拷 30 MB）。
// 目前补丁只针对 Windows 裁剪（Linux/macOS 的 evdev / IOKit 符号集合不同），
// 但脚本本身跨平台，非 Windows 上也能跑，只是不保证链接通过。

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const selfDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(selfDir, "..", "..", "..");
const patchDir = path.join(selfDir, "patches");

const lock = fs.readFileSync(path.join(root, "Cargo.lock"), "utf8");
const entry = lock.match(/name = "sdl3-src"\s*\nversion = "([^"]+)"(?:\s*\nsource = "[^"]+"\s*\nchecksum = "([0-9a-f]+)")?/);
if (!entry) {
  console.error("sdl vendor: Cargo.lock 里找不到 sdl3-src");
  process.exit(1);
}
const version = entry[1];
const checksum = entry[2] || "";

const patches = fs.readdirSync(patchDir).filter((f) => f.endsWith(".patch")).sort();
const patchHash = crypto.createHash("sha256");
for (const p of patches) patchHash.update(fs.readFileSync(path.join(patchDir, p)));

const isWindows = process.platform === "win32";
const dest = path.join(root, "target", "vendor", "sdl3-src");
const stampFile = path.join(dest, ".mml-vendor-stamp");
const stamp = crypto.createHash("sha256").update(version + "\n" + (isWindows ? patchHash.digest("hex") : "no-patch")).digest("hex");

if (fs.existsSync(stampFile) && fs.readFileSync(stampFile, "utf8").trim() === stamp) {
  console.log("sdl vendor: 已是最新（" + version + "），跳过");
  process.exit(0);
}

/** 在 cargo registry 里找已解包的源码 */
function findInRegistry() {
  const home = process.env.CARGO_HOME || path.join(os.homedir(), ".cargo");
  const srcRoot = path.join(home, "registry", "src");
  if (!fs.existsSync(srcRoot)) return null;
  for (const index of fs.readdirSync(srcRoot)) {
    const dir = path.join(srcRoot, index, "sdl3-src-" + version);
    if (fs.existsSync(path.join(dir, "SDL", "CMakeLists.txt"))) return dir;
  }
  return null;
}

/** 从 static.crates.io 下载 .crate 并解包，返回源码目录 */
function downloadCrate() {
  const url = "https://static.crates.io/crates/sdl3-src/sdl3-src-" + version + ".crate";
  const tmp = path.join(root, "target", "temp");
  fs.mkdirSync(tmp, { recursive: true });
  const crateFile = path.join(tmp, "sdl3-src-" + version + ".crate");
  console.log("sdl vendor: registry 里没有源码，下载 " + url);
  execFileSync("curl", ["-fsSL", "-o", crateFile, url], { stdio: "inherit" });
  if (checksum) {
    const actual = crypto.createHash("sha256").update(fs.readFileSync(crateFile)).digest("hex");
    if (actual !== checksum) {
      console.error("sdl vendor: checksum 不匹配\n  期望 " + checksum + "\n  实际 " + actual);
      process.exit(1);
    }
  }
  const extractDir = path.join(tmp, "sdl3-src-extract");
  fs.rmSync(extractDir, { recursive: true, force: true });
  fs.mkdirSync(extractDir, { recursive: true });
  execFileSync("tar", ["-xf", crateFile, "-C", extractDir], { stdio: "inherit" });
  const dir = path.join(extractDir, "sdl3-src-" + version);
  if (!fs.existsSync(path.join(dir, "SDL", "CMakeLists.txt"))) {
    console.error("sdl vendor: 解包后找不到 SDL/CMakeLists.txt");
    process.exit(1);
  }
  return dir;
}

const source = findInRegistry() || downloadCrate();
console.log("sdl vendor: 源 " + source + " → " + dest);
fs.rmSync(dest, { recursive: true, force: true });
fs.mkdirSync(path.dirname(dest), { recursive: true });
fs.cpSync(source, dest, { recursive: true });

// 非 Windows 只还原原始源码：补丁里的空桩是按 Windows 的符号集合整理的，
// evdev / IOKit 的引用还没有对应桩，打上去会在链接阶段缺符号。
if (!isWindows) {
  fs.writeFileSync(stampFile, stamp, "utf8");
  console.log("sdl vendor: 非 Windows，使用未打补丁的源码（" + dest + "）");
  process.exit(0);
}

// 应用补丁：git apply 在仓库内以仓库根为基准，显式 --directory 指到产物目录。
const rel = path.relative(root, dest).split(path.sep).join("/");
const tmpPatchDir = path.join(root, "target", "temp");
fs.mkdirSync(tmpPatchDir, { recursive: true });
for (const name of patches) {
  const text = fs.readFileSync(path.join(patchDir, name), "utf8").replace(/\r\n/g, "\n");
  const tmpPatch = path.join(tmpPatchDir, "vendor-" + name);
  fs.writeFileSync(tmpPatch, text, "utf8");
  const touched = [...text.matchAll(/^\+\+\+ b\/(.+)$/gm)].map((m) => m[1].trim());
  try {
    execFileSync("git", ["apply", "--whitespace=nowarn", "--directory=" + rel, tmpPatch], {
      cwd: root,
      stdio: "pipe",
    });
  } catch (error) {
    console.error("sdl vendor: 应用补丁失败 " + name);
    console.error(String(error.stderr || error.message));
    process.exit(1);
  }
  for (const file of touched) {
    const outFile = path.join(dest, file);
    if (!fs.existsSync(outFile)) {
      console.error("sdl vendor: 补丁声明的文件不存在 " + file);
      process.exit(1);
    }
    if (!fs.readFileSync(outFile, "utf8").includes("MML")) {
      console.error("sdl vendor: 补丁未落到 " + file + "（找不到 MML 标记）");
      process.exit(1);
    }
  }
  console.log("sdl vendor: 已应用 " + name + "（" + touched.length + " 个文件）");
}

fs.writeFileSync(stampFile, stamp, "utf8");
console.log("sdl vendor: 完成 " + dest);
