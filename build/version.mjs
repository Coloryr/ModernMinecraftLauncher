#!/usr/bin/env node
// 版本号的唯一来源是 Git 标签，不再维护版本文件：
// HEAD 落在版本标签上就用标签名，否则取最近的可达标签（本地构建、releases/** 分支）。
// CI 的 check-version 用 MML_VERSION 把发版标签传进来，构建机不必带完整标签历史。
//
// 用法：node build/version.mjs 打印版本；set-version 与 package-*.mjs 以模块方式引用。
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** 版本标签的 glob：0.1.0 / v0.1.0 / v0.1.0-pre 都算，testbuild-1 这类不算 */
const TAG_PATTERNS = ["[0-9]*", "v[0-9]*"];

/**
 * 把标签或环境变量里的版本号补齐成三段
 *
 * 标签写 `v1.0` 时清单里要落成 `1.0.0`：少一段会被安装包拼成 1.0.20261028。
 *
 * - `text`: 版本号，可带 v 前缀与 -pre 这类后缀
 *
 * # 返回值
 *
 * 三段版本号，后缀原样保留；格式不对返回空串
 */
export function normalizeVersion(text) {
    const matched = /^v?(\d+(?:\.\d+){0,2})((?:[-+][0-9A-Za-z.-]+)?)$/.exec(text.trim());
    if (!matched) return "";
    const [major, minor = "0", patch = "0"] = matched[1].split(".");
    return `${major}.${minor}.${patch}${matched[2]}`;
}

/**
 * 在仓库里跑一条 git 命令
 *
 * - `args`: 传给 git 的参数
 *
 * # 返回值
 *
 * 命令输出并去掉首尾空白；命令失败或不在 Git 仓库里时返回空串
 */
function git(args) {
    try {
        return execFileSync("git", args, {
            cwd: root,
            encoding: "utf8",
            stdio: ["ignore", "pipe", "ignore"],
        }).trim();
    } catch {
        return "";
    }
}

/**
 * 列出描述 HEAD 的版本标签
 *
 * - `extra`: describe 的附加参数，如 `--exact-match` / `--abbrev=0`
 *
 * # 返回值
 *
 * 标签名数组；一个都匹配不上时是空数组
 */
function describe(extra) {
    const matched = TAG_PATTERNS.flatMap((pattern) => ["--match", pattern]);
    return git(["describe", "--tags", ...extra, ...matched])
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean);
}

/**
 * 解析本次构建的版本
 *
 * 顺序：MML_VERSION 环境变量、HEAD 上的版本标签、最近的可达版本标签。
 *
 * # 返回值
 *
 * 三段版本号，如 `0.1.0-pre`
 *
 * # 异常
 *
 * 三处都拿不到版本号时抛错，提示补标签
 */
export function resolveVersion() {
    const fromEnv = (process.env.MML_VERSION ?? "").trim();
    if (fromEnv) {
        const version = normalizeVersion(fromEnv);
        if (!version) throw new Error(`MML_VERSION 不是版本号：${fromEnv}`);
        return version;
    }

    for (const tag of describe(["--exact-match"])) {
        const version = normalizeVersion(tag);
        if (version) return version;
    }

    for (const tag of describe(["--abbrev=0"])) {
        const version = normalizeVersion(tag);
        if (!version) continue;
        console.warn(`HEAD 不在版本标签上，按最近的标签 ${tag} 构建。`);
        return version;
    }

    throw new Error("仓库里没有版本标签：先打一个（如 v0.1.0）再构建。");
}

// 直接执行时打印版本，供脚本与 bash 取用；被 import 时只导出函数
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    try {
        console.log(resolveVersion());
    } catch (error) {
        console.error(error.message);
        process.exit(1);
    }
}
