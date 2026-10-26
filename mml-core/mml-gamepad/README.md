# 手柄输入与测试

通过 gilrs 读取设备，使用现有映射配置生成 M²L 游戏 IPC 消息。
不向操作系统注入键盘或鼠标事件。

入口为“设置 → 客户端设置 → 打开手柄配置”。保留上游的多配置、
按钮和轴绑定、修饰组合键及摇杆参数；配置保存在运行目录的 `gamepad/` 下，
保存后由同一套内核采样和转发线程生效。

配置窗口增加实时测试面板：选择设备后查看两个摇杆与按下的按钮。
测试面板只读取已有采样快照，不另开驱动，也不发送游戏输入；
已经生效的映射仍会向游戏转发输入。测试设备选择不改变实际转发设备。
转发仍按上游规则选择第一只设备，并优先选取已抓取鼠标的游戏实例。

切换目标、设备或鼠标抓取模式时，先释放旧按键。切换时仍按住的按钮
须先松开再触发，避免在菜单或另一个实例中误点。方向轴可立即恢复移动。
手柄断开时释放已发出的按键，游戏连接断开后重置转发状态。

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
自动测试不能替代驱动和实际手感验证。上游的 `gamepad_manual` 用例默认忽略，
需连接手柄后按源码内命令单独运行。

## English

Open Settings → Client settings → Open gamepad settings. Existing profiles, button and axis
bindings, modifier combinations and stick settings remain in the upstream `gamepad/` configuration.
The live test panel reads the existing device snapshots. It does not open another driver or send
input; active game mappings continue running. Selecting a test device does not change the device
used for forwarding. Forwarding retains the upstream first-device and mouse-capture target rules.

Target, device and capture-mode changes release held actions. Buttons held during a transition
must be released before they trigger again; directional axes can resume immediately.
Tests exercise the actual relay over localhost TCP, including repeat suppression, mode changes,
device loss, game reconnection, target changes and isolation between game instances.
Hardware acceptance still requires an actual controller for hotplug, directions, deadzones,
buttons and menu transitions. The upstream hardware-only manual test remains ignored by default.
