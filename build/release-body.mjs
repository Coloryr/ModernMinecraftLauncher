#!/usr/bin/env node
// 生成 GitHub Release 的发布说明：固定的产物说明 + 每个产物的 SHA1 / SHA256。
// 用法：node build/release-body.mjs --kind release --version v0.0.1 --artifacts artifacts --out body.md
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

/**
 * 取命令行参数的值
 *
 * - `name`: 参数名，如 `--kind`
 *
 * # 返回值
 *
 * 参数值；没给返回空串
 */
function arg(name) {
    const index = process.argv.indexOf(name);
    return index >= 0 ? (process.argv[index + 1] ?? "") : "";
}

const kind = arg("--kind");
const version = arg("--version");
const artifacts = path.resolve(arg("--artifacts") || "artifacts");
const out = path.resolve(arg("--out"));

if (!["release", "prerelease"].includes(kind) || !version || !arg("--out")) {
    console.error(
        "用法：node build/release-body.mjs --kind release|prerelease --version <标签> --artifacts <目录> --out <文件>",
    );
    process.exit(1);
}
if (!fs.existsSync(artifacts)) {
    console.error(`产物目录不存在：${artifacts}`);
    process.exit(1);
}

/**
 * 递归列出目录下的所有文件
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
 * 计算一个文件的 SHA1 与 SHA256
 *
 * - `file`: 文件路径
 *
 * # 返回值
 *
 * `{ sha1, sha256 }`，小写十六进制
 */
function checksum(file) {
    const data = fs.readFileSync(file);
    return {
        sha1: crypto.createHash("sha1").update(data).digest("hex"),
        sha256: crypto.createHash("sha256").update(data).digest("hex"),
    };
}

const files = walk(artifacts).sort();
if (!files.length) {
    console.error(`${artifacts} 下没有产物。`);
    process.exit(1);
}

const lines = [`M²L ${version} ${kind === "prerelease" ? "预发布构建" : "构建产物"}`, ""];
if (kind === "release") {
    lines.push("- Windows：x64 / x86 / arm64（MSI / NSIS；arm64 为 NSIS）");
    lines.push("- Linux：x64 / arm64（deb / rpm / AppImage；x64 另有 Flatpak / Arch），x86 只出 deb / rpm");
    lines.push("- macOS：x64 / arm64（dmg / app.tar.gz）");
} else {
    lines.push("- 预发布只提供可执行文件，不打安装包；主界面会提示「预发布版」。");
    lines.push("- Windows：x64 / x86 / arm64（.exe）");
    lines.push("- Linux：x64 / arm64 / x86（下载后 `chmod +x`）");
    lines.push("- macOS：x64 / arm64（下载后 `chmod +x`）");
}
lines.push("");
lines.push("### 校验和");
lines.push("");
lines.push("| 文件 | SHA1 | SHA256 |");
lines.push("| --- | --- | --- |");
for (const file of files) {
    const { sha1, sha256 } = checksum(file);
    lines.push(`| \`${path.basename(file)}\` | \`${sha1}\` | \`${sha256}\` |`);
}
lines.push("");

fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, lines.join("\n"), "utf8");
console.log(`发布说明已生成：${out}`);
