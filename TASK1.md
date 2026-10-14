# TASK1：标题栏按钮尺寸与间距、窗口内边距与控件一致性检查（临时任务）

## 要做什么

把**所有窗口**过一遍，查三件事：

1. **标题栏**：`WindowFrame` 的 `.frame-head`，以及各窗口塞进 `#head-right` 槽的东西 —— 图标按钮的尺寸、内边距、彼此间距、与 chip / 下拉之间的间距，是否同一套。
2. **窗口之间是否统一**：同一角色的东西在不同窗口里是不是同一个尺寸、同一档间距。
3. **窗口内部控件与窗口边距**：内容区的左右与下内边距、行内控件高度与行间距，各窗口是否一致。

**现状：16 个窗口已全部查完**（四条口径 + 四组细查），结论见下。改代码前先定 B 栏。

## 口径（判定依据）

| 项 | 出处 | 值 |
| --- | --- | --- |
| 标题栏高度 | `styles/themes.css` | `--titlebar-h: 64px` |
| **控件统一高度** | `styles/controls.css:31` | `--ctl-h: 32px` —— 输入框 / 下拉 / 按钮（`.s-sm` 与 `.s-md` 都是它）/ 开关 / 步进器 / 分段控件整轨，**全是这一档** |
| 分段控件内胆 | `components/ui/SegmentedTabs.vue:67` | `calc(var(--ctl-h) - 6px)` = 26px（整轨仍是 32px） |
| 开关滑块 | `components/ui/BaseSwitch.vue:48` | `calc(var(--ctl-h) - 6px)` = 26px |
| 圆角 / 字号 | `styles/controls.css` | `--ctl-radius: 8px` / `--ctl-font: 12.5px` |
| 窗口内容区内边距 | `components/ui/WindowFrame.vue:171` | `.frame-body { padding: 22px 26px }`；`.gutter` 变体右内边距 `calc(26px - 9px)`；`.fill` 变体右 26px |
| 标题栏元素间距 | `components/ui/WindowFrame.vue:119` | `.frame-head { gap: 14px }`；窗口自己在槽里再包一层时用 8px |
| 标题栏图标按钮 | `LogWindow.vue` / `ConfigWindow.vue` 的 `.icon-btn-head` | `padding: 0 10px` + `:deep(.ui-btn) { flex-shrink: 0 }` |
| 标题栏下拉 | 各窗口 scoped `.head-select` | 共有：字号继承 `.field-select`、箭头 `right 7px` / `11px`；宽度口径见 A1 |
| 实例 chip | `components/ui/InstanceChip.vue` | 名字必给，版本可选 |

> ⚠️ **`controls.css:38-46` 那张表不能当判定依据**：它自己写着"预留档位（当前全 app 无人使用，按需取用）"，
> 列的是 22 / 27 / 28 / 30 / 35 / 36 / 42px 这些**还没用**的档位与它们曾经的出处。
> 现状是"全 app 一档 32px"。本轮第一版口径误读了它，四组细查都独立复核并纠正了。

## 结论（按主题归并）

### 1. 标题栏图标按钮：四种做法并存（A2）

| 做法 | 窗口 | 现状 |
| --- | --- | --- |
| `.icon-btn-head` 方块化 | log、config | `padding: 0 10px` + `flex-shrink: 0`（正确样板） |
| 普通 `BaseButton` | resource、export、add_resource | 只拿到 `.s-sm` 的 `padding: 0 14px`，文字按钮宽度随内容 |
| 自造 `.topbar-icon-btn` | main | `width/height: var(--ctl-h)`，**圆角 10px** 而非 8px |
| 自绘小图标键 | account `.head-search-clear`(≈12px)、config `.cfg-search-clear`(20px)、block `.search-clear`/`.block-del`(20px) | 三档高度并存，都无 `--ctl-h` 兜底 |

另有两处**缺保护**：block 的 `padding: 0 10px` 没配 `flex-shrink: 0`（父容器 `flex-shrink: 1` 会压它）；
resource / add_resource 的槽里没有 `:deep(.inst-chip) { min-width: 0 }`（窄窗时 chip 不省略）。

