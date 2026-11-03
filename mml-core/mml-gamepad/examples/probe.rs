//! 只枚举 SDL 控制器，不创建窗口、不启动映射线程或发送游戏输入。

fn main() -> Result<(), String> {
    let sdl = sdl2::init()?;
    let subsystem = sdl.game_controller()?;
    for index in 0..subsystem.num_joysticks()? {
        if !subsystem.is_game_controller(index) {
            continue;
        }
        let controller = subsystem.open(index).map_err(|error| error.to_string())?;
        println!("[{}] {}", controller.instance_id(), controller.name());
    }
    Ok(())
}
