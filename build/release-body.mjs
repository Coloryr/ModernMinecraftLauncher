#!/usr/bin/env node
// 生成 GitHub Release 的发布说明：更新日志取自 docs/changelog.md，
// 后面附固定的产物说明与每个产物的 SHA1 / SHA256。
// 用法：node build/release-body.mjs --kind release --version v0.0.1 --artifacts artifacts --out body.md
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { normalizeVersion, packageVersionOf } from "./version.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const changelogFile = path.join(root, "docs", "changelog.md");

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
const tag = arg("--version");
const artifacts = path.resolve(arg("--artifacts") || "artifacts");
const out = path.resolve(arg("--out"));

if (!["release", "prerelease"].includes(kind) || !tag || !arg("--out")) {
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
 * 从更新日志里取出某个版本的正文
 *
 * 版本标题形如 `## 0.1.7`，后面可以带「（开发中）」这类说明；取到下一个二级标题为止。
 *
 * - `version`: 三段版本号，如 `0.1.7`
 *
 * # 返回值
 *
 * 该版本的正文行，已去掉标题与首尾空行
 *
 * # 异常
 *
 * 更新日志文件不存在、没有该版本或正文为空时抛错
 */
function changelogOf(version) {
    if (!fs.existsSync(changelogFile)) {
        throw new Error(`找不到更新日志：${changelogFile}`);
    }
    const lines = fs.readFileSync(changelogFile, "utf8").replace(/\r\n/g, "\n").split("\n");
    const start = lines.findIndex((line) => {
        if (!line.startsWith("## ")) return false;
        const head = line.slice(3).trimStart().replace(/^v/, "");
        if (!head.startsWith(version)) return false;
        const next = head.charAt(version.length);
        return next === "" || next === " " || next === "（" || next === "(";
    });
    if (start < 0) {
        const known = lines.filter((line) => line.startsWith("## ")).map((line) => line.trim());
        throw new Error(
            `docs/changelog.md 里没有 ${version} 的更新日志，请补上「## ${version}」。已有：\n${known.join("\n")}`,
        );
    }
    const body = [];
    for (const line of lines.slice(start + 1)) {
        if (line.startsWith("## ")) break;
        body.push(line);
    }
    while (body.length && !body[0].trim()) body.shift();
    while (body.length && !body[body.length - 1].trim()) body.pop();
    if (!body.length) {
        throw new Error(`docs/changelog.md 里 ${version} 的更新日志是空的。`);
    }
    return body;
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

// 标签可能带 v 前缀与 -pre 后缀，更新日志的标题只写三段版本号
const docVersion = packageVersionOf(normalizeVersion(tag) || tag.replace(/^v/, ""));

let changelog;
try {
    changelog = changelogOf(docVersion);
} catch (error) {
    console.error(error.message);
    process.exit(1);
}

const files = walk(artifacts).sort();
if (!files.length) {
    console.error(`${artifacts} 下没有产物。`);
    process.exit(1);
}

const lines = [`M²L ${tag} ${kind === "prerelease" ? "预发布构建" : "构建产物"}`, ""];
lines.push("## 更新日志", "", ...changelog, "");
lines.push("## 构建产物", "");
if (kind === "release") {
    lines.push("- Windows：x64 / arm64（MSI / NSIS；arm64 为 NSIS）");
    lines.push("- Linux：x64 / arm64（deb / rpm / AppImage；x64 另有 Flatpak / Arch）");
    lines.push("- macOS：x64 / arm64（dmg / app.tar.gz）");
} else {
    lines.push("- 预发布不打安装包：Windows 只出单个可执行文件，macOS 出自签的 `.app` 压缩包，Linux 与正式版一样出安装包。");
    lines.push("- Windows：x64 / arm64（直接运行的 exe）");
    lines.push("- Linux：x64 / arm64（deb / rpm / AppImage；x64 另有 Flatpak / Arch）");
    lines.push("- macOS：x64 / arm64（解压得到 `.app`，首次打开需在「隐私与安全性」里放行）");
}
lines.push("");
lines.push("## 校验和");
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
console.log(`发布说明已生成：${out}（更新日志取 docs/changelog.md 的 ${docVersion}）`);
