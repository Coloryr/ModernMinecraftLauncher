# task1 — 从依赖入手优化 exe 体积

## 1. 现状数据

- 基线：`target/release/mml-gui.exe` = **30.8 MB**；`target/debug/mml-gui.exe` = 83.2 MB。
- 按 Windows 目标过滤后，exe 里静态链接 **477 个 crate**，其中 **38 个名字出现两个及以上版本**。
- 发布 profile 已经压到底了：`lto = true` + `codegen-units = 1` + `opt-level = 3` + `panic = "abort"` + `strip = true`。
  **继续在 profile 上做文章收益很小**，体积只剩"少链几个 crate"这一条路。
- 所以本任务的全部动作都是：**去掉重复的一整代依赖、关掉用不到的特性、把只为几行代码引入的大依赖换掉**。

## 2. 先测量，再动手

不要把"感觉大"当结论。`strip = true` 会把符号抹掉，`cargo bloat` 读不到东西，先做一个带符号的临时 profile：

```toml
# 临时加到根 Cargo.toml，量完就删（记得标注 TEMP）
[profile.sizeprobe]
inherits = "release"
strip = false
debug = 1
codegen-units = 16
lto = "thin"
```

```
cargo build --profile sizeprobe --features custom-protocol -p mml-gui
cargo install cargo-bloat --locked   # 只装一次
cargo bloat --profile sizeprobe --features custom-protocol -p mml-gui -n 40 --crates
```

产出：按 crate 排的前 40 名。**后续每一项改动都只认这张表指认的对象**，其余一律不动。
每改一处，重新量一次 exe 大小并记账，最后汇总到第 6 节的表里。

量完立刻删掉 `[profile.sizeprobe]`（§2 / §13 的 TEMP 约定）。

## 3. 一级：重复了一整代的依赖（收益最大、最好验证）

这几组的共同点：**同一件事有两套实现同时在 exe 里**，去掉一套不影响功能。

### A1. rfd 双份

- 事实：`mml-gui` 直接依赖 `rfd 0.17.2`；`tauri-plugin-dialog 2.7.3` 内部带 `rfd 0.16.0`。
  两套文件对话框实现 + 两代 `windows-sys`（0.60 / 0.61）同时进包。
- 改法（二选一，不要都留）：
  1. 去掉直接依赖 `rfd`，改用 `tauri-plugin-dialog` 的 API；
  2. 去掉 `tauri-plugin-dialog`，只留 `rfd`（前端不能自己调 rfd，所以要在 Rust 侧补命令）。
  优先试 1：`rfd` 现有 4 处引用，改成 plugin 的 `blocking_pick_file` 之类即可。
- 验证：文件选择、目录选择、保存对话框各点一次；`cargo tree -d` 里 `rfd` 只剩一个版本。

### A2. HTML 解析两代

- 事实：`scraper 0.29`（→ html5ever 0.40 / selectors 0.41 / cssparser 0.38）是我们自己用的；
  `tauri-utils` 带 `dom_query 0.28`（→ html5ever 0.39 / selectors 0.38 / cssparser 0.37）。
  两套整整两代 HTML5 解析器。
- 判据：`dom_query` 来自 tauri 内部，动不了；能动的只有 `scraper`。
  `scraper` 在源码里只有 2 处用法，先看这两处是不是"取一段 HTML 里的几个字段"。
- 改法：若是简单取值，换成已有的 `regex`（本来就在依赖里，35 处引用），删掉 `scraper`。
  若确实是复杂选择器，保留 `scraper`，把这条移到"不做"。
- 预期：去掉 html5ever / selectors / cssparser / web_atoms / phf / string_cache 各自的新一代。

### A3. 压缩与加密两代

- 事实：
  - `zip 9.0.5` → aes 0.9 / cipher 0.5 / lzma-rust2 0.22；
  - `sevenz-rust2 0.20.2` → aes 0.8 / cipher 0.4 / sha2 0.10 / digest 0.10 / lzma-rust2 0.16。
  即 RustCrypto 与 lzma 各两代（`cipher` / `crypto-common` / `block-buffer` / `inout` / `cpufeatures` / `lzma-rust2` 全成对出现）。
- 改法：
  1. 先看 `sevenz-rust2` 有没有升到 digest 0.11 的版本，有就升（最省事）；
  2. 升不上就把 `sevenz-rust2` 的特性关掉：现在开了 `aes256 bzip2 compress ppmd util`。
     我们只用它**解压** 7z 整合包，`compress`、`ppmd`、`bzip2` 是否可以关，取决于要支持哪些老包。
  3. `zip` 侧见 B1——它现在带着 aes / bzip2 / lzma / xz / zstd 全套。

### A4. Windows 绑定两代

- 事实：全工程用 `windows 0.62` + `windows-sys 0.61`，唯独 `clipboard-rs 0.3.5` 带进来
  `windows 0.59` + `windows-sys 0.60`（还带 `windows-core / -result / -strings / -implement` 的 0.59 一套）。
- 改法（按顺序试）：
  1. `clipboard-rs` 是否已有基于 0.62 的新版；
  2. 没有就用已有的 `windows` 特性自己写剪贴板（读/写文本 + 文件路径列表，代码量不大）；
  3. 再不行才考虑 tauri 的剪贴板插件。