### 2. `.head-select`：四份定义已经走样（A1）

| 窗口 | min-width | max-width | 省略号 | 备注 |
| --- | --- | --- | --- | --- |
| account | 128px | 无 | 无 | 另写死 `font-size` |
| config | 170px | 280px | 有，**缺 `nowrap`** | |
| export | 260px | 340px | 有 | |
| log | 170px | 280px | 有 | 事实样板 |

### 3. 标题栏槽内的分组间距三档并存（A4）

8px（log / config / block）· 12px（collect 的筛选条）· 14px（settings / resource / add_resource
没有自己包一层，直接吃 `WindowFrame` 的 `gap: 14px`）。

### 4. 窗口内容区内边距：标准是 22/26，但一半窗口在跟它对抗（A5 / A6）

`.frame-body` 给的 22/26 是唯一标准，可是：

- **settings**：先 `margin: -22px -26px` 抵消、再由 `.tab-content` 重加 18/22/24；
  且 `height: calc(100% + 44px)` 与那个 22px 硬耦合 —— 改 `.frame-body` 会静默错位。
- **block**：`BlockPanel.vue:165-168` 三边负外边距 + `padding-right: 0`。
- **resource**：`margin-right: -26px` 抵消，26 / `calc(26px + 9px)` 在窗口内重复 4 处。
- **add_resource**：`.res-list` `margin: 0 -8px` + `padding: 0 8px 8px`。
- **main**：根本不用 WindowFrame，内容区四套内边距 —— `.detail` 16/26/130、
  `.news-page` 18/28/130、`.list-mode` 20/30/30、`.lock-error` 40/28/130（底部 130 出现三次）。

### 5. 单窗口模式换了外壳，内边距跟着变（A13）

- log → `FloatingPanel`：内边距 12px 16px（`.fp-body`），不是 22/26；
- download → `BaseModal`：`padding: 22px 15px`；
- 其余窗口单窗口模式仍是页面级，走 22/26。

### 6. 筛选下拉被 padding 撑成 42px（A9，本轮最实的一条）

`add_modpack/modpack-mode.css:24` 与 `:41`、`add_resource/add-resource-window.css:40`：
`.sel-filter` 用 `padding: 10px`（行高 20 + 上下 20 + 边框 2）= **42px**，而同一行的搜索框是 32px，
`min-height: var(--ctl-h)` 只是下限、压不住它。两处注释还写着"`--ctl-h`（42）"，
与唯一来源 `controls.css:31`（32px）**直接矛盾**。

### 7. 控件高度的其余偏离

- **无 `--ctl-h` 兜底**：`resource/parts/ServerFormModal.vue:67` 的 `.form-input`（只挂 `form-input`，
  没有 `min-height`，实际约 23px）；`.search-btn`（无 height，靠 stretch 拉伸）。
- **靠 padding 撑、无定高**：`multi-bar.css` 的 `.multi-btn` / `.multi-exit`（7px 12px ≈31px）、
  collect 的 `.group-act` / `.add-group-row` / `.ctx-item` / `.pick-item`、block 的 `.empty-clear`。
- **写死但疑似有意**：`resource.css:930` 表格头单元格 28px（同窗口注释却自称"全 app 那一档 32px"）。

### 8. 圆角偏离 8px（A12）

7px（`resource.css:325` `.mini-btn`、`DownloadWindow.vue:592`、`FileTreePanel.vue:85` `.files-btn`）
· 9px（`collect-window.css:38`/`:106`、`multi-bar.css:12`）· 10px（`MainTopbar.vue:174` `.topbar-icon-btn`）。

### 9. 已澄清（不是偏离）

- **36px 不是控件档**：resource 的 `.item-icon`/`.sk-icon`、add_resource 的 `.pack-desc` —— 图标与描述高度。
- **28 / 27 / 22px 在源码里已不存在**：统一到 `--ctl-h` 之后只剩 32px 一档（分段控件内胆 26px）。
- `.head-select` 只出现在 account / config / export / log 四个窗口；add_resource 标题栏**没有按钮**；
  stats / help 没有槽内容；main 完全不用 WindowFrame（`.frame-body` 与它无关）。
