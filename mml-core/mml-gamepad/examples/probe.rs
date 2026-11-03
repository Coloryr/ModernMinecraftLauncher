//! 只枚举 SDL 手柄，不创建窗口、不启动映射线程或发送游戏输入。

fn main() -> Result<(), String> {
    let sdl = sdl3::init().map_err(|error| error.to_string())?;
    let subsystem = sdl.gamepad().map_err(|error| error.to_string())?;
    for id in subsystem.gamepads().map_err(|error| error.to_string())? {
        let name = subsystem
            .name_for_id(id)
            .unwrap_or_else(|_| String::from("?"));
        let vendor = subsystem.vendor_for_id(id).unwrap_or_default();
        let product = subsystem.product_for_id(id).unwrap_or_default();
        println!("[{id:?}] {name} vendor={vendor:#06x} product={product:#06x}");
    }
    Ok(())
}
