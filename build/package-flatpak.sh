#!/usr/bin/env bash
# 用已构建的 target/release/mml-gui 打 Flatpak 单文件包，不重新编译。
# 只做 OSTree 提交与 bundle，所以不需要装 Flatpak SDK / runtime；运行时要装
# org.freedesktop.Platform（由用户或 CI 自行安装）。
#
# 用法：bash build/package-flatpak.sh
# 产物：build_out/mml-linux-<arch>-<version>.flatpak

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(tr -d '[:space:]' < "$root/build/version")"
runtime="${MML_FLATPAK_RUNTIME:-25.08}"

if ! command -v flatpak >/dev/null 2>&1; then
    echo '未找到 flatpak，请先安装 flatpak。' >&2
    exit 1
fi

binary="$root/target/release/mml-gui"
if [ ! -f "$binary" ]; then
    echo "找不到 ${binary}，请先跑 build/build-release.sh。" >&2
    exit 1
fi

case "$(uname -m)" in
    x86_64) host_arch=x86_64; label=x64 ;;
    aarch64|arm64) host_arch=aarch64; label=arm64 ;;
    *) echo "不支持的架构：$(uname -m)" >&2; exit 1 ;;
esac

work="$root/target/package/flatpak"
build="$work/build"
rm -rf "$work"
mkdir -p "$build/files/bin" \
    "$build/files/share/applications" \
    "$build/files/share/icons/hicolor/256x256/apps" \
    "$build/files/share/metainfo" \
    "$build/export/share/applications" \
    "$build/export/share/icons/hicolor/256x256/apps" \
    "$build/var"

install -m755 "$binary" "$build/files/bin/mml-gui"
install -m644 "$root/build/linux/com.coloryr.mml.desktop" \
    "$build/files/share/applications/com.coloryr.mml.desktop"
install -m644 "$root/build/linux/com.coloryr.mml.metainfo.xml" \
    "$build/files/share/metainfo/com.coloryr.mml.metainfo.xml"
install -m644 "$root/mml-gui/src-tauri/icons/128x128@2x.png" \
    "$build/files/share/icons/hicolor/256x256/apps/com.coloryr.mml.png"

# export/ 里必须是普通文件：指向 files/ 的符号链接在安装时会被跳过
install -m644 "$root/build/linux/com.coloryr.mml.desktop" \
    "$build/export/share/applications/com.coloryr.mml.desktop"
install -m644 "$root/mml-gui/src-tauri/icons/128x128@2x.png" \
    "$build/export/share/icons/hicolor/256x256/apps/com.coloryr.mml.png"

cat > "$build/metadata" <<EOF
[Application]
name=com.coloryr.mml
runtime=org.freedesktop.Platform/$host_arch/$runtime
command=mml-gui
EOF

repo="$work/repo"
out="$root/build_out/mml-linux-$label-$version.flatpak"
mkdir -p "$root/build_out"
rm -f "$out"

flatpak build-export --arch="$host_arch" "$repo" "$build" master
flatpak build-bundle --arch="$host_arch" "$repo" "$out" com.coloryr.mml master

echo "已生成：${out#"$root"/}"
