# TASK.md — 前端死控件与可优化点（首轮）

> 审查时间：2026-11-24
> 范围：`mml-vue/src/windows/**`，**排除 `resource/` 与 `add_resource/`**（另一会话正在改）；**云存档、联机大厅仍在计划中，本轮一并排除**。
> 本轮只做审查，未改代码。行号以当前工作区为准，动手前请再核对一次。

## P0 死控件 / 假入口

| # | 位置 | 现象 | 建议 |
| --- | --- | --- | --- |
| 1 | `windows/collect/CollectWindow.vue:795-827`（`startDownload` 779-785） | 收藏里的存档 / 数据包点“安装”只弹一句“请到资源窗口操作”，不跳转、不带项目；`openDownloadWindow` 的 `add_resource` 分支因此永远走不到 | 直接 `openWindow("add_resource", { project })`，或给“在资源窗口打开” |
| 2 | `windows/settings/parts/tabs/ExperimentalTab.vue:120,126` | “URI 启动”“沙盒启动”两个开关恒带 `disabled`，点击无反应、`v-model` 也不回写；后端注释也写明“生效要等实现” | 先隐藏 / 删除；实现后再放出来 |
| 3 | `windows/settings/parts/tabs/ExperimentalTab.vue:99` + `lib/resourceMap.ts:71-77` | 处理中关掉设置窗口再打开，`attachMapping` 只把 `mapping` 置 true，没有完成事件或轮询复位；后端跑完后按钮仍永久停在“正在处理…” | `attachMapping` 轮询 `resourceMapRunning()`，或补完成事件 |
| 4 | `windows/account/SkinPage.vue:689-709` | 当前装备的皮肤 / 披风被 `item.active` 永久禁用，全窗没有“取下 / 还原默认”入口，后端也没有清空命令 | 后端补清空能力，前端在已装备项给“取下” |
| 5 | `windows/block/BlockPanel.vue:577-593` + `useBlockList.ts:200` | 首轮渲染成功后整窗没有重新渲染入口；`startRender(force)` 的 `force` 只在未渲染的提示卡里传 false，`true` 分支不可达 | 标题栏或分组菜单补“重新渲染”，调 `startRender(true)` |
| 6 | `windows/main/MainWindow.vue:1334`（ActionId 594-602） | `ActionId` 含 `export`，但 `onAction` 的 switch 没有对应 case，会落到 `actions.wip` 兜底；目前没有调用点，属死代码 | 补 `case "export"`，或删掉 wip 兜底让类型检查兜住漏接 |
| 7 | `windows/settings/parts/tabs/ClientTab.vue:232` | 登录方式锁定“添加”按钮的 `:disabled="!addLockOptions.length"` 恒为 false（`SERVER_LOCK_TYPES` 三项永远在候选里），禁用态永远到不了 | 删掉该条件，或改为按 `validateLockServer()` 结果禁用 |

## P1 功能缺陷

