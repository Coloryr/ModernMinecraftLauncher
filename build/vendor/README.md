# build/vendor：第三方源码还原

仓库里**不存**第三方源码副本，只存「拉到哪个版本 + 打了什么补丁」。构建前由这里的脚本还原到
`target/vendor/`（已被 git 忽略），Cargo 通过 path 依赖 / `[patch.crates-io]` 使用它们。

| 目录 | 来源 | 产物 | 调用方 |
| --- | --- | --- | --- |
| `tauri-plugin-decoration/prepare.ps1` / `.sh` | GitHub `oovz/tauri-plugin-decoration` 固定 commit | `target/vendor/tauri-plugin-decoration` | `check`、`build/*`、`mml-gui/tauri.*`、CI |
| `sdl3-src/prepare.mjs` | crates.io `sdl3-src-<ver>`（优先用本机 registry 缓存，否则下载并校验 checksum） | `target/vendor/sdl3-src` | 同上 |

cargo 在**解析依赖图时**就要读这些路径下的 `Cargo.toml`，早于任何构建脚本，所以上面的入口都
先跑还原、再执行 cargo；裸 `cargo check` 之前必须先跑过一次。

## SDL 裁剪补丁

`sdl3-src/patches/sdl-trim.patch` 对我们的 SDL 构建做三件事：

1. `SDL/src/dynapi/SDL_dynapi.h`：关掉 `SDL_DYNAMIC_API`。启动器静态链接 SDL、不需要运行时换库，
   关掉后链接器才能丢掉没人引用的 API；
2. `SDL/CMakeLists.txt`：源文件列表去掉 `video` / `render` / `audio` / `gpu` / `camera` /
   `haptic` / `sensor` / `power`（我们只用 joystick / gamepad / hidapi），并强制只编静态库；
3. 新增 `SDL/src/mml_stubs.c`：保留代码里对被移除 API 的引用所需的空实现。

效果：SDL 在 exe 里从约 2.5 MB 降到约 0.9 MB。手柄特性（含 Misc / Paddle / Touchpad 这些额外键）
不变。

**补丁在 Windows 与 Linux 上应用**：两边的引用集合都验证过（Linux 上用空桩链接通过、prerelease
二进制从 52.1 MB 降到 50.3 MB）。macOS 的 IOKit 引用还没验证，默认只还原未打补丁的源码，
需要时可 `MML_SDL_PATCH_ALL=1` 强制打开。

### 升级 SDL 时

1. 改 `sdl3-sys` 版本 → `cargo update -p sdl3-sys`，确认 `Cargo.lock` 里的 `sdl3-src` 版本；
2. 跑 `node build/vendor/sdl3-src/prepare.mjs`，脚本会把新版本源码还原出来并按旧补丁尝试应用；
3. 补丁冲突时按新源码更新 `sdl3-src/patches/sdl-trim.patch`（保持 `MML` 标记，脚本靠它校验补丁是否真的落地）；
4. 链接报 `LNK2019` 时，把缺失的符号补进 `mml_stubs.c` 对应的补丁块，重新生成补丁。
