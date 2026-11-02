# 手柄读取换 SDL2

## 目标

用 `sdl2` crate 替换 `gilrs`，把手柄读取统一到 SDL，Windows 上不再需要自己维护 XInput 兜底。

## 为什么换

- WGI（gilrs 在 Windows 的默认后端）**要求进程有前台窗口**才收得到手柄事件；游戏在前台、启动器在后台时完全静默，而手柄映射正好是这个场景。
- SDL 自带多驱动（XInput / WGI / RawInput / HIDAPI / DirectInput）并去重。本机实测同一只手柄：
  `name="Xbox Series X Controller"`、`num_joysticks()=1`、自带标准映射串（a:b0,b:b1,…,lefttrigger:a4,righttrigger:a5）。
- 换掉后可以**删掉自己的 XInput 实现**与「XInput 兼容模式」开关，少一套代码、少一个设置项。

## 版本选择

**SDL2**（`sdl2 = { version = "0.38", features = ["bundled"] }`）：

- 已在本机实测：CMake 4.3 + MSVC，clean build 约 65 秒，静态链接，发布物不带 DLL；
- `sdl2` crate 0.38（2025-12，567 万下载）稳定；`sdl3` crate 0.22（2026-01，27 万下载）仍是 0.x，
  新 API 对手柄读取没有实质收益。SDL2 进维护期不影响这次改动；以后要换 SDL3，读取层是隔离的。

## 步骤

1. **依赖**：根 `Cargo.toml` 的 workspace 依赖把 `gilrs` 换成 `sdl2`（`features = ["bundled"]`）；
   `mml-core/mml-gamepad/Cargo.toml` 同步，并去掉 `[target.'cfg(windows)'.dependencies] windows`。
2. **读取层重写**：`mml-core/mml-gamepad/src/reader.rs`
   - `Gilrs::new()` → `sdl2::init()` + `sdl.game_controller()` + `sdl.event_pump()`，全部在读取线程里创建（SDL 上下文不是 `Send`）；
   - 事件：`ControllerDeviceAdded` / `ControllerDeviceRemoved` / `ControllerButtonDown` / `Up` / `ControllerAxisMotion`；
   - 打开设备：`controllers.open(index)`，保存 `GameController` 句柄，别只存 index；
   - 轴：`ControllerAxis` 的 `LeftX/LeftY/RightX/RightY` 除 32767；`TriggerLeft/TriggerRight` 除 32767，同时写按钮模拟量；
   - 按钮：`ControllerButton` 的 A/B/X/Y、Back/Guide/Start、LeftStick/RightStick、Shoulder、DPad、`Paddle1..4` 映射到 `extras`。
3. **删除旧路径**：`src/xinput.rs`；`reader.rs` 里的 `XINPUT` / `XINPUT_SLOTS` / `sync_xinput` / `claim_xinput` /
   `adopt_xinput_name` / `ReaderSettings` / `set_xinput` / `load_settings`（连带 `GAMEPAD_READER_FILE`、
   `map.rs` 的扫描排除、`names` 测试登记、`lib.rs` 的 `mod xinput` 与导出）。
4. **IPC / 前端**：删 `gamepad_xinput` / `gamepad_set_xinput` 两条命令与生成的 `commands.gamepad.xinput` / `.setXinput`；
   删 `GamepadTab.vue` 的「XInput 兼容模式」开关、`useSettingsGamepad` 的 `xinput` / `setXinput`、两条 i18n 文案。
5. **其它**：`examples/gamepad_demo.rs` 去掉"前后两段切换 XInput"的逻辑，改成直接读；
   `examples/probe.rs` 同步；changelog 里那条「XInput 兼容模式」改写成本次改动的说明。

## 需要注意

- 事件循环自己节拍：`pump.poll_event()` 取空后 `sleep 8~16ms`，不能空转；
- `SDL_INIT_GAMECONTROLLER` 不需要视频子系统，也不要求窗口，后台可读；
- 断开事件带的是 instance id，要用它反查保存的句柄再删除；
- 首次编译多出 SDL 源码的 C 编译（本机约 65 秒），CI 三平台都要有 cmake；
- 转发与映射配置代码不用动，只要 `Pad` 的按钮 / 轴数组口径不变。

## 验证

- `cargo test -p mml-gamepad --lib`；
- `cargo check -p mml-gamepad -p mml-gui`（会重新生成 `bindings.ts`）；
- `cd mml-vue && npx vue-tsc --noEmit`；
- 手动：`cargo run -p mml-gamepad --example gamepad_demo` 应直接出值，且启动器在后台时也有输入。

## 参考

`target/temp/sdl-spike` 是可行性验证用的临时工程（bundled SDL2 + 枚举/轮询），可直接 `cargo run` 复现枚举结果；
换完后整个删掉。

## 此前遗留（未完成，保持原样）

- mml-gui里面的thread::spawn，除了循环任务或者可能的地方，全部改成tokio::spawn，然后完全异步
