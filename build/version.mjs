#!/usr/bin/env node
// 版本号的唯一来源是仓库里的版本文件 docs/version：一行纯数字版本，如 0.1.9。
// 预览版由发版标签的 -pre 后缀表示，不写进文件；构建前由 set-version 同步进应用清单。
// MML_VERSION 环境变量可以临时覆盖，CI 用它把带后缀的版号传给各构建机。
//
// 用法：node build/version.mjs 打印版本；--package 打印安装包版本；
//       --tag v0.1.9-pre 把标签解析成版本号（CI 校验标签与文件是否一致）。
// set-version 与 package-*.mjs 以模块方式引用这里的函数。
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** 版本文件，相对仓库根 */
const VERSION_FILE = "docs/version";

/**
 * 把版本号补齐成三段
 *
 * 版本文件写 `1.0` 时清单里要落成 `1.0.0`：少一段会被安装包拼成 1.0.20261028。
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
 * 读版本文件
 *
 * 文件只写数字版本；带 `-pre` 这类后缀说明写错了位置——后缀该写在发版标签上。
 *
 * # 返回值
 *
 * 三段版本号，如 `0.1.9`
 *
 * # 异常
 *
 * 文件不存在、内容不是版本号，或带后缀时抛错
 */
function readVersionFile() {
    let text;
    try {
        text = fs.readFileSync(path.join(root, VERSION_FILE), "utf8").trim();
    } catch {
        throw new Error(`读不到版本文件：${VERSION_FILE}`);
    }
    const version = normalizeVersion(text);
    if (!version) throw new Error(`${VERSION_FILE} 不是版本号：${text}`);
    if (version !== packageVersionOf(version)) {
        throw new Error(`${VERSION_FILE} 只写数字版本，预览版后缀写在发版标签上：${text}`);
    }
    return version;
}

/**
 * 解析本次构建的版本
 *
 * 顺序：MML_VERSION 环境变量、版本文件。
 *
 * # 返回值
 *
 * 三段版本号，如 `0.1.9` 或 `0.1.9-pre`
 *
 * # 异常
 *
 * 版本文件缺失、内容不是版本号，或 MML_VERSION 不是版本号时抛错
 */
export function resolveVersion() {
    const fromEnv = (process.env.MML_VERSION ?? "").trim();
    if (fromEnv) {
        const version = normalizeVersion(fromEnv);
        if (!version) throw new Error(`MML_VERSION 不是版本号：${fromEnv}`);
        return version;
    }
    return readVersionFile();
}

/**
 * 把版本收成安装包能接受的形式
 *
 * Windows 安装包要求版本纯数字，`0.1.1-pre` 会被 WiX 直接拒绝；
 * 后缀保留用于产物命名与 Release，清单与安装包元数据用去掉后缀的版本。
 *
 * - `version`: 三段版本号，可带 -pre 这类后缀
 *
 * # 返回值
 *
 * 纯数字三段版本号，如 `0.1.1`
 */
export function packageVersionOf(version) {
    return version.replace(/[-+].*$/, "");
}

// 直接执行时按参数输出，供脚本与 bash 取用；被 import 时只导出函数
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    const args = process.argv.slice(2);
    const tagAt = args.indexOf("--tag");
    try {
        if (tagAt >= 0) {
            const tag = (args[tagAt + 1] ?? "").trim();
            const version = normalizeVersion(tag);
            if (!version) throw new Error(`不是版本标签：${tag}`);
            console.log(version);
        } else {
            const version = resolveVersion();
            console.log(args.includes("--package") ? packageVersionOf(version) : version);
        }
    } catch (error) {
        console.error(error.message);
        process.exit(1);
    }
}
