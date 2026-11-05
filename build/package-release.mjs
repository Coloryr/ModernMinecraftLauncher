#!/usr/bin/env node
// 收集 Tauri 的安装包，按版本命名后复制到 build_out/。
//
// 支持多架构：Tauri 用 --target 构建时产物在 target/<三元组>/release/bundle/，
// 宿主直接构建的产物在 target/release/bundle/。两处都扫，按目录里的三元组
// 判断产物架构，文件名带上 x64 / x86 / arm64。
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { packageVersionOf, resolveVersion } from "./version.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

let version;
try {
    version = resolveVersion();
} catch (error) {
    console.error(error.message);
    process.exit(1);
}

// 清单里写的是安装包版本（版本去掉 -pre 后缀）；产物名仍用完整版本，预发布才分得清
const packageVersion = packageVersionOf(version);
const cargoVersion = (fs.readFileSync(
    path.join(root, "mml-gui", "src-tauri", "Cargo.toml"),
    "utf8",
).match(/^version\s*=\s*"([^"]+)"/m) || [])[1];
const tauriVersion = JSON.parse(
    fs.readFileSync(path.join(root, "mml-gui", "src-tauri", "tauri.conf.json"), "utf8"),
).version;
if (cargoVersion !== packageVersion || tauriVersion !== packageVersion) {
    console.error(
        "版本不一致：安装包版本=" + packageVersion + "（版本 " + version + "），Cargo.toml=" + cargoVersion + "，tauri.conf.json=" + tauriVersion,
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

/** 找出所有构建输出目录：宿主一份 + 每个 --target 一份，不含 bundle */
function execDirs() {
    const found = [];
    if (!fs.existsSync(targetDir)) return found;
    for (const entry of fs.readdirSync(targetDir, { withFileTypes: true })) {
        if (!entry.isDirectory() || entry.name === "release") continue;
        const dir = path.join(targetDir, entry.name, "release");
        if (fs.existsSync(dir)) found.push({ dir, triple: entry.name });
    }
    const host = path.join(targetDir, "release");
    if (fs.existsSync(host)) found.push({ dir: host, triple: "release" });
    return found;
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
 * 取 Tauri 打出的 macOS .app 目录
 *
 * - `dir`: Tauri 产物目录，形如 target/<三元组>/release/bundle
 *
 * # 返回值
 *
 * .app 的绝对路径；没有时返回 null
 */
function appBundleOf(dir) {
    const macos = path.join(dir, "macos");
    if (!fs.existsSync(macos)) return null;
    const found = fs
        .readdirSync(macos, { withFileTypes: true })
        .filter((entry) => entry.isDirectory() && entry.name.endsWith(".app"));
    return found.length ? path.join(macos, found[0].name) : null;
}

/**
 * 给 .app 补一道 ad-hoc 签名
 *
 * 配置了证书时 Tauri 已经签过，这里验过就保留；没签过才补 `--sign -`，
 * 免得把证书签名覆盖成 ad-hoc。
 *
 * - `app`: .app 的绝对路径
 */
function signApp(app) {
    try {
        execFileSync("codesign", ["--verify", "--deep", "--verbose=1", app], { stdio: "inherit" });
        return;
    } catch {
        console.warn("签名缺失或无效，补一道 ad-hoc 签名：" + path.relative(root, app));
    }
    execFileSync("codesign", ["--force", "--deep", "--sign", "-", app], { stdio: "inherit" });
    execFileSync("codesign", ["--verify", "--deep", "--verbose=1", app], { stdio: "inherit" });
}

/**
 * 把 .app 压成 zip
 *
 * 用 ditto 而不是 zip：它保留 .app 里的符号链接、扩展属性与可执行位。
 *
 * - `app`: .app 的绝对路径
 * - `dest`: zip 的绝对路径
 */
function zipApp(app, dest) {
    execFileSync("ditto", ["-c", "-k", "--sequesterRsrc", "--keepParent", app, dest], { stdio: "inherit" });
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
const copied = new Map();
fs.mkdirSync(outDir, { recursive: true });

for (const item of dirs) {
    const arch = archOf(item.triple);
    const files = walk(item.dir);
    // macOS 预发布只打 .app（没有 dmg）：自签后压成 zip，不发裸可执行文件
    if (os === "macos" && !files.some((file) => file.toLowerCase().endsWith(".dmg"))) {
        const app = appBundleOf(item.dir);
        if (app) {
            const name = "mml-macos-" + arch + "-" + version + ".app.zip";
            if (!copied.has(name)) {
                const dest = path.join(outDir, name);
                signApp(app);
                zipApp(app, dest);
                copied.set(name, app);
                console.log(path.relative(root, app) + " -> " + path.relative(root, dest));
            }
            continue;
        }
    }
    for (const file of files) {
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

// 不打包的预发布（Windows）只留可执行文件，产物在 target/<三元组>/release 下
if (!copied.size) {
    const ext = os === "windows" ? ".exe" : "";
    const candidates = ["mml-gui" + ext, "ModernMinecraftLauncher" + ext];
    for (const item of execDirs()) {
        const src = candidates.map((name) => path.join(item.dir, name)).find((file) => fs.existsSync(file));
        if (!src) continue;
        const name = "mml-" + os + "-" + archOf(item.triple) + "-" + version + ext;
        if (copied.has(name)) continue;
        copied.set(name, src);
        const dest = path.join(outDir, name);
        fs.copyFileSync(src, dest);
        console.log(path.relative(root, src) + " -> " + path.relative(root, dest));
    }
    // macOS 的可执行文件下载后要能直接跑
    for (const name of copied.keys()) fs.chmodSync(path.join(outDir, name), 0o755);
}

if (!copied.size) {
    console.error(
        "没有在 " + path.relative(root, targetDir) + " 下找到安装包或可执行文件（找过 target/release、target/<三元组>/release）",
    );
    process.exit(1);
}

console.log("版本 " + version + "：" + copied.size + " 个产物已输出到 " + path.relative(root, outDir) + "/");
