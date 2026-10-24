#!/usr/bin/env node
// 收集 Tauri 的安装包，按 build/version 重命名后复制到 build_out/。
// 本地只处理当前系统；Windows / macOS / Linux 各跑一次，由 CI 汇总。
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const versionFile = path.join(root, "build", "version");
const rawVersion = fs.existsSync(versionFile)
    ? fs.readFileSync(versionFile, "utf8").trim()
    : "";

if (!rawVersion) {
    console.error("build/version 为空或不存在。");
    process.exit(1);
}

// 版本文件写 0.0.1；即便误写成 v0.0.1 也按数字版本命名产物
const version = rawVersion.replace(/^v/, "");

// 应用清单必须与发布版本一致，否则安装包元数据和产物名会对不上
const cargoVersion = (fs.readFileSync(
    path.join(root, "mml-gui", "src-tauri", "Cargo.toml"),
    "utf8",
).match(/^version\s*=\s*"([^"]+)"/m) || [])[1];
const tauriVersion = JSON.parse(
    fs.readFileSync(path.join(root, "mml-gui", "src-tauri", "tauri.conf.json"), "utf8"),
).version;
if (cargoVersion !== version || tauriVersion !== version) {
    console.error(
        `版本不一致：build/version=${version}，Cargo.toml=${cargoVersion}，tauri.conf.json=${tauriVersion}`,
    );
    process.exit(1);
}

const bundleDir = path.join(root, "target", "release", "bundle");
const outDir = path.join(root, "build_out");

if (!fs.existsSync(bundleDir)) {
    console.error(`找不到 Tauri 产物目录：${path.relative(root, bundleDir)}`);
    process.exit(1);
}

const os = process.platform === "win32"
    ? "windows"
    : process.platform === "darwin"
        ? "macos"
        : "linux";
const arch = process.arch === "x64" ? "x64" : process.arch === "ia32" ? "x86" : process.arch;

/**
 * 递归列出目录下的文件
 *
 * - `dir`: 要遍历的目录
 *
 * # 返回值
 *
 * 文件绝对路径数组
 */
function walk(dir) {
    const result = [];
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) result.push(...walk(full));
        else result.push(full);
    }
    return result;
}

/**
 * 按当前系统与扩展名给出发布包文件名
 *
 * - `file`: Tauri 产出的文件路径
 *
 * # 返回值
 *
 * 重命名后的文件名；不认识的产物返回 `null`
 */
function releaseName(file) {
    const lower = file.toLowerCase();
    if (os === "windows" && lower.endsWith(".msi")) return `mml-windows-${arch}-${version}.msi`;
    if (os === "windows" && lower.endsWith(".exe")) return `mml-windows-${arch}-${version}-setup.exe`;
    if (os === "linux" && lower.endsWith(".deb")) return `mml-linux-${arch}-${version}.deb`;
    if (os === "linux" && lower.endsWith(".rpm")) return `mml-linux-${arch}-${version}.rpm`;
    if (os === "linux" && lower.endsWith(".appimage")) return `mml-linux-${arch}-${version}.AppImage`;
    if (os === "macos" && lower.endsWith(".dmg")) return `mml-macos-${arch}-${version}.dmg`;
    if (os === "macos" && lower.endsWith(".app.tar.gz")) return `mml-macos-${arch}-${version}.app.tar.gz`;
    return null;
}

fs.mkdirSync(outDir, { recursive: true });
const copied = [];
for (const file of walk(bundleDir)) {
    const name = releaseName(file);
    if (!name) continue;
    const dest = path.join(outDir, name);
    fs.copyFileSync(file, dest);
    copied.push(`${path.relative(root, file)} -> ${path.relative(root, dest)}`);
}

if (!copied.length) {
    console.error(`没有在 ${path.relative(root, bundleDir)} 找到当前平台的安装包。`);
    process.exit(1);
}

for (const line of copied) console.log(line);
console.log(`版本 ${version}：${copied.length} 个产物已输出到 ${path.relative(root, outDir)}/`);