- 主窗口 `.topbar` 高度同样吃 `--titlebar-h`，没有偏离。

## 执行进度

### 第 1 批：标题栏标准收敛 + 42px 下拉（已完成，`cd mml-vue && npm run build` ✓）

**新增共享标准** `mml-vue/src/styles/titlebar.css`（在 `styles/index.css` 里 import）：

- `.frame-head .head-select`：宽度 `min 170` / `max 280`、`nowrap` + `ellipsis`、箭头 `right 7px / 11px`，
  **不写字号**（继承 `.field-select` 的 `--ctl-font`，避免换档位时脱钩）；
- `.frame-head .ui-btn { flex-shrink: 0 }`、`.frame-head .ui-btn.icon-btn-head { padding: 0 10px }`、
  `.frame-head .inst-chip { min-width: 0 }`。
- 为什么必须是全局样式：槽内容由各窗口自己的模板编译，写在窗口 scoped 样式里只对自己生效 ——
  这正是过去抄多份、四处走样的根因。文件头写了这一条。

**删除的副本**：config / log 的 `.head-select`、`.icon-btn-head`、`:deep(.ui-btn)`、`:deep(.inst-chip)`；
block 的 `.block-top :deep(.ui-btn)`（改成给它那两颗纯图标键挂 `.icon-btn-head`，并因此拿到
`flex-shrink: 0`）。**保留并写明理由的例外**：账户 `.head-select { min-width: 128px }`（窄筛选）、
导出 `.head-select { min-width: 260px; max-width: 340px }`（实例名长）。

**间距**：`WindowFrame` 的 `.frame-head` 间距 `14px → 8px`（与各窗口自己包一层时用的 8px 对齐）；
collect 的 `.collect-toolbar .collect-filters` `12px → 8px`。

**42px 筛选下拉（A9）**：`modpack-mode.css` 两处、`add-resource-window.css` 一处的 `padding: 10px`
删除 —— 那是当年配 `--ctl-h: 42px` 写的（`20 行高 + 10×2 + 2 边框 = 42`），`--ctl-h` 统一到 32 后
就成了孤儿，让下拉比同排搜索框高一头。现在交给 `.field-select` 自己的 `5px`（正好 32），
两处旧注释一并改写成"为什么不写"。

**其它**：`.form-input`（ServerFormModal）补 `min-height: var(--ctl-h)`、字号回 `--ctl-font`；
账户 `.head-search-clear` 改成 20px 方形热区（与配置文件窗口那枚同口径）、其内层 input 字号
`13px → var(--ctl-font)`。

### 下一批（待做）

- **A3**：两套标题栏搜索合并（account `.head-search` 168px / `--bg-card` / 无下拉
  vs settings `.set-search` 280px / `--bg-solid` / 有下拉）—— 字号已在第 1 批统一，剩下的差异要动结构；
- **A2 尾巴**：主窗口 `.topbar-icon-btn`（圆角 10px）并入标题栏标准（它不在 `.frame-head` 里，
  共享选择器要一并覆盖 `.topbar`）；
- **A5 / A6**：settings 与 main 的内边距"抵消再重加"要么收敛、要么就地写明理由；
- **A7** `.empty-tip` 覆写、**A11** 靠 padding 撑的按钮、**A12** 7/9/10px 圆角、
  **A13** 单窗口外壳内边距、**A14** log 的 `.lv` 旧注释、**A15** resource 表头 28px；
- **B1–B6**：等定档（本轮未动）。

## 偏离清单

### A. 已确认（建议改）

