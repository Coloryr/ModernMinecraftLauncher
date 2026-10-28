# 前端页面截图目录

`mml-vue` 把每个窗口与状态做成一个固定截图场景，另有一组控件预览页（`control-*`）。
每个场景都有独立名字，可以只渲染其中一张，不需要等整批跑完。

## 用法

```bash
cd mml-vue

# 只跑一张：名字就是下面的「图片」列
npm run shots -- --scene=main-grid
npm run shots -- main-grid

# 一次跑多张
npm run shots -- --scene=main-grid,settings-ui

# 按前缀成组跑
npm run shots -- --only=settings
npm run shots -- --only=control

# 看全部名字 / 输出 Markdown 表
npm run shots -- --list
npm run shots -- --list=md

# 清空旧截图后整批重跑
npm run shots -- --clean
```

只跑部分场景时，输出目录里的其他截图会保留，可以逐张补齐；`--clean` 才会先清空。
浏览器默认用 Windows Edge，其他平台用 `--edge=<路径>` 指定；端口默认 1420，可用 `--port=` 改。
更多细节见 [`mml-vue/scripts/screenshot/README.md`](../mml-vue/scripts/screenshot/README.md)。

产物：

- `target/screens/<图片名>.png`：每个场景一张，视口见下表；
- `target/screens/index.html`：本次与历史截图的索引页；
- `target/screens/report.json`：失败原因、页面错误、未 mock 的命令。

## 场景表

共 216 个场景，按窗口分组。
「窗口」列是页面的 `kind`，窗口族场景会自动带上实例参数，不需要在命令里另写；
控件场景的 `control` 是截图工具里的预览页，不是启动器窗口。

### 主窗口

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `main-grid` | 主窗口 · 网格视图 | `main` | 1280×800 |
| `main-group` | 主窗口 · 分组视图 | `main` | 1280×800 |
| `main-detail` | 主窗口 · 实例详情 | `main` | 1280×800 |
| `main-detail-scroll` | 主窗口 · 实例详情 · 滚动 | `main` | 1280×900 |
| `main-advanced` | 主窗口 · 实例设置 · 扩展参数 | `main` | 1280×900 |
| `main-advanced-scroll` | 主窗口 · 实例设置 · 扩展参数 · 滚动 | `main` | 1280×900 |
| `main-home` | 主窗口 · 主页 | `main` | 1280×800 |
| `main-empty` | 主窗口 · 空状态 | `main` | 1280×800 |
| `main-ctx-menu` | 主窗口 · 右键菜单 | `main` | 1280×800 |
| `main-rename-modal` | 主窗口 · 重命名弹窗 | `main` | 1280×800 |
| `main-copy-modal` | 主窗口 · 复制弹窗 | `main` | 1280×800 |
| `main-group-menu` | 主窗口 · 分组菜单 | `main` | 1280×800 |
| `main-move-group` | 主窗口 · 转移分组 | `main` | 1280×800 |
| `main-delete-group` | 主窗口 · 删除分组 | `main` | 1280×800 |
| `main-multi-bar` | 主窗口 · 多选工具条 | `main` | 1280×800 |
| `main-multi-delete` | 主窗口 · 批量删除 | `main` | 1280×800 |
| `main-search-empty` | 主窗口 · 搜索 · 空状态 | `main` | 1280×800 |
| `main-add-group-modal` | 主窗口 · 添加分组弹窗 | `main` | 1280×800 |
| `main-launch` | 主窗口 · 点击启动（断言命令） | `main` | 1280×800 |
| `main-create-group` | 主窗口 · 新建分组（断言命令） | `main` | 1280×800 |

### 设置

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `settings-ui` | 设置 · 界面 | `settings` | 1100×760 |
| `settings-java` | 设置 · Java | `settings` | 1100×860 |
| `settings-network` | 设置 · 网络与下载 | `settings` | 1100×780 |
| `settings-launch` | 设置 · 游戏启动 | `settings` | 1100×780 |
| `settings-client` | 设置 · 客户端设置 | `settings` | 1100×780 |
| `settings-gamepad` | 设置 · 手柄 | `settings` | 1100×860 |
| `settings-skin` | 设置 · 皮肤与头像 | `settings` | 1100×800 |
| `settings-local` | 设置 · 本地文件 | `settings` | 1100×760 |
| `settings-experimental` | 设置 · 实验性功能 | `settings` | 1100×760 |
| `settings-search` | 设置 · 搜索 | `settings` | 1100×760 |
| `settings-java-scanning` | 设置 · Java · 扫描中 | `settings` | 1100×860 |
| `settings-reset-modal` | 设置 · 恢复默认弹窗 | `settings` | 1100×760 |
| `settings-java-remove-all` | 设置 · Java · 删除全部 | `settings` | 1100×860 |
| `settings-client-lock` | 设置 · 客户端设置 · 登录方式锁定 | `settings` | 1100×780 |
| `settings-local-clear` | 设置 · 本地文件 · 清理确认 | `settings` | 1100×760 |
| `settings-gamepad-new` | 设置 · 手柄 · 新建配置 | `settings` | 1100×860 |
| `settings-save` | 设置 · 改动保存（断言命令） | `settings` | 1100×760 |

