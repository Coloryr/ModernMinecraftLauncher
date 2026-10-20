# 主题包：类型判定与 zip 合法性校验

## 1. 背景

启动器现在有三种互相独立的主题包：

| 种 | 设置位置 | 产物 | 现有导入命令 |
| --- | --- | --- | --- |
| 自定义主页面 | 客户端设置 | `custom_home.zip` | `custom_home_import` |
| 自定义 CSS | 界面 | `custom_ui/css/<id>.zip` | `custom_ui_import_css` |
| 完全自定义界面 | 界面 | `custom_ui/page.zip` | `custom_ui_import_page` |

目前类型由「用户点的是哪个导入按钮」决定，导入只检查入口文件是否存在，没有检查：

- 包内容与导入类型是否匹配；
- zip 本身是否合法（损坏、加密、路径穿越、重复条目、压缩炸弹等）；
- 整包套目录后入口是否仍在根；
- 失败时是否一定保留旧包。

目标：**先判定类型，再校验 zip，最后才替换**。任何一步失败都不改动用户正在用的包。

## 2. 类型定义

```rust
pub(crate) enum ThemeKind {
    /// 自定义主页面：只替换主窗口主页内容区
    Home,
    /// 自定义 CSS：多份，按条目启用
    Css,
    /// 完全自定义界面：整页 / 按 kind 替换窗口
    Page,
}
```

三种包的入口签名：

| 类型 | 包根必需 | 可选 |
| --- | --- | --- |
| `Home` | `index.html` 或 `custom.js` | `custom.css` |
| `Css` | `custom.css` | `custom.js` |
| `Page` | `index.html` 或 `custom.js` | `custom.css` |

**Home 与 Page 的入口完全相同，单看文件无法区分。** 必须二选一：

### 方案 A：根清单（推荐）

包根放 `theme.json`：

```json
{
  "type": "home",
  "name": "Coloryr 服务器主页",
  "version": "1.0.0",
  "api": 1
}
```

- `type`：`home` / `css` / `page`，必填；
- `name` / `version`：可选，给设置页显示；
- `api`：可选，主题接口版本，默认 1；
- 文件必须在包根，整包套一层目录时先解开那层再找；
- JSON 不允许注释、尾逗号，大小上限 64 KiB。

### 方案 B：导入上下文（兼容旧包）

没有 `theme.json` 时，类型取用户点的导入入口，只做「入口文件匹配」校验：

- 导入 CSS：必须有 `custom.css`，忽略 `index.html`；
- 导入 Home / Page：必须有 `index.html` 或 `custom.js`；
- 如果同时有 `custom.css` 但没有 `index.html` / `custom.js`，判定为 CSS；
- 仍然无法区分 Home 与 Page，按导入入口处理，并在设置页提示「未声明类型」。

**最终决定：两种都支持。有清单以清单为准；没有清单按导入入口，且只能做兼容性校验。**

### 类型不匹配

- 清单声明 `type: "css"`，却从「完全自定义界面」导入：拒绝，报 `err.themeTypeMismatch`；
- 清单声明 `home` / `page`，入口文件缺失：拒绝，报 `err.themeMissingEntry`；
- 清单 JSON 非法 / `type` 未知：拒绝，报 `err.themeManifestInvalid`，不猜。

## 3. zip 合法性校验

校验分三层，全部在替换旧包之前完成。

### 3.1 容器层

- 文件存在、可读、非空；
- 能作为 zip 打开，中央目录完整，非分卷 / 非多磁盘；
- 不支持加密条目，发现任意加密条目直接拒绝；
- 仅接受普通文件与目录，拒绝符号链接、设备、FIFO；
- 条目名必须是合法 UTF-8；反斜杠统一成正斜杠后处理；
- 压缩包文件本身大小上限，建议 256 MiB；
- 条目数上限沿用 `MAX_ENTRIES = 4096`；
- 单条目解压后上限沿用 `MAX_ENTRY_BYTES = 128 MiB`；
- 总解压后上限沿用 `MAX_TOTAL_BYTES = 512 MiB`；
- 压缩比上限，建议 200:1，防高压缩比炸弹；
- 可选：逐条目读一遍校验 CRC；成本高时至少校验中央目录声明的尺寸是否可信。

### 3.2 路径层

对每个条目名做规范化后检查：

- 不允许绝对路径（`/xxx`、`C:\xxx`、`\\server\share`）；
- 不允许任何 `..` 段；`.` 段去掉；
- 不允许 NUL、控制字符；
- 不允许规范化后为空的路径；
- 不允许大小写不敏感重名（Windows 上 `A.png` 与 `a.png` 只能留一个）；
- 不允许规范化后重复的条目；
- 目录条目可以存在，但不能作为入口。

现有 `resolve_entry` 仍是请求期的最后一道防线；导入期校验是为了「坏包根本不入库」。

### 3.3 布局层

