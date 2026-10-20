# mml-ipc —— M²L 的 IPC 契约（npm 包）

跨 IPC 的类型、命令包装、事件名与自定义主页面的 `window.mml` 契约，供
[mml-vue](../mml-vue) 与 [mml-ui-demo](../mml-ui-demo) 共用；类型与命令来自 Rust 源码生成。

> 本包只有 npm 内容，不含 Rust crate。生成器在仓库根 [`ipc-gen/`](../ipc-gen)。

## 目录

| 路径 | 来源 | 说明 |
| --- | --- | --- |
| `src/bindings.ts` | 生成 | `createCommands(invoke)` 命令包装，再导出全部类型 |
| `src/types/**` | 生成 | 跨 IPC 类型，按来源 .rs 一个文件；`types/index.ts` 是 barrel，跨文件引用自带 `import type` |
| `src/listens.ts` | 生成 | 事件名常量、`EventPayloads` 类型映射与 `createEvents(on)` 订阅函数 |
| `src/invoke.ts` | 手写 | 传输层类型 `Invoke`，把 Tauri / 自定义主页的差异挡在包外 |
| `src/home.ts` | 手写 | 自定义主页契约：`MmlApi`（`window.mml`），白名单事件从 `EventPayloads` 里挑 |
| `src/index.ts` | 手写 | 包入口，再导出上面几份 |

**`src/bindings.ts`、`src/listens.ts` 与 `src/types/**` 都是生成产物，不要手改。**
缺命令或字段时改 Rust 源码，再跑：

```bash
cd mml-gui && npm run gen
```

## 用法

命令包装不绑定传输层，调用方自己给 `invoke`：

```ts
// Tauri 页面（mml-vue）
import { invoke } from "@tauri-apps/api/core";
import { createCommands } from "mml-ipc";

const commands = createCommands(invoke);
await commands.main.getInstances();
```

```ts
// 自定义主页面（mml-ui-demo）：window.mml 由启动器注入
import { createCommands } from "mml-ipc";
import { invoke } from "./bridge";

const commands = createCommands((cmd, args) => invoke(cmd, args ?? undefined));
await commands.main.launchGame(uuid, null, null, null);
```

自定义主页面直接用包里的 `MmlApi` 类型，导入本包即带 `window.mml` 的全局声明：

```ts
import { LaunchState, type MmlApi, type StateEvent } from "mml-ipc";

const mml: MmlApi | undefined = window.mml;
const off = mml?.on(LaunchState, (e: StateEvent) => console.log(e.uuid, e.state));
```

事件除了名字，还生成了带 payload 类型的订阅函数，接上自己的 `on` 即可：

```ts
import { createEvents } from "mml-ipc";

const { onLaunchState, onGameExit } = createEvents((event, cb) => window.mml!.on(event, cb));
const off = onLaunchState((e) => console.log(e.uuid, e.state, e.progress));
```

类型可以直接按需导入（包入口会带上 `types/` 的全部类型）：

```ts
import type { ChunkFindDto, InstanceInfoDto, StateEvent } from "mml-ipc";
```

命令 / 事件名、分组与 DTO 的生成规则见仓库根 [AGENTS.md](../AGENTS.md) §4。