### 资源管理

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `resource-mods` | 资源管理 · 模组 | `resource` | 1280×800 |
| `resource-mods-table` | 资源管理 · 模组 · 表格 | `resource` | 1280×800 |
| `resource-packs` | 资源管理 · 资源包 | `resource` | 1280×800 |
| `resource-saves` | 资源管理 · 存档 | `resource` | 1280×800 |
| `resource-shaders` | 资源管理 · 光影 | `resource` | 1280×800 |
| `resource-servers` | 资源管理 · 服务器 | `resource` | 1280×800 |
| `resource-schematics` | 资源管理 · 投影 | `resource` | 1280×800 |
| `resource-screenshots` | 资源管理 · 截图 | `resource` | 1280×800 |
| `resource-empty` | 资源管理 · 空状态 | `resource` | 1280×800 |
| `resource-group-modal` | 资源管理 · 分组弹窗 | `resource` | 1280×800 |
| `resource-mod-note` | 资源管理 · 模组 · 备注 | `resource` | 1280×800 |
| `resource-auto-mark` | 资源管理 · 自动标记 | `resource` | 1280×800 |
| `resource-row-menu` | 资源管理 · 行右键菜单 | `resource` | 1280×800 |
| `resource-server-edit` | 资源管理 · 服务器 · 编辑 | `resource` | 1280×800 |
| `resource-group-menu` | 资源管理 · 分组菜单 | `resource` | 1280×800 |
| `resource-update-check` | 资源管理 · 更新检查 | `resource` | 1280×800 |
| `resource-update-log` | 资源管理 · 更新记录 | `resource` | 1280×800 |
| `resource-dep-test` | 资源管理 · 依赖检测 | `resource` | 1280×800 |
| `resource-dep-graph` | 资源管理 · 依赖图 | `resource` | 1280×800 |
| `resource-server-add` | 资源管理 · 服务器 · 添加 | `resource` | 1280×800 |
| `resource-export-modal` | 资源管理 · 导出弹窗 | `resource` | 1280×800 |

### 账户

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `account-loaded` | 账户 · 已加载 | `account` | 1000×700 |
| `account-empty` | 账户 · 空状态 | `account` | 1000×700 |
| `account-list` | 账户 · 列表视图 | `account` | 1000×700 |
| `account-detail` | 账户 · 详情视图 | `account` | 1000×700 |
| `account-list-no-match` | 账户 · 列表视图 · 无匹配 | `account` | 1000×700 |
| `account-detail-no-match` | 账户 · 详情视图 · 无匹配 | `account` | 1000×700 |
| `account-add` | 账户 · 添加 | `account` | 1000×700 |
| `account-add-microsoft` | 账户 · 添加 · 微软 | `account` | 1000×700 |
| `account-add-littleskin` | 账户 · 添加 · LittleSkin | `account` | 1000×700 |
| `account-add-authlib` | 账户 · 添加 · 外置登录 | `account` | 1000×700 |
| `account-add-invalid` | 账户 · 添加 · 未填用户名 | `account` | 1000×700 |
| `account-add-invalid-authlib` | 账户 · 添加 · 未填服务器与账密 | `account` | 1000×700 |
| `account-edit` | 账户 · 编辑 | `account` | 1000×700 |
| `account-delete` | 账户 · 删除 | `account` | 1000×700 |
| `account-relogin` | 账户 · 重新登录 | `account` | 1000×700 |
| `account-oauth` | 账户 · 设备码登录 | `account` | 1000×700 |
| `account-tooltip` | 账户 · 提示 | `account` | 1000×700 |
| `account-context` | 账户 · 右键 | `account` | 1000×700 |
| `account-oauth-xbox` | 账户 · 设备码登录 · Xbox | `account` | 1000×700 |
| `account-oauth-xsts` | 账户 · 设备码登录 · XSTS | `account` | 1000×700 |
| `account-oauth-token` | 账户 · 设备码登录 · 令牌交换 | `account` | 1000×700 |
| `account-oauth-profile` | 账户 · 设备码登录 · 拉取档案 | `account` | 1000×700 |
| `account-oauth-fail` | 账户 · 设备码登录 · 失败 | `account` | 1000×700 |
| `account-skin` | 账户 · 皮肤 | `account` | 1000×720 |
| `account-add-ok` | 账户 · 添加成功（断言命令） | `account` | 1000×700 |