| # | 位置 | 现象 | 建议 |
| --- | --- | --- | --- |
| 8 | `windows/account/AccountWindow.vue:545-548` | “刷新 Token”用 `void` 发起后立刻提示“已刷新”；后端抛错时照样提示成功，连点还会并发请求 | await 结果，只在成功时提示，并加 busy 禁用 |
| 9 | `windows/download/DownloadWindow.vue:199-202`（配合 233-237） | “停止全部”先置 `keepOpen = true`，紧接着 `refresh()` 在任务表非空时又把它改回 false；取消是异步的，任务清空后窗口仍会自动关闭，与“主动停止后保留窗口”的注释相反 | `keepOpen` 只在恢复下载 / 新建任务时清零，别在轮询里重置 |
| 10 | `windows/download/DownloadWindow.vue:221-248` | 暂停 / 继续 / 停止三个 handler 没有 busy 防重复，也没有 try/catch；连点会重复发 IPC，失败只留下未处理的 Promise，界面无提示 | 加 busy ref + `try/catch` 弹 `tErr` |
| 11 | `windows/settings/parts/tabs/JavaTab.vue:122` | “压缩包导入”的 disabled 只算 scanning / scanningDir，没算 `importingJava`；导入中可再次打开文件框，`importProgress` 被覆盖 | disabled 补 `importingJava` |
| 12 | `windows/config/ConfigWindow.vue:563-586` | Ctrl+S 监听挂在 `window` 上，只在 `onBeforeUnmount` 摘除；单窗口模式 App.vue 用 KeepAlive，切到别的页面后按 Ctrl+S 仍会保存隐藏的配置窗口 | 用 `usePageActive` 守卫，或改成 `onActivated/onDeactivated` 装卸监听 |
| 13 | `windows/java_download/JavaDownloadWindow.vue:206-234`（由 183 的 `useWindowRefresh` 触发） | 每次切回窗口 / F5，`loadOptions()` 都把类型、主版本、系统、架构重置为平台默认，用户刚选的筛选丢失 | 仅首次打开或换源时给默认值；刷新保留已选值（不在候选里才回退） |
| 14 | `windows/export/ExportWindow.vue:481-487` | 切换导出格式会清空 `pickedFiles` 并重扫目录，用户手选的勾选 / 排除全部丢失且无提示 | 切格式前提示，或按格式分别缓存勾选 |
| 15 | `windows/collect/CollectWindow.vue:516-528` + 1196 | 新建分组先关弹窗再 await；重名等失败只弹 toast，用户输入丢了；点“取消”也不清 `newGroupName`，下次打开还留着上次的字 | 成功后再关、失败保留输入；取消时清空草稿 |
| 16 | `windows/settings/parts/tabs/NetworkTab.vue:113-114` | 代理“保存”不受 `proxyDirty` 控制，没改动也提示已生效；代理草稿在切页 / 关窗时静默丢弃（标题栏只有未保存提示，无法阻止关闭） | `:disabled="!proxyDirty"`，离开时提示 |
| 17 | `windows/settings/SettingsWindow.vue:124-128,250-255` | 锁定口径有两套：`UiTab.vue:329` 用 `v-if="!locked"` 藏了材质 / 不透明度，但 `search.ts:72-83` 仍把这两项索引在未锁定的 `window` 组；`LaunchTab.vue:220` 的自动进入存档同理（search 203-207 指向 `launchCheck`）。锁定时搜索会跳到被藏掉的控件 | 锁定范围收敛成唯一真源，或搜索前按锁定状态过滤 |
| 18 | `windows/log/LogWindow.vue:660-724`、`windows/download/DownloadWindow.vue:291-315` | `onMounted` 里先 await（syncInstance / refresh），之后才注册事件；窗口在这段 await 期间被关掉时，订阅在 unmount 之后才挂上且永不注销 | 每个 await 后检查已卸载标志，或先订阅再加载 |
| 19 | `windows/settings/parts/GamepadKeyPicker.vue:124-125` | 轮询 catch 里 `tErr(error)` 的返回值被直接丢弃；采样失败时界面完全无反馈，还每 100ms 继续失败 | 显示错误 / toast，并停掉本轮轮询 |

## P2 优化项

