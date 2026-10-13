# task1.md · 待修错误清单

> 临时任务文件，不是仓库文档：这批任务做完就删掉（与 AGENTS.md §6.1 的 TASK.md 同规则）。
> 每条都写清「现象 / 证据 / 位置 / 候选修法 / 验收」，照着能直接动手。

## P0 · 删实例走回收站失败，6 个用例红

**现象**：删实例时 `SHFileOperationW` 返回 `0x8`，删除失败，实例留在盘上。

**证据**（本机实测）：

```
cargo test -p mml-game --test real_launcher_packs
  install_real_hmcl_modpack ... FAILED
  install_real_hmcl_portable_pack ... FAILED
  install_real_mml_instance_archive ... FAILED
  install_real_multimc_dir ... FAILED
  install_real_mmc_instance ... FAILED
  mml_instance_archive_roundtrip ... FAILED   ← 卡在收尾的 delete_instance
  mmc_and_hmcl_export_roundtrip ... ok        ← 新加的用例，不删东西，绿

FileSystemError(FileSystemErrorData {
    path: "...\\target\\temp\\mml-real-launcher-packs-<pid>\\minecraft\\instances\\<实例名>",
    error: "SHFileOperationW failed with error code: 0x8",
})
```

**已确认与本次导出改动无关**：把我的新用例过滤掉后这 6 条照样红，失败点全在
`mml_game::delete_instance` / `add_game` 的安装收尾，没碰过导出路径。

**位置**：

- 调用方：`mml-core/mml-game/src/lib.rs:740` `delete_instance`，实删在 `lib.rs:1010`
  的 `path_helper::move_to_trash(self.get_base_path())`
- 实现：`mml-core/mml-sys/src/path_helper.rs:250` `move_to_trash_windows`
  → `move_to_trash`（`path_helper.rs:128` 起）

**排查步骤**（按顺序，先分清是"路径特有"还是"全局"）：

1. 用**真实实例目录**（不是 `target\temp`）删一次实例，看是否同样 0x8 —— 只红在 temp
   说明与该盘的回收站设置有关，两条路都要修但不能混为一谈；
2. 确认 `0x8` 到底是谁返回的：文档里 `SHFileOperationW` 的错误码是 `0x71` 起的 `DE_*`
   系列，**没有 0x8**。现在的代码只把 `0` 当成功、`0x71` 当用户取消，其余一律按错误报，
   所以先要把真实返回码打出来（临时日志带 `[TEMP]` 前缀，查完删掉，见 AGENTS.md §13）；
3. 顺便看 `fAnyOperationsAborted`：结构体字段填了却**从来没检查过**，用户取消时会拿一个
   无关的返回码去报错。

**候选修法**（从代价小到大，前两条优先）：

1. 工作线程里调 `SHFileOperationW` 前先 `CoInitializeEx(None, COINIT_APARTMENTTHREADED)`
   （结束再 `CoUninitialize`）。该 API 要求调用线程初始化 COM，现在新起的线程没做这件事，
   是这类"神秘错误码"的常见来源；
2. 补 `fAnyOperationsAborted` 检查，取消按 `ErrorType::TaskCancel` 返回，别再落到错误分支；
3. 回收站确实不可用（盘符不支持、组策略禁用）时**回退成直接删除**，并记一条长期日志说明
   "文件没有进回收站"——这一步是产品行为改动（用户会失去从回收站找回实例的机会），
   **动手前先确认**；
4. 换 `IFileOperation`（现代 API）替掉 `SHFileOperationW`，可选，不着急。

**验收**：`cargo test -p mml-game --test real_launcher_packs` 全绿（含原有 6 条）；
删实例后实例目录消失，回收站里能看到它（回退直接删除的分支除外）。

## P1 · `ExportPackType::Zip` 还是 `todo!()`

- **现象**：`mml-core/mml-game/src/game_export.rs:110` 是 `ExportPackType::Zip => todo!()`，
  只要有调用方传 `Zip` 就会 panic。
- **现状**：导出窗口只发 `curseforge` / `modrinth` / `mmc` / `hmcl` 四种，走不到这里；
  `ExportPackType::MML` 也没有前端入口。
- **候选修法**：要么把 `Zip` 实现成"只打包文件、不写清单"，要么把变体删掉。
- **验收**：`game_export.rs` 里不再有 `todo!()`；`cargo check -p mml-game` 通过。

## P1 · 下载窗口轮询把 IPC 异常静默吞掉

- **现象**：`mml-vue/src/windows/download/DownloadWindow.vue` 的 `refresh()` 里
  `catch { }` 保持空状态，注释本意是"纯浏览器模式无 IPC"（`~169` 行）。后端真出错时
  界面只会一直显示"所有下载线程空闲"，日志里什么都没有。
- **候选修法**：只在 Tauri 环境下、且**只报一次**（避免 700ms 一次的刷屏）打
  `console.warn`（eslint 放行 `warn` / `error`）。
- **验收**：`npm run build` 通过；故意让 `download_get_status` 失败时控制台出现一条警告。

## P2 · 核心 i18n 语言不跟随界面

- **现象**：内核日志走 `mml_names::i18`，语言由 `mml_names::init` 读 `lang.txt` 决定，
  而全仓**没有写 `lang.txt` 的地方**，所以核心日志永远是中文——界面切到 English 也一样。
- **候选修法**：`gui_config.locale` 变化时 `mml_names::i18::set(Box::new(ZhCn / EnUs))`，
  或写一份 `lang.txt`。二选一，需先定口径。
- **验收**：界面切英文后，内核侧日志变英文。

## P2 · `ExportInfoDto` 三个字段已无人使用

- **现象**：导出窗口改版后不再显示版本与加载器，`ExportInfoDto` 的 `version` /
  `loader` / `loaderVersion` 前端一处没用了（清单里的版本与加载器由内核从实例设置取，
  与这个 DTO 无关）。
- **候选修法**：连同 `export_get_info` 里的赋值一起删掉；想留着以后在「整合包信息」页
  显示就保持原样。
- **验收**：删了的话 `cargo check -p mml-gui` + `npm run build` 通过。

## P2 · MMC / HMCL 表达不了自定义加载器（已知限制）

- **现象**：`mmc()` / `hmcl()` 只为 Forge / NeoForge / Fabric / Quilt 写加载器组件，
  自定义加载器（`LoaderType::Custom`）导出的包里只有游戏版本。
- **现状**：已在两个函数的 `# 返回值` 里写明，不是 bug，属于格式本身的限制。
- **候选修法**：导出时给一条提示，或在包内加自定义加载器信息（两种格式都没有标准位置）。