### 添加实例

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `add-new` | 添加实例 · 新建 | `add` | 1100×760 |
| `add-archive` | 添加实例 · 压缩包 | `add` | 1100×760 |
| `add-folder` | 添加实例 · 文件夹 | `add` | 1100×760 |
| `add-online` | 添加实例 · 在线地址 | `add` | 1100×760 |
| `add-new-loading` | 添加实例 · 新建 · 加载中 | `add` | 1100×760 |
| `add-new-loaded` | 添加实例 · 新建 · 已加载 | `add` | 1100×760 |
| `add-new-type-drop` | 添加实例 · 新建 · 类型下拉 | `add` | 1100×760 |
| `add-new-group-drop` | 添加实例 · 新建分组下拉 | `add` | 1100×760 |
| `add-new-version-loading` | 添加实例 · 新建 · 版本 · 加载中 | `add` | 1100×760 |
| `add-new-version-loaded` | 添加实例 · 新建 · 版本 · 已加载 | `add` | 1100×760 |
| `add-new-loader-version-loading` | 添加实例 · 新建 · 加载器 · 版本 · 加载中 | `add` | 1100×760 |
| `add-new-loader-version-loaded` | 添加实例 · 新建 · 加载器 · 版本 · 已加载 | `add` | 1100×760 |
| `add-new-empty-error` | 添加实例 · 新建 · 空状态 · 错误 | `add` | 1100×760 |
| `add-new-version-error` | 添加实例 · 新建 · 版本 · 错误 | `add` | 1100×760 |
| `add-archive-loading` | 添加实例 · 压缩包 · 加载中 | `add` | 1100×760 |
| `add-archive-loaded` | 添加实例 · 压缩包 · 已加载 | `add` | 1100×760 |
| `add-archive-empty-error` | 添加实例 · 压缩包 · 空状态 · 错误 | `add` | 1100×760 |
| `add-archive-read-error` | 添加实例 · 压缩包读取失败 | `add` | 1100×760 |
| `add-folder-confirm` | 添加实例 · 文件夹 · 确认 | `add` | 1100×760 |
| `add-folder-loading` | 添加实例 · 文件夹 · 加载中 | `add` | 1100×760 |
| `add-folder-loaded` | 添加实例 · 文件夹 · 已加载 | `add` | 1100×760 |
| `add-folder-tree-loaded` | 添加实例 · 文件夹 · 目录树 · 已加载 | `add` | 1100×760 |
| `add-folder-scan-error` | 添加实例 · 文件夹扫描失败 | `add` | 1100×760 |
| `add-folder-empty-error` | 添加实例 · 文件夹 · 空状态 · 错误 | `add` | 1100×760 |
| `add-online-probing` | 添加实例 · 在线地址 · 探测中 | `add` | 1100×760 |
| `add-online-loaded` | 添加实例 · 在线地址 · 已加载 | `add` | 1100×760 |
| `add-online-error` | 添加实例 · 在线地址 · 错误 | `add` | 1100×760 |
| `add-online-empty-error` | 添加实例 · 在线地址 · 空状态 · 错误 | `add` | 1100×760 |
| `add-continue-modal` | 添加实例 · 继续安装弹窗 | `add` | 1100×760 |
| `add-name-conflict-modal` | 添加实例 · 重名弹窗 | `add` | 1100×760 |
| `add-pack-progress-modal` | 添加实例 · 整合包进度弹窗 | `add` | 1100×760 |
| `add-resource-loaded` | 添加资源 · 搜索结果 | `add_resource` | 1180×800 |

