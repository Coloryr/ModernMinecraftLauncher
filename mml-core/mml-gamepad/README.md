# 手柄输入与测试

通过 gilrs 采样设备，将标准手柄输入转换为 M²L 游戏 IPC 消息。
不向操作系统注入键盘或鼠标事件。

启动器入口为“设置 → 客户端设置 → 手柄”。先选设备与目标实例、保存，
再启用游戏控制。目标游戏需由启动器启动并连接输入通道。
设备测试面板不依赖启用控制，可查看两个摇杆与按钮状态。
设备列表支持热插拔；同型号设备可能报告相同 GUID，当前配置按 GUID 选择。

- 游戏抓取鼠标时：左摇杆 WASD，右摇杆视角，A 跳跃，B 潜行，X 背包，Y 副手。
- RT 攻击，LT 使用，L3 冲刺，R3 F5，Select 丢弃，Start ESC。
- 菜单中：左摇杆光标，A 左键，X 右键，B / Start ESC。
- 肩键、十字上下：边沿触发滚轮，按住不会连发。
- 禁用、换目标、断开设备、退出线程时释放原通道按键。
- 模式切换时，按住的按钮须先松开再使用，避免跳跃键误点菜单。

自动测试：

```sh
cargo test -p mml-gamepad --test mapping --test input_bridge
```

`mapping` 覆盖死区、重复抑制、切模式释放、共享 ESC、速度与帧停顿限制。
`input_bridge` 建立两个真实本机 TCP 游戏连接，检查映射后消息的编码、
接收实例隔离、抓取状态切换和释放消息，不需要物理手柄。
还会在按住方向与扳机时断开连接，验证断线发送失败、映射重置与同一实例重连；
重连后恢复方向输入，扳机松开再按才发送攻击，全程检查另一实例没有收到输入。

设备探测：

```sh
cargo run -p mml-gamepad --example probe
```

物理设备验收仍需连接手柄后检查热插拔、摇杆方向与死区、各按钮、
游戏和菜单切换、禁用控制后的按键释放。自动测试不能替代驱动和实际手感验证。

## English

Open Settings → Client settings → Gamepad. Choose a device and a target instance, save the
settings, then enable control. The game must connect to the launcher's input channel.
The live device panel works while control is disabled. Device selection uses the GUID;
identical models may share a GUID.

The tests cover pure mappings and real localhost TCP delivery to the selected game connection.
The bridge test also disconnects while movement and attack are held, resets the mapper and
reconnects the same instance. It verifies failed delivery while disconnected, restored movement,
release before attack can trigger again, and continued isolation from the other instance.
They do not inject system input or require a physical controller. Hardware acceptance still
requires checking hotplug, stick directions, deadzones, buttons, menu transitions and key release
with an actual controller.
