// 把 src/ 打成可导入启动器的整页包（iframe 路线）
//
// 包根是 index.html：启动器用 mml-ui 协议伺服它并注入 window.mml，前端由
// App.vue 的 CustomUiPage 开 iframe。包里没有 custom.js，不会走窗口组件回调。
//
// 打包手段（零依赖，哪个可用用哪个）：
//   1. bsdtar：Windows 10+ 自带的 tar.exe、macOS 自带的 tar 都是 bsdtar；
//   2. PowerShell 的 Compress-Archive：Windows 一定有。
import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(root, "src");
const stage = join(root, "dist");
const zipPath = join(root, "custom-ui-html-demo.zip");

for (const name of ["index.html", "theme.json"]) {
  if (!existsSync(join(src, name))) {
    console.error("✗ 找不到 src/" + name + "：整页包的入口与清单都需要手写");
    process.exit(1);
  }
}

/** 各系统的目录标记与缩略图缓存：不打包，也不在包里留目录 */
const JUNK_NAMES = new Set([
  ".ds_store", ".localized", ".apdisk", ".directory",
  "thumbs.db", "ehthumbs.db", "desktop.ini",
  ".spotlight-v100", ".trashes", ".fseventsd", ".temporaryitems",
  "$recycle.bin", "system volume information", "lost+found",
]);
const JUNK_PREFIXES = ["._", ".trash-"];

/** 名字是不是系统垃圾，大小写不敏感 */
function isJunk(name) {
  const lower = name.toLowerCase();
  return JUNK_NAMES.has(lower) || JUNK_PREFIXES.some((prefix) => lower.startsWith(prefix)) || name === "Icon\r";
}

/** 递归删掉系统垃圾，PowerShell 兜底打包时也不会带上 */
function stripJunk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (isJunk(entry.name)) {
      rmSync(path, { recursive: entry.isDirectory(), force: true });
    } else if (entry.isDirectory()) {
      stripJunk(path);
    }
  }
}

const manifest = JSON.parse(readFileSync(join(src, "theme.json"), "utf8"));
if (manifest.type !== "page") {
  console.error("✗ src/theme.json 的 type 是 " + manifest.type + "，整页包必须是 page");
  process.exit(1);
}

// 整页包的包根就是 src/ 的内容；assets 之类的新文件不用改脚本
rmSync(stage, { recursive: true, force: true });
cpSync(src, stage, { recursive: true });

// 系统垃圾不进包
stripJunk(stage);
const entries = readdirSync(stage).filter((name) => name !== "__MACOSX" && !isJunk(name));
rmSync(zipPath, { force: true });

function run(cmd, args) {
  const res = spawnSync(cmd, args, { stdio: "inherit" });
  if (res.error) return false;
  return res.status === 0;
}

function isRunnable(cmd, probeArgs) {
  const res = spawnSync(cmd, probeArgs, { stdio: "ignore" });
  return !res.error && res.status === 0;
}

function packWithTar(bin) {
  return run(bin, ["-a", "-c", "-f", zipPath, "--exclude=.DS_Store", "-C", stage, ...entries]);
}

function psQuote(text) {
  return "'" + text.replace(/'/g, "''") + "'";
}

function packWithPowerShell(bin) {
  const script = [
    "$ErrorActionPreference = 'Stop'",
    "Compress-Archive -Path " + psQuote(join(stage, "*")) + " -DestinationPath " + psQuote(zipPath) + " -Force",
  ].join("; ");
  return run(bin, ["-NoProfile", "-NonInteractive", "-Command", script]);
}

let tool = "";
const tarBin = process.platform === "win32" ? "tar.exe" : "tar";
if (isRunnable(tarBin, ["--version"]) && packWithTar(tarBin)) {
  tool = "bsdtar";
} else {
  for (const bin of ["powershell.exe", "pwsh"]) {
    if (!isRunnable(bin, ["-NoProfile", "-Command", "exit 0"])) continue;
    if (packWithPowerShell(bin)) {
      tool = "PowerShell Compress-Archive";
      break;
    }
  }
}

rmSync(stage, { recursive: true, force: true });

if (!tool || !existsSync(zipPath)) {
  console.error("✗ 打包失败：没有可用的 zip 工具。");
  process.exit(1);
}

const kb = (statSync(zipPath).size / 1024).toFixed(1);
console.log("");
console.log("✓ 打包完成（" + tool + "）：" + zipPath);
console.log("  " + entries.length + " 个顶层条目，" + kb + " KB，包根是 index.html / style.css / app.js / theme.json");
console.log("  导入：启动器 → 设置 → 界面 → 主题包，选上面这个 zip");