### 下载整合包

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `addmodpack` | 下载整合包 | `add_modpack` | 1150×800 |
| `addmodpack-detail` | 下载整合包 · 详情 | `add_modpack` | 1150×800 |
| `addmodpack-manual` | 下载整合包 · 手动 | `add_modpack` | 1150×800 |
| `addmodpack-unstar` | 下载整合包 · 取消收藏 | `add_modpack` | 1150×800 |
| `addmodpack-loading` | 下载整合包 · 加载中 | `add_modpack` | 1150×800 |
| `addmodpack-error` | 下载整合包 · 错误 | `add_modpack` | 1150×800 |
| `addmodpack-install` | 下载整合包 · 安装 | `add_modpack` | 1150×800 |
| `addmodpack-install-popup` | 下载整合包 · 安装进度弹窗 | `add_modpack` | 1150×800 |
| `addmodpack-files` | 下载整合包 · 文件 | `add_modpack` | 1150×900 |
| `addmodpack-install-error` | 下载整合包 · 安装失败 | `add_modpack` | 1150×800 |

### 添加资源

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `add-resource` | 添加资源 | `add_resource` | 1200×800 |

### 收藏

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `collect-loaded` | 收藏 · 已加载 | `collect` | 1100×760 |
| `collect-card-menu` | 收藏 · 卡片菜单 | `collect` | 1100×760 |
| `collect-remove-modal` | 收藏 · 移除弹窗 | `collect` | 1100×760 |
| `collect-clear-modal` | 收藏 · 清空弹窗 | `collect` | 1100×760 |
| `collect-delete-group-modal` | 收藏 · 删除分组弹窗 | `collect` | 1100×760 |
| `collect-add-group-modal` | 收藏 · 添加分组弹窗 | `collect` | 1100×760 |
| `collect-install-modal` | 收藏 · 安装确认弹窗 | `collect` | 1100×760 |
| `collect-empty` | 收藏 · 空状态 | `collect` | 1100×760 |

### 下载

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `download-loaded` | 下载 · 已加载 | `download` | 1000×700 |
| `download-empty` | 下载 · 空状态 | `download` | 1000×700 |
| `download-paused` | 下载 · 已暂停 | `download` | 1000×700 |
| `download-stop-modal` | 下载 · 停止确认弹窗 | `download` | 1000×700 |
| `download-pause` | 下载 · 暂停全部（断言命令） | `download` | 1000×700 |
| `download-failed` | 下载 · 有失败任务 | `download` | 1000×700 |

### Java 下载

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `java-download` | Java 下载 · 列表 | `java_download` | 1000×720 |
| `java-loading` | Java 下载 · 加载中 | `java_download` | 1000×720 |
| `java-download-start` | Java 下载 · 点击下载（断言命令） | `java_download` | 1000×700 |
| `java-download-empty` | Java 下载 · 无匹配 | `java_download` | 1000×700 |

### 方块与物品

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `block-empty` | 方块与物品 · 空状态 | `block` | 1150×800 |
| `block-rendered` | 方块与物品 · 已渲染 | `block` | 1150×800 |
| `block-rendering` | 方块与物品 · 渲染中 | `block` | 1150×800 |
| `block-error` | 方块与物品 · 错误 | `block` | 1150×800 |
| `block-detail` | 方块与物品 · 详情 | `block` | 1150×800 |
| `block-multi` | 方块与物品 · 多选 | `block` | 1150×800 |
| `block-group-menu` | 方块与物品 · 分组菜单 | `block` | 1150×800 |
| `block-new-group` | 方块与物品 · 新建分组 | `block` | 1150×800 |
| `block-set-group` | 方块与物品 · 设置分组 | `block` | 1150×800 |
| `block-group-menu-custom` | 方块与物品 · 分组菜单 · 自定义 | `block` | 1150×800 |
| `block-group-rename` | 方块与物品 · 分组重命名 | `block` | 1150×800 |
| `block-group-remove` | 方块与物品 · 移除分组 | `block` | 1150×800 |
| `block-skin-add` | 方块与物品 · 添加皮肤 | `block` | 1150×800 |
| `block-close-confirm` | 方块与物品 · 关闭确认 | `block` | 1150×800 |

### 游戏日志

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `log` | 游戏日志 | `log` | 1100×720 |
| `log-stop-modal` | 游戏日志 · 停止弹窗 | `log` | 1100×720 |
| `log-crash-modal` | 游戏日志 · 崩溃分析 | `log` | 1100×720 |
| `log-upload-modal` | 游戏日志 · 上传弹窗 | `log` | 1100×720 |
| `log-empty` | 日志 · 无输出 | `log` | 1100×780 |

