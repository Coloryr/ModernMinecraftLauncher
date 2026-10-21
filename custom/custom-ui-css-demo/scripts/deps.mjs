// 依赖来源：优先用仓库里的本地包，仓库不在时克隆一份
//
// package.json 里 mml-ipc / mml-vue 写的是 file:.mml-deps/<名字>，本脚本在 npm 解析依赖
// 之前把它们铺好：上级目录有仓库就直接链过去，没有就浅克隆到 .mml-deps/repo 再链。
// 三个示例各存一份，单独拷走一个示例目录也能装起来。

import { execFileSync } from "node:child_process";
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** 示例目录，.mml-deps 与 package.json 都相对它 */
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
/** 链接落点，已被 .gitignore 忽略 */
const DEPS = join(ROOT, ".mml-deps");
/** 仓库根：示例在 custom/<示例>/ 下，往上两层就是仓库 */
const REPO = resolve(ROOT, "..", "..");
/** 仓库不在时克隆到这里，多个包共用一份 */
const CLONE = join(DEPS, "repo");
/** 仓库地址；内网镜像或 fork 可以用 MML_REPO_URL 覆盖 */
const REPO_URL = process.env.MML_REPO_URL || "https://github.com/Coloryr/ModernMinecraftLauncher.git";
/** package.json 里指向 .mml-deps 的前缀，据此认出要铺哪些包 */
const SPEC_PREFIX = "file:.mml-deps/";
/** 克隆时不需要的东西 */
const SKIP = new Set(["node_modules", ".git", "dist", "dist-theme", "target"]);

/** 读出 package.json 里要铺的包名 */
function wantedPackages() {
  const pkg = JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8"));
  const deps = { ...pkg.dependencies, ...pkg.devDependencies };
  return Object.entries(deps)
    .filter(([, spec]) => typeof spec === "string" && spec.startsWith(SPEC_PREFIX))
    .map(([name]) => name);
}

/** 目录里是否有可用的包 */
function hasPackage(dir) {
  return existsSync(join(dir, "package.json"));
}

/** 删掉已有落点，悬空的链接也一并清掉 */
function clear(target) {
  try {
    lstatSync(target);
  } catch {
    return;
  }
  rmSync(target, { recursive: true, force: true });
}

/**
 * 把 source 链到 target
 *
 * - `source`: 真实的包目录
 * - `target`: .mml-deps 下的落点
 *
 * 链接失败时退化成复制，保证安装能继续。
 */
function linkTo(source, target) {
  clear(target);
  try {
    symlinkSync(source, target, process.platform === "win32" ? "junction" : "dir");
    return "link";
  } catch {
    // 复制时跳过 node_modules / .git 这些，避免把仓库整份搬过来
    cpSync(source, target, {
      recursive: true,
      filter: (from) => {
        if (from === source) return true;
        return !SKIP.has(from.slice(source.length + 1).split(/[\\/]/)[0]);
      },
    });
    return "copy";
  }
}

/**
 * 浅克隆仓库，只取需要的包
 *
 * - `names`: 要检出的包目录名
 *
 * 走 sparse checkout：整仓有测试用的长路径样本，Windows 上全量检出会撞 MAX_PATH。
 */
function ensureClone(names) {
  if (hasPackage(CLONE)) return;
  mkdirSync(DEPS, { recursive: true });
  clear(CLONE);
  const args = [];
  // core.longpaths 只在 Windows 上有意义，其余平台传了也会被忽略
  if (process.platform === "win32") args.push("-c", "core.longpaths=true");
  args.push("clone", "--depth", "1", "--sparse", REPO_URL, CLONE);
  try {
    execFileSync("git", args, { stdio: "inherit" });
    execFileSync("git", ["-C", CLONE, "sparse-checkout", "set", ...names], { stdio: "inherit" });
  } catch {
    clear(CLONE);
    console.error("✗ 克隆失败：" + REPO_URL);
    console.error("  确认已装 git 且能访问网络，或手动把仓库放到 " + REPO);
    process.exit(1);
  }
}

function main() {
  const names = wantedPackages();
  if (names.length === 0) return;

  mkdirSync(DEPS, { recursive: true });
  const repoHere = names.every((name) => hasPackage(join(REPO, name)));
  if (!repoHere && !hasPackage(CLONE)) {
    console.log("· 上级目录没有 " + names.join(" / ") + "，先浅克隆仓库到 .mml-deps/repo");
    ensureClone(names);
  }

  for (const name of names) {
    const target = join(DEPS, name);
    if (hasPackage(target)) continue;

    const local = join(REPO, name);
    const source = hasPackage(local) ? local : join(CLONE, name);
    if (!hasPackage(source)) {
      console.error("✗ 找不到 " + name + "：" + source);
      process.exit(1);
    }

    const how = linkTo(source, target);
    console.log("· " + name + " ← " + source + (how === "copy" ? "（复制）" : ""));
  }
}

main();
