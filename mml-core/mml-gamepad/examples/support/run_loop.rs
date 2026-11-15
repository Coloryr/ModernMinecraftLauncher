//! 命令行示例的等待与 run loop
//!
//! macOS 的 GameController 靠进程的 run loop 才会返回已连接手柄。命令行示例没有事件循环，
//! 枚举与采集期间要手动跑一小段；其它平台就是普通等待。

/// 在当前线程停留，macOS 上同时驱动 run loop
///
/// - `seconds`: 停留时长，单位秒
#[cfg(target_os = "macos")]
pub fn run(seconds: f64) {
    use std::ffi::c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFRunLoopDefaultMode: *const c_void;
        fn CFRunLoopRunInMode(
            mode: *const c_void,
            seconds: f64,
            return_after_source_handled: u8,
        ) -> i32;
    }

    // 只读常量与纯 FFI 调用，没有跨线程共享状态
    unsafe {
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, seconds, 0);
    }
}

/// 在当前线程停留
///
/// - `seconds`: 停留时长，单位秒
#[cfg(not(target_os = "macos"))]
pub fn run(seconds: f64) {
    std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
}