- 注意：`clipboard-rs` 在 `mml-sys` 里只有 3 处引用，属于"小功能拖大依赖"的典型。

### A5. miniz_oxide 两版

- 事实：`flate2 1.1` 用 miniz_oxide 0.9，`png 0.18` 用 miniz_oxide 0.8；且 `flate2` 同时开了
  `miniz_oxide` 与 `zlib-rs` 两个后端（`rust_backend` + `any_zlib` + `runtime_detection`）。
- 改法：统一 `flate2` 只用 rust 后端（`default-features = false, features = ["rust_backend"]`），
  别开 `zlib-rs`；`png 0.18` 那版是否跟着升一档，看 `cargo tree -d` 收敛结果。
- 风险：`zlib-rs` 关掉后解压速度可能略降，需要拿导出整合包跑一次对比。

## 4. 二级：特性裁剪（改动小，但要先确认用到哪些格式）

逐条都要求"先查代码里有没有走到这个格式"，查到用途再关。

### B1. zip 特性

- 现在实际启用：`aes-crypto bzip2 deflate deflate64 lzma ppmd xz zstd zopfli chrono time`。
- 我们只是读写整合包 / 资源包，绝大多数是 deflate。若确认没有加密 zip、没有 zstd/xz 成员，
  改成：
  ```toml
  zip = { version = "~9.0.5", default-features = false, features = ["deflate", "chrono"] }
  ```
- 单项都值几十到几百 KB，合起来是本任务最划算的一条；风险只在"用户塞了个奇怪压缩方法的包"。

### B2. image 的 bmp / tiff

- 声明里是 `default-features = false, features = ["png", "jpeg", "webp"]`，但解析结果是
  `bmp jpeg png tiff webp` —— 说明**还有别的 crate 开了 image 的默认特性**，需要查出来（`cargo tree -i image`）。
- 改法：要么把那个依赖的特性改窄，要么用 `png` + 解码器替代 `image`（`png` 本来就在依赖里）。

### B3. wgpu 后端

- 现在启用 `dx12 gles metal vulkan web webgpu wgsl`；Windows 上 `metal` / `web` 是空壳，
  但 `gles` 与 `vulkan` 会真的编进 exe。
- 改法：`wgpu = { version = "~30.0.1", default-features = false, features = ["dx12", "wgsl", "std"] }`，
  确认启动器只在这台 Windows 上跑 D3D12。
- 风险：老显卡 / 远程桌面环境可能只有 GL 可用，需要保留一条回退路径或先确认显卡支持 D3D12。

### B4. reqwest 特性

- 现在启用了 `form` 与 `multipart`。若代码里搜不到 `.multipart(` / 表单上传，两个都可以关。
- `default-tls` 与 `rustls` 同时开着（`system-proxy` 也在），确认只用一套 TLS 后端。

### B5. 其余零散

- `tar` 的 `xattr`（Windows 无用，但 cfg 可能已排除）。
- `notify` 只被 `mml-game` 用，若用途只是"发现实例目录变了"，改成定时轮询可去掉整个文件监视链路。
- `rquickjs`（QuickJS 引擎）被 `mml-net` 依赖，先确认现在还有没有真在跑 JS；没有就删。

## 5. 三级：现在别动

- **profile 参数**：已经是最小组合，没有余量。
- **UPX / 压缩壳**：会让杀软误报，启动变慢，不做。
- **砍功能**：任何"去掉某种压缩格式支持 / 去掉某个渲染后端"的改动，必须先确认没有用户路径用到，
  否则按"体积换功能"另开一轮讨论。
- **前端 dist 内嵌**：属于另一条线，不在本任务里；要动也是先看 `mml-vue/dist` 的实际大小。

## 6. 执行顺序与记账

| 顺序 | 项 | 预期 | 风险 |
| --- | --- | --- | --- |
| 1 | A1 rfd 二选一 | 中 | 低：两套对话框行为需对齐 |
| 2 | A4 clipboard-rs 换 windows 0.62 | 中 | 低 |
| 3 | B1 zip 特性裁剪 | 中 | 中：老包可能不认 |
| 4 | A2 scraper 换 regex | 中 | 中：解析逻辑要重写 |
| 5 | B3 wgpu 只留 dx12 | 大 | 中高：显卡 / 远程桌面 |
| 6 | A3 / A5 加密与 lzma 收敛 | 小到中 | 低 |
| 7 | B2 / B4 / B5 | 小 | 低 |

每项做完记录：**exe 大小（MB，保留一位）+ 是否通过冒烟**。
冒烟口径：启动器能开、能进实例设置、能启动一次游戏；改动涉及压缩就再导入一次整合包。

## 7. 验收

- `cargo tree --target x86_64-pc-windows-msvc -d` 里：
  `rfd` / `windows` / `html5ever` / `lzma-rust2` / `miniz_oxide` 各自只剩一个版本。
- `target/release/mml-gui.exe` 相比 30.8 MB 有可记账的下降，且第 6 节每行都有实测数字。
- 无功能回归：文件对话框、剪贴板、整合包导入、皮肤 / 头像渲染、启动游戏。