| # | 位置 | 现象 | 建议 |
| --- | --- | --- | --- |
| 20 | `windows/settings/composables/useSettingsClient.ts:152-158` | 服务器地址 setter 每敲一个字符就 `applyClient` 整份落盘（一次 IPC + 写 config）；同页音乐地址用的是 `@change` | 改 `@change` 或加防抖 |
| 21 | `windows/download/DownloadWindow.vue:29,277-281` | 固定 100ms 拉一次完整下载状态，没有任务时也照跑，前台每秒 10 次 IPC | 有任务时 100ms，空闲 500-1000ms 或停表 |
| 22 | `windows/collect/CollectWindow.vue:316-320` | `fillIcons` 对全部收藏串行 await，收藏多时图标逐个蹦出；每次 reload 都重走一遍 | 限并发 4-6，或只解析可视区 |
| 23 | `windows/block/composables/useBlockList.ts:286-315` | 每次按键过滤都对全部方块调用 `catLabel()`，内部各一次 `cats.find` + `t()`；方块上千时输入掉帧 | 预计算 `Map<catUuid, label>`，过滤时只查表 |
| 24 | `components/AccountActions.vue:37-80`、`windows/main/sidebar/MainSidebar.vue:650,749-766`、`windows/collect/CollectWindow.vue:1113-1117`、`windows/block/parts/BlockGrid.vue:302-311` | 可访问性：账户操作六个图标按钮只有 `v-tip` 没有 `aria-label`；侧栏实例行是 `div` + click，不能 Tab / Enter；收藏卡片同样；方块星标 / 删除角标是 `span`，CSS 写了“键盘聚焦时露出”但焦点永远到不了；侧栏“收起”按钮缺 aria-label | 图标按钮补 `aria-label`；行 / 卡片改 button 或补 `role+tabindex` 与 Enter/Space；角标改 button |
| 25 | `windows/settings/parts/SettingsGroup.vue:19-23` | 搜索命中高亮时加 `px-3 py-2.5 -mx-3 -my-2.5`，改变盒模型；`scrollIntoView` 后元素立刻变高，页面会跳一下 | 高亮只用 outline / box-shadow |
| 26 | `windows/settings/search.ts:97-130`（同 12-42、231） | 皮肤页 `rotX / rotY` 各登记 3 次，搜“旋转”出 6 条同文案同分组结果，`SettingsSearch` 的 key 重复；`programIcon`、实验性开关、本地文件三项没有搜索入口 | 索引项带所属小节区分；补齐缺失分组 |
| 27 | 结构 | 超过 400 行的 Vue：MainWindow 3239、ExportWindow 1496、MainSidebar 1351、CollectWindow 1251、AddInstanceWindow 1216、AccountWindow 989、StatsWindow 969、LogWindow 964；`server_instance/useServerInstance.ts` 1864 行 | 按 AGENTS §5.1 目录化：主窗口拆侧栏 / 详情 / 启动 / 首启；导出拆格式 / 文件树 / 模组；收藏拆卡片 / 分组 / 安装 |

## 已排除（计划中，不计入本轮）

- **联机大厅**：`components/HomePage.vue:149` 的入口只在源码构建显示、点击只弹“制作中”，全仓库没有 `openWindow("lobby")`；`windows/lobby/MappingLobby.vue:49-60` 的创建 / 加入按钮依赖后端 `available:false`，永远禁用。功能整体未上线，等排期时再核对入口接线。
- **云存档同步**：`windows/main/ctxmenu/MainCtxMenu.vue:88` + `MainWindow.vue:997-1000` 的实例右键项只在调试构建出现、点击只弹“制作中”。按计划中排除，上线时再接资源窗口的存档页。

## 已核对、判定不是问题（避免重复排查）

- 设置页 22 个可重置分组在 6 个 composable 里都有 `resetGroup` 分支。
- 账户 / 收藏 / 方块右键菜单项全部有真实实现，没有只弹 toast 的桩。
- 日志窗口的上传、崩溃分析、打开目录、测试启动 / 取消均接到真实命令。
- 下载暂停 / 继续 / 停止本身能生效（命令存在），问题只在防重复与错误提示。
- Java 下载列表、帮助页链接、导出 / 配置 / 服务端实例的主要动作均可用。
- `zh-CN.ts` 与 `en-US.ts` 各 2625 个键，完全对齐；窗口里没有缺失的静态 `t()` 键。
- 窗口的 interval / ResizeObserver / skinview3d viewer 都有对称清理；仅上面第 18 条是挂载时序问题。
