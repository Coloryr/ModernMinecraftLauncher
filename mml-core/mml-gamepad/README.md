# 手柄输入与测试

通过 SDL3 读取设备，使用现有映射配置生成 M²L 游戏 IPC 消息。
不向操作系统注入键盘或鼠标事件。
SDL3 使用 bundled + static-link 从源码静态构建，需要 CMake 和 C/C++ 编译器，运行时无需额外的 SDL3 DLL。
读取线程不创建窗口，并允许后台事件；原有映射的轴方向与扳机按钮保持不变。

入口是「设置 → 手柄」；主窗口标题栏检测到手柄时也会出现手柄键，
可以直接选设备、挑配置、开关游戏内输入。保留上游的多配置、
按钮和轴绑定、修饰组合键及摇杆参数；配置保存在运行目录的 `gamepad/` 下，
保存后由同一套内核采样和转发线程生效。

配置窗口增加实时测试面板：选择设备后查看两个摇杆与按下的按钮。
测试面板只读取已有采样快照，不另开驱动，也不发送游戏输入；
已经生效的映射仍会向游戏转发输入。测试设备选择不改变实际转发设备。
转发用配置里选中的设备，没选或那台没插时回落到第一个已连接的手柄；
多开时优先选已抓取鼠标的游戏实例。

切换目标、设备或鼠标抓取模式时，先释放旧按键。切换时仍按住的按钮
须先松开再触发，避免在菜单或另一个实例中误点。方向轴可立即恢复移动。
手柄断开时释放已发出的按键，游戏连接断开后重置转发状态。

鼠标模式（未抓取鼠标，翻背包、开菜单）与游戏内模式（已抓取鼠标）可以各用一套按钮绑定。
配置项 `SeparateCursor` 默认打开，`CursorButtons` 存鼠标模式那套，缺省表示与 `Buttons` 相同 ——
老配置读进来两套内容一致，行为与升级前一样；关掉 `SeparateCursor` 后两种模式共用 `Buttons`。
轴绑定与摇杆参数两边共用，只有按钮按模式分开。

自动测试：

```sh
cargo test -p mml-gamepad --lib
```

映射与采样单测覆盖配置解析、规则范围、组合键、方向轴、滚轮和状态快照。
`tcp_relay_releases_on_mode_device_target_changes_and_reconnect` 使用真实本机 TCP
通道和可控设备快照，验证按键、位移、滚轮编码，重复抑制，切模式、设备断开、
游戏断线重连、目标切换时的按键释放与实例隔离。测试使用实际转发逻辑，
不依赖物理手柄，不模拟系统输入。

设备探测：

```sh
cargo run -p mml-gamepad --example probe
```

物理验收仍需连接手柄后检查热插拔、摇杆方向与死区、各按钮、游戏和菜单切换。
自动测试不能替代驱动和实际手感验证；接上手柄后跑 `cargo run -p mml-gamepad --example gamepad_demo`
做手感验收。

## English

Open Settings → Gamepad. A gamepad button also shows up in the main window title bar while a
controller is connected, for picking the device, the profile and the in-game switch. Existing
profiles, button and axis bindings, modifier combinations and stick settings remain in the
upstream `gamepad/` configuration.
The live test panel reads the existing device snapshots. It does not open another driver or send
input; active game mappings continue running. Selecting a test device does not change the device
used for forwarding. Forwarding uses the device picked in the profile and falls back to the first
connected one; with several games running it prefers the one that captured the mouse.

Target, device and capture-mode changes release held actions. Buttons held during a transition
must be released before they trigger again; directional axes can resume immediately.

Mouse mode (cursor free: inventories, menus) and in-game mode (cursor captured) can keep separate
button bindings. `SeparateCursor` is on by default and `CursorButtons` holds the mouse-mode set;
when it is absent the two modes share `Buttons`, so existing profiles behave exactly as before.
Axis bindings and stick settings stay shared; only buttons are split.
Tests exercise the actual relay over localhost TCP, including repeat suppression, mode changes,
device loss, game reconnection, target changes and isolation between game instances.
Hardware acceptance still requires an actual controller for hotplug, directions, deadzones,
buttons and menu transitions. The upstream hardware-only manual test remains ignored by default.
