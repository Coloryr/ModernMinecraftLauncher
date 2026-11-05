#!/usr/bin/env node
// 把版本文件里的版本同步到应用清单，安装包元数据只认纯数字版本，所以这里写的是去掉 -pre 后缀的版本。
// 版本由 build-release / build-prerelease 在构建前自动调用。
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { packageVersionOf, resolveVersion } from "./version.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

let version;
try {
    // 版本 0.1.1-pre -> 清单 0.1.1；正式版本来就没有后缀，原样写入
    version = packageVersionOf(resolveVersion());
} catch (error) {
    console.error(error.message);
    process.exit(1);
}

const targets = [
    {
        file: "mml-gui/src-tauri/Cargo.toml",
        pattern: /^version\s*=\s*"[^"]+"/m,
        value: `version = "${version}"`,
    },
    {
        file: "mml-gui/src-tauri/tauri.conf.json",
        pattern: /"version"\s*:\s*"[^"]+"/,
        value: `"version": "${version}"`,
    },
];

for (const target of targets) {
    const abs = path.join(root, target.file);
    const text = fs.readFileSync(abs, "utf8");
    if (!target.pattern.test(text)) {
        console.error(`找不到版本字段：${target.file}`);
        process.exit(1);
    }
    const next = text.replace(target.pattern, target.value);
    if (next !== text) {
        fs.writeFileSync(abs, next, "utf8");
        console.log(`${target.file} -> ${version}`);
    }
}

console.log(`版本已同步：${version}`);
