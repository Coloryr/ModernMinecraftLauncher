// 把示例打成可导入启动器的 zip
//
// CSS 包约定：**zip 根目录放 custom.css**，图片等静态资源可以放子目录，CSS 里用相对路径
// 引用。入口 custom.js 由 Vite 从 src/replace.ts（含 ReplaceButton.vue）打包，
// 再与 custom.css 一起压进 zip；包内其余条目保持原相对路径。
//
// 打包手段（零依赖，哪个可用用哪个）：
//   1. bsdtar：Windows 10+ 自带的 tar.exe、macOS 自带的 tar 都是 bsdtar，
//      写出的条目用正斜杠，是最标准的 zip；
//   2. PowerShell 的 Compress-Archive：Windows 一定有（条目用反斜杠，
//      zip crate 的 Utf8WindowsPath 两种分隔符都能还原）。
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const zipPath = join(root, "mml-ui-demo-custom-css.zip");

/** 入口打包产物；放在包根，mml-vue 见到它才会加载入口 */
const ENTRY_OUTPUT = join(root, "custom.js");

/** 用 Vite 把入口与 Vue 组件打成 IIFE（Vue 走页面全局 __mmlVue）；失败返回 false */
function buildEntry(): boolean {
  const vite = join(root, "node_modules", "vite", "bin", "vite.js");
  if (!existsSync(vite)) {
    console.error("✗ 没装 vite，先 npm install");
    return false;
  }
  const res = spawnSync(process.execPath, [vite, "build"], { cwd: root, stdio: "inherit" });
  if (res.error || res.status !== 0) {
    console.error("✗ 入口构建失败");
    return false;
  }
  const built = join(root, "dist", "custom.js");
  if (!existsSync(built)) {
    console.error("✗ Vite 没生成 dist/custom.js");
    return false;
  }
  copyFileSync(built, ENTRY_OUTPUT);
  return true;
}

/** 包根主题清单：打包时临时写出，导入时按它判定类型 */
const MANIFEST = join(root, "theme.json");

/** 删掉本次生成的 custom.js 与 dist，不留在工作区 */
function cleanupEntry() {
  rmSync(ENTRY_OUTPUT, { force: true });
  rmSync(join(root, "dist"), { recursive: true, force: true });
}

/** 删掉打包期临时写的清单，不留在源码目录 */
function cleanupManifest() {
  rmSync(MANIFEST, { force: true });
}

// 不进包：工具、说明、依赖与旧包，包里只留 custom.css 和静态资源
const SKIP = new Set(["src", "scripts", "dist", "node_modules", "package.json", "package-lock.json", "README.md", ".gitignore", "vite.config.ts"]);

if (!buildEntry()) {
  cleanupEntry();
  process.exit(1);
}

writeFileSync(
  MANIFEST,
  JSON.stringify({ type: "css", name: "mml-ui-demo-custom-css", version: "1.0.0", api: 1 }, null, 2) + "\n",
);

const entries = readdirSync(root).filter((name) => !SKIP.has(name) && !name.endsWith(".zip"));
if (!entries.includes("custom.css")) {
  console.error("✗ 根目录没有 custom.css，打出来的包启动器认不了");
  cleanupManifest();
  cleanupEntry();
  process.exit(1);
}

// 先删旧包，免得失败时留下上一次的产物让人误以为成功
rmSync(zipPath, { force: true });

/** 简单跑一条命令（stdio 继承，直接看到工具自己的输出） */
function run(cmd: string, args: string[]): boolean {
  const res = spawnSync(cmd, args, { stdio: "inherit" });
  if (res.error) return false;
  return res.status === 0;
}

/** 命令是否存在且能跑（探针输出丢弃） */
function isRunnable(cmd: string, probeArgs: string[]): boolean {
  const res = spawnSync(cmd, probeArgs, { stdio: "ignore" });
  return !res.error && res.status === 0;
}

/** 用 bsdtar 打包；返回是否成功 */
function packWithTar(bin: string): boolean {
  return run(bin, ["-a", "-c", "-f", zipPath, "-C", root, ...entries]);
}

/** PowerShell 单引号字符串转义 */
function psQuote(text: string): string {
  return `'${text.replace(/'/g, "''")}'`;
}

/** 用 PowerShell Compress-Archive 打包；返回是否成功 */
function packWithPowerShell(bin: string): boolean {
  const script = [
    "$ErrorActionPreference = 'Stop'",
    `Compress-Archive -Path ${psQuote(join(root, "*"))} -DestinationPath ${psQuote(zipPath)} -Force`,
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
      "  手工处理：把本目录下的 custom.css 与 custom.js（及静态资源）压成 zip，\n" +
      "  保证压缩包根目录下就是 custom.css，然后导入启动器。",
  );
  cleanupManifest();
  cleanupEntry();
  process.exit(1);
}

const kb = (statSync(zipPath).size / 1024).toFixed(1);
console.log("");
console.log(`✓ 打包完成（${tool}）：${zipPath}`);
console.log(`  ${entries.length} 个顶层条目，${kb} KB，zip 根目录即 theme.json / custom.css 与 custom.js`);
console.log("  导入：启动器 → 设置 → 界面 → 样式包 → 导入样式包");
cleanupManifest();
cleanupEntry();
