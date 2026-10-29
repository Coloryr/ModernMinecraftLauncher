# ColorMC 迁移测试样本

这是**真实的 ColorMC 工作目录**（Windows / Debug 构建）裁出来的一份，供迁移功能
（`src/windows/colormc.rs`）的用例使用 —— 与 `mml-core/mml-game/tests/packs/` 里的真实启动器
导出包同一个路数：**格式对不对用真数据验**，不靠自己编一份可能走样的。

来源：`<ColorMC 仓库>\src\ColorMC.Launcher\bin\Debug\net10.0\colormc\`

## 搬进来的

| 条目 | 说明 |
| --- | --- |
| `config.json` | ColorMC 的全局配置（真实，2635 B） |
| `collect.json` | 收藏（真实，2402 B） |
| `count.dat` | 游玩统计（真实，897 B） |
| `gui.json` / `window.json` / `cloud.json` / `frp.json` / `lock` | 工作目录根的真实小文件 |
| `logs.log` | **裁剪过**：真实那份 2.3 MB，这里只留开头 64 KB（按字节截、末尾对齐到换行，不改编码） |
| `minecraft/instances/` | **三个真实实例**，原样搬运 |

三个实例（就是 ColorMC 自己建的，名字与内容都是真的）：

- `1.12.2-Forge-14.23.5.2864` —— `game.json` + `launch.json` + `.minecraft/`
- `1.21.11-NeoForge-21.11.38-beta` —— 另有 `guisetting.json`（ColorMC 的界面设置）、`modfileinfo.json`
- `26.1-NeoForge-26.1.0.19-beta`

## 没搬进来的（体积原因）

真实工作目录里还有下面这些，用例按需要**临时建同名空目录**代替：

| 条目 | 真实体积 |
| --- | --- |
| `download/` | 933 MB（ColorMC 的下载缓存） |
| `image/` | 223 MB（图片缓存） |
| `block/` | 6.2 MB / 2346 个文件（方块图） |
| `minecraft/assets`、`minecraft/libraries`、`minecraft/versions` | 合计约 1.7 GB（共享资源、依赖库、版本库） |
| `java/`、`tools/`、`frpc/`、`inputs/` | 空目录（git 不跟踪空目录） |

**jar 二进制不入仓**：`mods/`、`.minecraft/.cache/jij/` 里那几个原本是真实的 mod / coremod jar（最大 1.4 MB），现在每个位置放一个同名 `.jar.txt` 占位（内容写明它替的是多大的 jar）—— 目录结构与文件名保持真实，用例要走「复制整个目录树」这条路径照旧，仓库里不带任何二进制。

被替换掉的 5 个 jar：

| 位置 | 真实大小 |
| --- | --- |
| `1.21.11-…/.minecraft/mods/jei-1.21.11-neoforge-27.4.0.15.jar` | 1456 KB |
| `1.21.11-…/.minecraft/.cache/jij/<sha>/mixinextras-neoforge-0.5.0.jar` | 706 KB |
| `1.21.11-…/.minecraft/.cache/jij/<sha>/net.neoforged.neoforge-coremods-21.11.38-beta.jar` | 14 KB |
| `26.1-…/.minecraft/.cache/jij/<sha>/mixinextras-neoforge-0.5.3.jar` | 709 KB |
| `26.1-…/.minecraft/.cache/jij/<sha>/net.neoforged.neoforge-coremods-26.1.0.19-beta.jar` | 14 KB |

## 注意

- 用例**只读**这份样本：移动模式的用例会先把样本复制到 `target/temp` 再搬，不会动这里。
- 样本里**没有** `auth.json` —— ColorMC 的账户文件不在工作目录里（在
  `%LOCALAPPDATA%\ColorMC\auth.json`），迁移时由账户迁移单独合并，不随工作目录搬运；
  这里不放真实令牌。

## 用法

用例在 `src/windows/colormc.rs` 的 `#[cfg(test)]` 模块里，模块路径含 `colormc`，按它过滤即可：

```bash
# 仓库根目录
cargo test -p mml-gui colormc
```

用例共用 `crate::test_support` 里的进程级 boot（内核单例每进程只能起一次），临时目录走
`mml_testutil`、落在仓库的 `target/temp` 下 —— 都不在系统 `%TEMP%`，也都不写回本目录。
