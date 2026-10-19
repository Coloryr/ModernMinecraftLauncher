// 打包 dist/ 成一个可导入启动器的 zip
//
// 整页自定义界面的入口定位规则：包内**根目录**的 index.html 优先；否则若整包只有一个顶层目录，
// 就取那个目录里的 index.html。所以这里绝不能把 `dist/` 这层目录打进 zip——
// zip 里第一条就是 index.html 本身。
//
// 打包手段（零依赖，哪个可用用哪个）：
//   1. bsdtar：Windows 10+ 自带的 tar.exe、macOS 自带的 tar 都是 bsdtar，
//      写出的条目用正斜杠，是最标准的 zip；
//   2. PowerShell 的 Compress-Archive：Windows 一定有（条目用反斜杠，
//      启动器解包走的是 zip crate 的 Utf8WindowsPath，两种分隔符都能还原目录结构）。
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, rmSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const zipPath = join(root, "mml-ui-demo-custom.zip");

if (!existsSync(dist)) {
  console.error("✗ 找不到 dist/：先跑 `npm run build:only`（或 vite build）再打包");
  process.exit(1);
}

// dist 下的顶层条目（index.html、assets/ …）就是 zip 的顶层条目
const entries = readdirSync(dist).filter((name) => name !== "__MACOSX");
if (!entries.includes("index.html")) {
  console.error("✗ dist/ 根目录没有 index.html，打出来的包启动器认不了");
  process.exit(1);
}

// 先删旧包，免得失败时留下上一次的产物让人误以为成功
rmSync(zipPath, { force: true });

/** 简单跑一条命令（stdio 继承，直接看到工具自己的输出） */
function run(cmd, args) {
  const res = spawnSync(cmd, args, { stdio: "inherit" });
  if (res.error) return false;
  return res.status === 0;
}

/** 命令是否存在且能跑（探针输出丢弃，别在打包日志里刷一屏版本号） */
function isRunnable(cmd, probeArgs) {
  const res = spawnSync(cmd, probeArgs, { stdio: "ignore" });
  return !res.error && res.status === 0;
}

/** 用 bsdtar 打包；返回是否成功 */
function packWithTar(bin) {
  // -a 按扩展名选格式（.zip）；-C 切到 dist，后面全是相对条目，保证 zip 根目录干净
  return run(bin, ["-a", "-c", "-f", zipPath, "-C", dist, ...entries]);
}

/** PowerShell 单引号字符串转义 */
function psQuote(text) {
  return `'${text.replace(/'/g, "''")}'`;
}

/** 用 PowerShell Compress-Archive 打包；返回是否成功 */
function packWithPowerShell(bin) {
  const script = [
    "$ErrorActionPreference = 'Stop'",
    `Compress-Archive -Path ${psQuote(join(dist, "*"))} -DestinationPath ${psQuote(zipPath)} -Force`,
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

if (!tool || !existsSync(zipPath)) {
  console.error(
    "✗ 打包失败：这台机器上既没有能写 zip 的 tar，也没有 PowerShell。\n" +
      "  手工处理：把 dist/ 里的**内容**（不是 dist 目录本身）压成 zip，\n" +
      "  保证压缩包根目录下就是 index.html，然后导入启动器。",
  );
  process.exit(1);
}

const kb = (statSync(zipPath).size / 1024).toFixed(1);
console.log("");
console.log(`✓ 打包完成（${tool}）：${zipPath}`);
console.log(`  ${entries.length} 个顶层条目，${kb} KB，zip 根目录即 index.html`);
console.log("  导入：启动器 → 设置 → 界面 → 自定义界面 → 整页重写，选上面这个 zip（后端接入后）");
