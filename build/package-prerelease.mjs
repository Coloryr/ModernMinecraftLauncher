#!/usr/bin/env node
// 收集预发布构建的可执行文件，按 平台 / 架构 / 版本 命名后复制到 build_out/。
//
// 预发布只出可执行文件，不打安装包：安装包元数据要求纯数字版本，放不下 -pre 这类后缀。
// 用法：node build/package-prerelease.mjs --os windows --arch x64
// 交叉构建（bash 版带 MML_TARGETS）时产物按三元组命名，用 --triple 指定。
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveVersion } from "./version.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/**
 * 取命令行参数的值
 *
 * - `name`: 参数名，如 `--os`
 *
 * # 返回值
 *
 * 参数值；没给返回空串
 */
function arg(name) {
    const index = process.argv.indexOf(name);
    return index >= 0 ? (process.argv[index + 1] ?? "") : "";
}

const os = arg("--os");
const arch = arg("--arch");
/** 交叉构建时的三元组；不填就用按架构命名的产物 */
const triple = arg("--triple");
const systems = ["windows", "linux", "macos"];
const arches = ["x64", "x86", "arm64"];
if (!systems.includes(os) || !arches.includes(arch)) {
    console.error("用法：node build/package-prerelease.mjs --os windows|linux|macos --arch x64|x86|arm64 [--triple 三元组]");
    process.exit(1);
}

let version;
try {
    version = resolveVersion();
} catch (error) {
    console.error(error.message);
    process.exit(1);
}

// build-prerelease.ps1 按架构命名；bash 版构建当前架构时产物叫 mml-gui，
// 用 MML_TARGETS 交叉构建时按三元组命名
const srcDir = path.join(root, "target", "prerelease");
const candidates = (os === "windows"
    ? [triple && `mml-gui-${triple}.exe`, `mml-gui-${arch}.exe`, "mml-gui.exe"]
    : [triple && `mml-gui-${triple}`, `mml-gui-${arch}`, "mml-gui"]).filter(Boolean);
const src = candidates.map((name) => path.join(srcDir, name)).find((file) => fs.existsSync(file));
if (!src) {
    console.error(`找不到预发布产物：${candidates.join(" / ")}（目录 ${path.relative(root, srcDir)}）`);
    process.exit(1);
}

// Windows 可执行文件带 .exe；Linux / macOS 保持无后缀，下载后需要 chmod +x
const ext = os === "windows" ? ".exe" : "";
const outDir = path.join(root, "build_out");
fs.mkdirSync(outDir, { recursive: true });
const dest = path.join(outDir, `mml-${os}-${arch}-${version}${ext}`);
fs.copyFileSync(src, dest);
if (os !== "windows") {
    fs.chmodSync(dest, 0o755);
}

console.log(path.relative(root, src) + " -> " + path.relative(root, dest));
