#!/usr/bin/env node
// 把 build/version 同步到应用清单，避免发版时漏改版本。
// 版本由 build-release / build-prerelease 在构建前自动调用。
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const versionFile = path.join(root, "build", "version");
const raw = fs.existsSync(versionFile) ? fs.readFileSync(versionFile, "utf8").trim() : "";

if (!raw) {
    console.error("build/version 为空或不存在。");
    process.exit(1);
}

const version = raw.replace(/^v/, "");
if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
    console.error(`版本号格式不对：${raw}`);
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
    {
        file: "mml-gui/package.json",
        pattern: /"version"\s*:\s*"[^"]+"/,
        value: `"version": "${version}"`,
    },
    {
        file: "mml-gui/package-lock.json",
        pattern: /"name": "mml-gui",(\r?\n)  "version": "[^"]+"/,
        value: `"name": "mml-gui",$1  "version": "${version}"`,
    },
    {
        file: "mml-gui/package-lock.json",
        pattern: /"": \{(\r?\n)      "name": "mml-gui",(\r?\n)      "version": "[^"]+"/,
        value: `"": {$1      "name": "mml-gui",$2      "version": "${version}"`,
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