| # | 位置 | 现状 | 建议统一到 |
| --- | --- | --- | --- |
| A1 | `.head-select` ×4 | 宽度 128 / 170 / 260；account 无 `max-width` 与省略号且写死字号；config 缺 `nowrap` | 共享一份：`min-width` 170、`max-width` 280、含 `nowrap` + `ellipsis`，字号继承 `.field-select`；特殊宽度写在窗口里并注明 |
| A2 | 标题栏图标按钮 | 四种做法；block 缺 `flex-shrink: 0`；resource / add_resource 缺 chip 的 `min-width: 0` | 收敛成共享 `.icon-btn-head`（32px 方块、8px 圆角、`padding: 0 10px`、`flex-shrink: 0`），主窗口那套并入 |
| A3 | 标题栏搜索 | account `.head-search`（168px / 字号 13 / `--bg-card`）与 settings `.set-search`（280px / 12.5 / `--bg-solid` / 有下拉）两套 | 抽共享组件，关键值取 `.field-input` 那套 |
| A4 | 槽内分组间距 | 8 / 12 / 14px 三档 | 定一档（建议 8px），未分组的补上包装 |
| A5 | settings 内容区 | 负边距抵消后重加 18/22/24，且与 22px 硬耦合 | 不再抵消，直接吃 22/26；需要更紧的间距改在卡片上 |
| A6 | main 内容区 | 四套内边距（左右 26/28/30/28，下 130 ×3） | 收敛成一套左右内边距 + 一处底部留白常量 |
| A7 | `AccountGrid.vue:206` | `.empty-tip` 被覆写成 `padding: 48px 0`（全局 10px） | 有意为之就加注释，否则回全局值 |
| A8 | 自绘小图标键 | 12 / 20 / 32px 三档并存（account、config、block） | 并入图标按钮那一档 |
| A9 | 筛选下拉 | `padding: 10px` 撑成 42px（add_modpack ×2、add_resource ×1），同排搜索框 32px；注释还写"`--ctl-h`（42）" | 压回 32px 档（去掉纵向 padding 或改 `min-height` 口径），并改掉那两处旧注释 |
| A10 | `ServerFormModal.vue:67` | `.form-input` 没挂 `.field-input`、无 `min-height`，约 23px | 挂上 `.field-input` 或补 `min-height: var(--ctl-h)` |
| A11 | 靠 padding 撑的按钮 | `.multi-btn`/`.multi-exit` ≈31px、collect 的 `.group-act`/`.add-group-row`/`.ctx-item`、block 的 `.empty-clear` | 需要成控件的补 `height: var(--ctl-h)`；是"行/标签"的加注释说明 |
| A12 | 圆角 | 7 / 9 / 10px 三处自造按钮偏离 8px | 统一 `var(--ctl-radius)` |
| A13 | 单窗口外壳 | log 用 FloatingPanel（12/16）、download 用 BaseModal（22/15） | 定一档单窗口内边距，或明确记下这两个外壳的差异理由 |
| A14 | `LogWindow.vue:590` | 注释称等级勾选"与开关同高"，但 `.lv` 无定高、是原生 checkbox | 注释改成实情，或给勾选行补定高 |
| A15 | `resource.css:930` | 表格头 28px 写死，同窗口注释自称"全 app 那一档 32px" | 要么用 `var(--ctl-h)`，要么把注释改成"表格头单独一档" |

### B. 待你定（像是有意为之）

| # | 位置 | 现状 | 问题 |
| --- | --- | --- | --- |
| B1 | main `.play-btn` 52px / `.launch-main-btn` 46px | 主页两个大按钮 | 给它们开两档，还是并到 32px？ |
| B2 | main 侧栏行高 48 / 50 / 50 | 三处写死 | 统一成一行高档（如 48）？ |
| B3 | `AccountSelector` 按钮 ≈48px | 与同排 32px `.topbar-icon-btn` 不齐 | 顶栏这一排统一到 32px？ |
| B4 | help `.faq-q` ≈47px 整行按钮 | 不在控件档位 | 按整行看待（视为正常），还是纳入档位？ |
| B5 | `.s-sm` 与 `.s-md` 现在都是 32px | 两个尺寸档已无区别 | 保留两个类名，还是并成一个？ |
| B6 | `.sel-filter` 若保留 42px（即 A9 的另一选择） | 筛选条比同排搜索框高 | 认可"筛选条 42px"单独一档，还是压到 32px？ |

## 逐窗口清单（已填完）

