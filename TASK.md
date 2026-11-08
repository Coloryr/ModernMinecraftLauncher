# TASK：跑全量测试并修复所有失败用例

## 目标

1. 先把仓库现有测试全部跑一遍，拿到**完整**的失败清单；
2. 逐个修复到全绿，不改变对外行为、不做顺手重构。

## 第一步：全量测试（先收集，别急着修）

默认的 `cargo test` 碰到第一个失败的测试二进制就停，会漏掉后面的失败项，所以第一次必须带
`--no-fail-fast`：

| 范围 | 命令 | 说明 |
| --- | --- | --- |
| Rust 全 workspace | `cargo test --workspace --no-fail-fast` | 只跑这一遍用于收集失败；之后修复验证按 crate 跑 |
| 桌面壳单测 | `cargo test -p mml-gui --lib` | 含 err_keys 与 server_instance 对账 |
| 前端单测 | `cd mml-vue; npm test` | node --test |
| 前端类型与 lint | `cd mml-vue; npx vue-tsc --noEmit; npx eslint .` | 只作为回归确认 |

把失败按「测试二进制 → 用例 → 断言差异」记进下面的清单，再开始改。

## 已知失败（本轮已确认）

`cargo test -p mml-game --test modpack_install`：

1. `install_curseforge_pack_end_to_end`
   - 位置：`mml-core/mml-game/tests/modpack_install.rs:241`
   - 断言：`assert_eq!(detected.name, "CF安装测试包")`
   - 实际：`left: "CF安装测试包-1.0"`，`right: "CF安装测试包"`

2. `install_modrinth_pack_end_to_end`
   - 位置：`mml-core/mml-game/tests/modpack_install.rs:336`
   - 断言：`assert_eq!(detected.name, "MR安装测试包")`
   - 实际：`left: "MR安装测试包-1.0.0"`，`right: "MR安装测试包"`

两条都是 `add_game::detect_pack` 返回的 `DetectedPack.name` 多了版本后缀。已用 `git stash`
去掉无关改动复现过，属既有失败，不是本轮改动引入的。

### 排查方向

- 看 `add_game::detect_pack_of`：CurseForge 的 `manifest.json`、Modrinth 的 `modrinth.index.json`
  分别从哪个字段取 name，有没有拼 `-版本`；再看测试夹具 `make_curseforge_pack` /
  `make_modrinth_pack` 写进清单的 name 是不是本身就带了版本。
- 判据：`detect_pack` 的 `name` 语义是「包名」还是「包名-版本」。安装后的实例名、发布端清单
  都在用它，先确认哪个是正确口径，再决定改代码还是改断言（**优先改与语义一致的那一侧**，
  不要为了过测试把两处都改掉）。
- 夹具里若本来就是「名称 + 版本」，修断言时同时检查同文件里后面所有对实例名的断言
  （`instance.name` 等），别只修第一条。

## 修复原则

- 一次只修一类失败；改完先跑该测试二进制，再跑所属 crate。
- 不改测试来掩盖真实回归：先判断是代码错还是断言过时（看提交历史 / 语义 / 调用方）。
- 不引入新依赖、不改公共 API、不顺手格式化无关文件。
- 浮点 / 时间 / 网络相关的用例若偶发失败，先复跑确认，再决定是否属于 flaky（属 flaky 的
  在用例里说明并加等待/重试，不要直接 `#[ignore]`）。

## 验收

- `cargo test --workspace --no-fail-fast` 全绿（网络 / Java 缺失而 `return` 的用例不算失败）。
- `cargo test -p mml-gui --lib`、`cd mml-vue && npm test` 全绿。
- 前端 `npx vue-tsc --noEmit` 与 `npx eslint .` 无 error。
- 失败清单里每一条都有结论（修好 / 判定为环境原因并写明）。

## 范围

- 只改 `E:\code\ModernMinecraftLauncher`；不动 `E:\code\mml-server-sync`。
- 不改 `server.json` 结构、不改四种配置目录同步方式、不改 mml-link 的行为。
