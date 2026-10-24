#!/usr/bin/env bash
# 拉取 tauri-plugin-decoration 指定版本源码 + 应用本地 patch（prepare.ps1 的 Unix 版）
#
# 为什么这样组织
# ------------------------------------------------------------------
# 我们需要改 tauri-plugin-decoration 的源码（让它接受应用提供的窗口按钮矩形，
# 这样既能保住 M²L 自己的标题栏外观、又保留 Win11 贴靠布局）。
#
# 仓库里**不存**第三方源码副本：只存 patch，构建前由本脚本按**固定 commit**
# 从上游克隆、再打 patch。这样：
#   - 仓库干净，没有大段第三方源码；
#   - 改动一目了然（就是 patches/ 里那几个文件）；
#   - 上游升级只需改 REF 并重放 patch，冲突处有 `M²L 本地修改` 注释可对照。
#
# 用法：
#   bash mml-gui/vendor/prepare.sh            # 幂等，已存在则跳过
#   bash mml-gui/vendor/prepare.sh --force    # 强制重新拉取
#
# 注意：产物放在 target/vendor 下（已被 git 忽略）。cargo 在**解析依赖图时**
# 就要求 path 依赖的 Cargo.toml 存在，所以这个脚本必须在 cargo 之前跑 ——
# 各构建入口（build-*.sh / npm 脚本）都已接好。

set -euo pipefail

# 上游仓库与版本。REF 用 **commit**（不是 tag）：tag 可被移动，commit 不会
REPO="https://github.com/oovz/tauri-plugin-decoration.git"
REF="235272b40d9a7b840ad49944e9fef334fde251ec"   # = v3.0.5
DIR_NAME="tauri-plugin-decoration"

VENDOR_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# <repo>/mml-gui/vendor → 上溯两层到仓库根
REPO_ROOT="$(cd "$VENDOR_DIR/../.." && pwd)"
PATCH_DIR="$VENDOR_DIR/patches"
OUT_DIR="$REPO_ROOT/target/vendor/$DIR_NAME"

FORCE=0
for arg in "$@"; do
    case "$arg" in
        --force|-f) FORCE=1 ;;
        *) echo "未知参数：$arg" >&2; exit 2 ;;
    esac
done

if [ -f "$OUT_DIR/Cargo.toml" ] && [ "$FORCE" -eq 0 ]; then
    echo "已存在，跳过：$OUT_DIR（要重新拉取加 --force）"
    exit 0
fi

rm -rf "$OUT_DIR"
mkdir -p "$(dirname "$OUT_DIR")"

# 1. 浅克隆到指定 commit 并检出
#    先 init + fetch 指定 commit（比 clone 整个仓库再 checkout 快得多）
echo "拉取上游 $REF ..."
git init --quiet "$OUT_DIR"
git -C "$OUT_DIR" remote add origin "$REPO"
git -C "$OUT_DIR" fetch --quiet --depth 1 origin "$REF"
git -C "$OUT_DIR" checkout --quiet FETCH_HEAD

# 上游源码即可编译，.git 不需要留着（也避免它被当成独立仓库干扰外层 git）
rm -rf "$OUT_DIR/.git"

# 2. 依次应用 patch
shopt -s nullglob
PATCHES=("$PATCH_DIR"/*.patch)
shopt -u nullglob

if [ ${#PATCHES[@]} -eq 0 ]; then
    echo "没有 patch，产物即上游原样：$OUT_DIR"
    exit 0
fi

# 排序，保证应用顺序稳定（与 ps1 版一致）
IFS=$'\n' PATCHES=($(printf '%s\n' "${PATCHES[@]}" | sort))
unset IFS

for p in "${PATCHES[@]}"; do
    name="$(basename "$p")"

    # 逐个文件确认 patch **真的**改了东西。
    # 之前踩过：git apply 报 "Skipped patch" 却退出码 0，什么都没改 —— 靠这个兜住。
    # 不用 mapfile：macOS 自带的 bash 3.2 没有它，CI 上会直接失败
    touched=()
    while IFS= read -r touched_file; do
        touched+=("$touched_file")
    done < <(sed -n 's|^+++ b/||p' "$p")

    # 产物落在 target/ 下 —— 那属于**本仓库**。git apply 在仓库内会以仓库根为
    # 基准解析补丁路径（src/js/... → <repoRoot>/src/js/...，不存在），
    # 而且这种"找不到"是**静默返回 0**的（实测）。所以显式 --directory 指定基准。
    rel="${OUT_DIR#"$REPO_ROOT"/}"

    # 打之前先把补丁的行尾归一化成 LF（理由见 ps1 版同名注释）：上游源码是 LF，
    # 而 Windows 上 core.autocrlf=true 会把仓库里的 *.patch 也检出成 CRLF，
    # 那样 git apply 一行上下文都匹配不上。仓库根 .gitattributes 已钉 eol=lf，
    # 这里再兜一层。中转文件按约定放 target/temp（见 AGENTS.md §6）。
    tmp_dir="$REPO_ROOT/target/temp"
    mkdir -p "$tmp_dir"
    tmp_patch="$tmp_dir/$name"
    sed 's/\r$//' "$p" > "$tmp_patch"

    if ! result="$(cd "$REPO_ROOT" && git apply --whitespace=nowarn --directory="$rel" "$tmp_patch" 2>&1)"; then
        echo "patch 应用失败：$name" >&2
        echo "$result" >&2
        exit 1
    fi

    for r in "${touched[@]}"; do
        out_file="$OUT_DIR/$r"
        if [ ! -f "$out_file" ]; then
            echo "patch $name 声明的文件不存在于产物：$r" >&2
            exit 1
        fi
        # 补丁后文件里必须留下标记，否则说明补丁没真正落地
        if ! grep -q 'M²L 本地修改' "$out_file"; then
            echo "patch $name 未落到 $r（找不到 M²L 本地修改 标记）" >&2
            exit 1
        fi
    done

    echo "已应用：$name（改动 ${#touched[@]} 个文件）"
done

echo "完成：$OUT_DIR"
