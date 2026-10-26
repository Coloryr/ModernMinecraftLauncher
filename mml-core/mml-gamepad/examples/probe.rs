//! 只枚举设备，不启动映射线程或发送游戏输入。

fn main() {
    match gilrs::Gilrs::new() {
        Ok(gilrs) => {
            let devices: Vec<_> = gilrs
                .gamepads()
                .map(|(_, device)| device.name().to_string())
                .collect();
            println!("{devices:?}");
        }
        Err(err) => eprintln!("{err}"),
    }
}
