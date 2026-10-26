#!/usr/bin/env node
// 收集 Tauri 的安装包，按 build/version 命名后复制到 build_out/。
//
// 支持多架构：Tauri 用 --target 构建时产物在 target/<三元组>/release/bundle/，
// 宿主直接构建的产物在 target/release/bundle/。两处都扫，按目录里的三元组
// 判断产物架构，文件名带上 x64 / x86 / arm64。
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
        "版本不一致：build/version=" + version + "，Cargo.toml=" + cargoVersion + "，tauri.conf.json=" + tauriVersion,
    );
    process.exit(1);
}

const targetDir = path.join(root, "target");
const outDir = path.join(root, "build_out");
const hostArch = process.arch === "x64" ? "x64" : process.arch === "ia32" ? "x86" : process.arch === "arm64" ? "arm64" : process.arch;
const os = process.platform === "win32"
    ? "windows"
    : process.platform === "darwin"
        ? "macos"
        : "linux";

/**
 * 从 target 下的目录名判断产物架构
 *
 * - triple: target/ 下的目录名，宿主直接构建时是 release
 *
 * # 返回值
 *
 * x64 / x86 / arm64 / universal；认不出时原样返回目录名
 */
function archOf(triple) {
    if (triple === "release") return hostArch;
    if (triple.includes("universal")) return "universal";
    if (triple.startsWith("x86_64") || triple.startsWith("amd64")) return "x64";
    if (/^i[3-6]86/.test(triple) || triple.startsWith("x86-")) return "x86";
    if (triple.startsWith("aarch64") || triple.startsWith("arm64")) return "arm64";
    return triple;
}

/** 找出所有 Tauri 产物目录：宿主一份 + 每个 --target 一份 */
function bundleDirs() {
    const found = [];
    if (!fs.existsSync(targetDir)) return found;
    // 先收 --target 的目录，再收宿主目录：同一架构两份时优先用按三元组构建的新产物
    for (const entry of fs.readdirSync(targetDir, { withFileTypes: true })) {
        if (!entry.isDirectory() || entry.name === "release") continue;
        const dir = path.join(targetDir, entry.name, "release", "bundle");
        if (fs.existsSync(dir)) found.push({ dir, triple: entry.name });
    }
    const host = path.join(targetDir, "release", "bundle");
    if (fs.existsSync(host)) found.push({ dir: host, triple: "release" });
    return found;
}

/**
 * 递归列出目录下的文件
 *
 * - dir: 要遍历的目录
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
 * 按系统、架构与扩展名给出发布包文件名
 *
 * - file: Tauri 产出的文件路径
 * - arch: 该产物所属架构
 *
 * # 返回值
 *
 * 重命名后的文件名；不认识的产物返回 null
 */
function releaseName(file, arch) {
    const lower = file.toLowerCase();
    const base = "mml-" + os + "-" + arch + "-" + version;
    if (os === "windows" && lower.endsWith(".msi")) return base + ".msi";
    if (os === "windows" && lower.endsWith(".exe")) return base + "-setup.exe";
    if (os === "linux" && lower.endsWith(".deb")) return base + ".deb";
    if (os === "linux" && lower.endsWith(".rpm")) return base + ".rpm";
    if (os === "linux" && lower.endsWith(".appimage")) return base + ".AppImage";
    if (os === "macos" && lower.endsWith(".dmg")) return base + ".dmg";
    if (os === "macos" && lower.endsWith(".app.tar.gz")) return base + ".app.tar.gz";
    return null;
}

const dirs = bundleDirs();
if (!dirs.length) {
    console.error("找不到 Tauri 产物目录：target/release/bundle 或 target/<三元组>/release/bundle");
    process.exit(1);
}

fs.mkdirSync(outDir, { recursive: true });
const copied = new Map();
for (const item of dirs) {
    const arch = archOf(item.triple);
    for (const file of walk(item.dir)) {
        const name = releaseName(file, arch);
        if (!name) continue;
        if (copied.has(name)) {
            console.warn("跳过重复产物 " + name + "（已从 " + path.relative(root, copied.get(name)) + " 收集）");
            continue;
        }
        copied.set(name, file);
        const dest = path.join(outDir, name);
        fs.copyFileSync(file, dest);
        console.log(path.relative(root, file) + " -> " + path.relative(root, dest));
    }
}

if (!copied.size) {
    console.error("没有在 " + path.relative(root, targetDir) + " 下找到当前平台的安装包。");
    process.exit(1);
}

console.log("版本 " + version + "：" + copied.size + " 个产物已输出到 " + path.relative(root, outDir) + "/");