| 窗口 | 标题栏内容 / 按钮 class | 内容区自加 padding | 写死的控件高度 | 主要 gap 档位 | 偏离 |
| --- | --- | --- | --- | --- | --- |
| main | `MainTopbar`，`.topbar-icon-btn`（自造 10px 圆角） | 不用 WindowFrame：16/26/130、18/28/130、20/30/30、40/28/130 | 52 / 46 / 48 / 50 / 50；账户按钮 ≈48 | 16 > 10 ≈ 8 > 6 | A2 A6 B1 B2 B3 |
| settings | 只有 `SettingsSearch` | 负边距抵消后重加 18/22/24 | 无（全走 `--ctl-h`；唯一例外 `.java-add-row .ui-btn{height:auto}`） | 8 ≈ 12 > 10 | A3 A5 B5 |
| stats | 无 | 无 | 无（窗口里没有控件） | 14 / 6 / 10 | — |
| help | 无 | 无 | 无（唯一按钮是整行 FAQ ≈47px） | 10 / 12 | B4 |
| resource | InstanceChip + BaseButton（普通） | 根无；补偿型 4 处 `padding-right: 26/35` + `margin: -26` | 28（表头）/ 36（图标）/ 22 / 18 / 15 | 6×6 > 8×5 ≈ 4×5；一级布局 18 | A2 A12 A15 |
| account | head-select + head-search + head-tabs | 无 | `.head-search-clear` 无定高（≈12px）；input 字号 13px | 8 > 6 / 10 / 12 > 14 | A1 A3 A7 A8 |
| add | 无 | 根无（卡片 16/18） | 无（全走 `--ctl-h`） | 8 / 10 / 14 / 6 / 2 | — |
| add_modpack | 无 | 根无（卡片 10/18） | **筛选控件 42px**；其余写死都是非控件 | 10 / 8 / 12 / 5 / 6 / 3 | A9 B6 |
| add_resource | InstanceChip + head-warn（无按钮） | 根无；`.res-list` `margin: -8` + `padding: 0 8 8` | **筛选下拉 42px**；36（描述）/ 32（图标）/ 22 / 16 / 15 | 8×8 > 6×4 > 12×3 | A2 A9 |
| collect | 筛选条 + 计数（组内 gap 12） | 根无；`.collect-empty` 48/0、`.multi-bar` 8/12 | 无 height：multi-btn ≈31、group-act、add-group-row、ctx-item | 12×5 > 8×3 > 4/6/10×2 | A4 A11 A12 |
| download | 无（单窗口外壳是 BaseModal） | 根无 | 无（全走 `--ctl-h`；8 / 6 是进度条） | 6 / 7 / 8 / 12 / 14 | A12 A13 |
| java_download | 无 | 根无（卡片 12/14） | 无（20px 是转圈） | 10 / 12 / 6 / 8 | — |
| block | `BlockToolbar`（`padding: 0 10px`，**缺 `flex-shrink: 0`**） | 显式归零：三边负外边距 + `padding-right: 0` | 20（×2，图标）/ 6（进度条）；格高 84/100/136 | 8×3 ≈ 10×3 > 12/16×2 | A2 A8 |
| log | InstanceChip + head-select + 5× `.icon-btn-head` | 根无（注释明说交给外壳） | 无（开关 32；等级勾选无定高） | 8 / 10 / 6 / 4 | A1 A4 A13 A14 |
| export | head-select（260/340）+ BaseButton（文字） | 根无（注释明说） | 无（6px 是进度条） | 12 / 8 / 6 / 2 / 4 | A1 A2 |
| config | InstanceChip + head-select + 状态文字 + 3× `.icon-btn-head` | 根无（`.cfg-list` 6） | 20px（`.cfg-search-clear`） | 12 / 6 / 2 / 10 / 14 / 8 | A1 A4 A8 |

## 交付物

1. ~~逐窗口清单填完~~ ✅
2. 偏离清单 A 栏直接改；B 栏先定档再改；
3. **只改确认偏离的地方**，不顺手重构；每改一批跑一次 `cd mml-vue && npm run build`；
4. 重复定义（`.head-select` / `.icon-btn-head` / 两套搜索）收敛到共享样式，不能收敛的要说明理由；
5. 本文件在任务完成后删除（约定见 AGENTS.md §6.1）。