### 导出实例

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `export` | 导出实例 | `export` | 1100×760 |
| `export-files` | 导出实例 · 文件 | `export` | 1100×800 |
| `export-meta-filled` | 导出实例 · 元数据已填 | `export` | 1100×800 |
| `export-mods` | 导出实例 · 模组 | `export` | 1100×800 |
| `export-curseforge` | 导出实例 · CurseForge | `export` | 1100×800 |
| `export-mmc` | 导出实例 · MMC | `export` | 1100×800 |
| `export-hmcl` | 导出实例 · HMCL | `export` | 1100×800 |
| `export-modrinth` | 导出实例 · Modrinth | `export` | 1100×760 |
| `export-progress` | 导出实例 · 进度 | `export` | 1100×760 |

### 联机大厅

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `lobby` | 联机大厅 | `lobby` | 1050×720 |
| `lobby-waiting` | 联机大厅 · 等待中 | `lobby` | 1050×720 |
| `lobby-error` | 联机大厅 · 错误 | `lobby` | 1050×720 |

### 配置文件

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `config` | 配置文件 | `config` | 1240×820 |
| `config-file` | 配置文件 · 文件 | `config` | 1240×820 |
| `config-tree` | 配置文件 · 目录树 | `config` | 1240×820 |
| `config-nbt` | 配置文件 · NBT | `config` | 1240×820 |
| `config-mca` | 配置文件 · MCA | `config` | 1240×820 |
| `config-mca-chunk` | 配置文件 · MCA · 区块 | `config` | 1240×820 |
| `config-image` | 配置文件 · 图片 | `config` | 1240×820 |
| `config-hex` | 配置文件 · 十六进制 | `config` | 1240×820 |
| `config-context` | 配置文件 · 右键 | `config` | 1240×820 |
| `config-folder-context` | 配置文件 · 文件夹右键菜单 | `config` | 1240×820 |

### 服务器实例

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `server-instance` | 服务器实例 | `server_instance` | 1050×760 |
| `server-instance-mods` | 服务器实例 · 模组 | `server_instance` | 1050×800 |
| `server-instance-packs` | 服务器实例 · 资源包 | `server_instance` | 1050×800 |
| `server-instance-configs` | 服务器实例 · 配置 | `server_instance` | 1050×800 |
| `server-instance-start` | 服务器实例 · 启动 | `server_instance` | 1050×800 |
| `server-instance-progress` | 服务器实例 · 进度 | `server_instance` | 1050×800 |

### 服务器包

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `server-pack` | 服务器包 | `server_pack` | 1100×760 |
| `server-pack-filled` | 服务器包 · 已填充 | `server_pack` | 1100×800 |
| `server-pack-progress` | 服务器包 · 进度 | `server_pack` | 1100×800 |
| `server-pack-done` | 服务器包 · 完成 | `server_pack` | 1100×800 |
| `server-pack-error` | 服务器包 · 错误 | `server_pack` | 1100×800 |

### 帮助

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `help` | 帮助 | `help` | 1050×760 |

### 统计

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `stats` | 统计 | `stats` | 1150×800 |
| `stats-other` | 统计 · 其他 | `stats` | 1150×800 |
| `stats-single-day` | 统计 · 单日 | `stats` | 1150×800 |
| `stats-empty` | 统计 · 无数据 | `stats` | 1150×800 |

### 控件

| 图片 | 画面 | 窗口 | 视口 |
| --- | --- | --- | --- |
| `control-buttons` | 控件 · 按钮 | `control` | 1000×620 |
| `control-inputs` | 控件 · 输入框 | `control` | 1000×360 |
| `control-selects` | 控件 · 下拉框 | `control` | 1000×380 |
| `control-select-open` | 控件 · 下拉框 · 展开 | `control` | 1000×600 |
| `control-switches` | 控件 · 开关 | `control` | 1000×300 |
| `control-segmented` | 控件 · 分段切换 | `control` | 1000×380 |
| `control-steppers` | 控件 · 数字步进器 | `control` | 1000×300 |
| `control-search` | 控件 · 搜索框 | `control` | 1000×300 |
| `control-form` | 控件 · 表单行 | `control` | 1000×520 |
| `control-date` | 控件 · 日期选择 | `control` | 1000×340 |
| `control-date-open` | 控件 · 日期选择 · 展开 | `control` | 1000×620 |
| `control-modal` | 控件 · 弹窗 | `control` | 1000×700 |
| `control-collapse` | 控件 · 折叠面板 | `control` | 1000×380 |
| `control-tabs` | 控件 · 图标页签 | `control` | 1000×400 |
| `control-icons` | 控件 · 图标 | `control` | 1100×580 |
