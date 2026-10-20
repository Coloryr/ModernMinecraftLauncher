// 把 dist-theme/ 打成可导入启动器的整页包
//
// 只包含窗口组件回调：custom.js（IIFE）+ custom.css（它的样式）。没有 index.html，
// 启动器会走 window.__mmlWindowPages 回调，不再用 iframe。
//
// 打包手段（零依赖，哪个可用用哪个）：
//   1. bsdtar：Windows 10+ 自带的 tar.exe、macOS 自带的 tar 都是 bsdtar；
//   2. PowerShell 的 Compress-Archive：Windows 一定有。
import { spawnSync } from "node:child_process";
import { copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const themeDist = join(root, "dist-theme");
const stage = join(root, "dist");
const zipPath = join(root, "mml-ui-demo-custom.zip");

if (!existsSync(join(themeDist, "custom.js"))) {
  console.error("✗ 找不到 dist-theme/custom.js：先跑 npm run build:only");
  process.exit(1);
}

// 组装 zip 根目录：custom.js + custom.css + 可能存在的 assets
rmSync(stage, { recursive: true, force: true });
mkdirSync(stage, { recursive: true });
copyFileSync(join(themeDist, "custom.js"), join(stage, "custom.js"));
const css = readdirSync(themeDist).find((name) => name.endsWith(".css"));
if (css) copyFileSync(join(themeDist, css), join(stage, "custom.css"));
if (existsSync(join(themeDist, "assets"))) {
  cpSync(join(themeDist, "assets"), join(stage, "assets"), { recursive: true });
}
// 包根主题清单：导入时按它判定类型，免得包里带了 custom.css 就被当成样式包
writeFileSync(
  join(stage, "theme.json"),
  JSON.stringify({ type: "page", name: "mml-ui-demo-custom", version: "1.0.0", api: 1 }, null, 2) + "\n",
);

const entries = readdirSync(stage).filter((name) => name !== "__MACOSX");
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
  return run(bin, ["-a", "-c", "-f", zipPath, "-C", stage, ...entries]);
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
console.log("  " + entries.length + " 个顶层条目，" + kb + " KB，包根是 theme.json / custom.js / custom.css");
console.log("  导入：启动器 → 设置 → 界面 → 主题包，选上面这个 zip");