- 支持整包套一层目录：先找唯一顶层目录，后续入口 / 清单都相对它判断；
- 顶层文件与顶层目录混放时，不认为有唯一顶层目录；
- 入口文件必须在包根（或解开唯一顶层目录后的根）；
- `theme.json` 同样按这个根查找；
- 入口文件必须是普通文件，不能是目录；
- `custom.js` / `custom.css` / `index.html` 名字大小写敏感，但 Windows 导入时按小写匹配一次并提示。

### 3.4 导入原子性

- 校验全部通过后才复制到临时文件；
- 临时文件写完再 `rename` 替换；
- 替换后重开常驻句柄；
- 任一步失败：删除临时文件、旧包与旧句柄保持不变；
- CSS 条目按 uuid 落盘时同样先写 `.tmp` 再替换。

## 4. 接口与数据结构

### 4.1 公共校验模块

建议放在 `mml-gui/src-tauri/src/windows/web_assets.rs` 或同目录新文件：

```rust
pub(crate) struct ThemeArchive {
    pub(crate) kind: ThemeKind,
    pub(crate) name: Option<String>,
    pub(crate) version: Option<String>,
    // 现有 WebArchive 字段
}

pub(crate) fn detect_theme(
    path: &Path,
    expected: Option<ThemeKind>,
) -> Result<ThemeInfo, String>;
```

- `detect_theme` 内部完成容器、路径、布局、清单校验；
- 返回类型与展示信息；
- 三种导入命令都调它，传各自的 `expected`；
- 以后新增主题类型只改 `ThemeKind` 与签名表。

### 4.2 DTO

`CustomHomeInfoDto` / `CustomUiInfoDto` 增加：

```rust
/// 实际类型；没有清单时按导入入口推断
pub kind: ThemeKindDto,
/// 主题名，来自 theme.json；没有时为空串
pub name: String,
/// 主题版本，来自 theme.json；没有时为空串
pub version: String,
```

`ThemeKindDto` 用 camelCase：`home` / `css` / `page`。

### 4.3 错误键与 i18n

新增（中英两套）：

- `err.themeTypeMismatch`：包类型与导入入口不匹配
- `err.themeTypeUnknown`：无法判定类型
- `err.themeManifestInvalid`：`theme.json` 非法
- `err.themeMissingEntry`：缺少该类型的入口文件
- `err.themeZipInvalid`：不是合法 zip
- `err.themeZipEncrypted`：包含加密条目
- `err.themeZipEntryPath`：条目路径越界
- `err.themeZipDuplicate`：条目重名
- `err.themeZipUnsupported`：包含不支持的条目类型
- `err.themeZipRatio`：压缩比异常

现有 `err.pageArchiveLimit` 保留，用于条目数 / 单条 / 总量超限。

## 5. 导入流程

```text
选择 zip
  → detect_theme(path, expected)
      → 打开 zip / 容器校验
      → 路径校验
      → 定位包根（唯一顶层目录）
      → 读取 theme.json（可选）
      → 类型判定 + 入口校验
  → 通过后复制到 <目标>.tmp
  → rename 替换
  → reload 常驻句柄
  → 广播 custom-home-change / custom-ui-change
  → 返回新 DTO
失败 → 删临时文件，旧包不动，返回错误键
```

- 三个导入命令仍分开，避免把 CSS 的「多份列表」和 Home / Page 的「单份覆盖」混在一起；
- 自动路由（一个入口根据清单分到三个命令）可作为后续增强，不在本次范围。

## 6. 测试

### 6.1 合法包

- Home：`index.html`、`custom.js`、两者都有、带 `custom.css`；
- Page：同上；
- CSS：`custom.css`、`custom.css + custom.js`；
- 整包套一层目录；
- 清单声明正确类型；
- 清单缺失时按导入入口成功。

### 6.2 非法包

- 不是 zip / 空文件 / 截断；
- 加密条目；
- `../`、绝对路径、盘符、NUL、控制字符；
- 大小写重名、规范化重名；
- 符号链接 / 设备文件；
- 超过 4096 条目、单条 128 MiB、总量 512 MiB、压缩比 200:1；
- 入口缺失、入口是目录；
- 清单 JSON 非法、`type` 未知；
- 清单类型与导入入口不匹配；
- Home / Page 互相导入（无清单时应成功，有清单时应拒绝）。

测试 zip 用 `mml_testutil::temp_dir()` 在 `target/temp` 构造，不要用系统临时目录。

### 6.3 回归

- 现有 `resolve_entry` 路径穿越测试继续通过；
- 导入失败后旧包仍能正常打开；
- 协议请求仍能从新包读到资源。

## 7. 不在本次范围

- 主题包签名 / 可信来源校验；
- 在线下载与更新；
- 自动把 Home / Page 合并成同一种主题；
- 解压到磁盘；
- 允许主题包执行 Rust / 原生代码。

## 8. 验收标准

- 导入任一种包前都能给出明确的类型判定结果；
- 类型不匹配、入口缺失、zip 非法时导入失败，旧包不丢；
- 损坏 / 加密 / 路径穿越 / 重名 / 炸弹包在导入期被拒绝，而不是等请求时 404；
- 错误提示走 i18n，中英两套齐全；
- 现有三种主题的独立开关与导入流程不变；
- 新增测试覆盖第 6 节的主要用例。
