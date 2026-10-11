//! 皮肤部件动画模块
//!
//! 以 120 帧为一个循环演算行走动画，每帧 0.01 秒，
//! 输出手臂、腿部、头部、披风的旋转角度。

use glam::Vec3;
use mml_skin::SkinType;

/// 皮肤的动画
#[derive(Debug, Clone)]
pub struct SkinAnimation {
    /// 当前动画帧，取值 0–119
    frame: i32,
    /// 不足一帧的剩余时间累计，单位秒
    count: f64,
    /// 是否已被关闭，关闭后 `tick` 返回 `false`
    close: bool,
    /// 是否正在播放
    pub run: bool,
    /// 皮肤类型，影响头部摆动走 x 轴还是 z 轴
    pub skin_type: SkinType,
    /// 手臂旋转角度，单位度，x / z 为侧摆、y 为前后摆
    pub arm: Vec3,
    /// 腿部旋转角度，单位度
    pub leg: Vec3,
    /// 头部旋转角度，单位度
    pub head: Vec3,
    /// 披风摆动角度，单位度
    pub cape: f32,
}

impl SkinAnimation {
    /// 创建动画，初始手臂抬起 40 度
    pub fn new() -> Self {
        Self {
            frame: 0,
            count: 0.0,
            close: false,
            run: false,
            skin_type: SkinType::Unknown,
            arm: Vec3::new(40.0, 0.0, 0.0),
            leg: Vec3::new(0.0, 0.0, 0.0),
            head: Vec3::new(0.0, 0.0, 0.0),
            cape: 0.0,
        }
    }

    /// 关闭动画
    pub fn close(&mut self) {
        self.run = false;
        self.close = true;
    }

    /// 进行动画演算
    ///
    /// - `time`: 本次推进的时间，单位秒，每累计 0.01 秒推进一帧
    ///
    /// # 返回值
    ///
    /// 动画未关闭返回 `true`，已关闭返回 `false`
    pub fn tick(&mut self, time: f64) -> bool {
        if self.run {
            self.count += time;
            while self.count > 0.01 {
                self.count -= 0.01;
                self.frame += 1;
            }

            if self.frame >= 120 {
                self.frame = 0;
            }

            if self.frame <= 60 {
                // 0 360
                // -180 180
                self.arm.y = self.frame as f32 * 6.0 - 180.0;

                // 0 180
                // 90 -90
                self.leg.y = 90.0 - self.frame as f32 * 3.0;

                // 0 6
                self.cape = self.frame as f32 / 10.0;

                // -30 30
                if self.skin_type == SkinType::NewSlim {
                    self.head.z = 0.0;
                    self.head.x = self.frame as f32 - 30.0;
                } else {
                    self.head.x = 0.0;
                    self.head.z = self.frame as f32 - 30.0;
                }
            } else {
                // 61 120
                // 6 0
                self.cape = 6.0 - (self.frame as f32 - 60.0) / 10.0;

                // 360 720
                // 180 -180
                self.arm.y = 540.0 - self.frame as f32 * 6.0;

                // 180 360
                // -90 90
                self.leg.y = self.frame as f32 * 3.0 - 270.0;

                // 30 -30
                if self.skin_type == SkinType::NewSlim {
                    self.head.z = 0.0;
                    self.head.x = 90.0 - self.frame as f32;
                } else {
                    self.head.x = 0.0;
                    self.head.z = 90.0 - self.frame as f32;
                }
            }
        }

        !self.close
    }

    /// 重置动画状态
    pub fn reset(&mut self) {
        self.frame = 0;
        self.count = 0.0;
        self.close = false;
        self.arm = Vec3::new(40.0, 0.0, 0.0);
        self.leg = Vec3::new(0.0, 0.0, 0.0);
        self.head = Vec3::new(0.0, 0.0, 0.0);
        self.cape = 0.0;
    }

    /// 获取当前动画帧，取值 0–119
    pub fn get_frame(&self) -> i32 {
        self.frame
    }

    /// 设置动画帧，对 120 取模
    pub fn set_frame(&mut self, frame: i32) {
        self.frame = frame % 120;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 初始状态：frame 为 0，手臂默认抬起 40 度
    #[test]
    fn test_new_animation_defaults() {
        let anim = SkinAnimation::new();
        assert_eq!(anim.get_frame(), 0);
        assert!(!anim.run);
        assert_eq!(anim.arm, Vec3::new(40.0, 0.0, 0.0));
        assert_eq!(anim.leg, Vec3::ZERO);
        assert_eq!(anim.head, Vec3::ZERO);
        assert_eq!(anim.cape, 0.0);
    }

    /// run 为 false 时 tick 不推进帧，但返回 true 表示未关闭
    #[test]
    fn test_tick_without_run() {
        let mut anim = SkinAnimation::new();
        assert!(anim.tick(1.0));
        assert_eq!(anim.get_frame(), 0);
        // 关闭后 tick 返回 false
        anim.close();
        assert!(!anim.run);
        assert!(!anim.tick(0.01));
    }

    /// set_frame 对 120 帧取模
    #[test]
    fn test_set_frame_modulo() {
        let mut anim = SkinAnimation::new();
        anim.set_frame(130);
        assert_eq!(anim.get_frame(), 10);
        anim.set_frame(120);
        assert_eq!(anim.get_frame(), 0);
    }

    /// 正常播放：每累计 0.01 秒推进一帧，并按帧号更新各部件角度
    #[test]
    fn test_tick_advances_frames_and_values() {
        let mut anim = SkinAnimation::new();
        anim.run = true;
        anim.set_frame(30);

        // 0.02 秒 -> 推进 1 帧 -> frame 31
        anim.tick(0.02);
        assert_eq!(anim.get_frame(), 31);

        // frame <= 60 分支的角度公式
        assert_eq!(anim.arm.x, 40.0, "arm.x 不受前半段影响");
        assert_eq!(anim.arm.y, 31.0 * 6.0 - 180.0);
        assert_eq!(anim.leg.y, 90.0 - 31.0 * 3.0);
        assert_eq!(anim.cape, 31.0 / 10.0);

        // 非纤细：head.x 为 0，head.z 摆动
        assert_eq!(anim.head.x, 0.0);
        assert_eq!(anim.head.z, 31.0 - 30.0);
    }

    /// 纤细皮肤 NewSlim 的动画走 head.x 摆动，head.z 固定为 0
    #[test]
    fn test_tick_slim_head_axis() {
        let mut anim = SkinAnimation::new();
        anim.run = true;
        anim.skin_type = SkinType::NewSlim;
        anim.set_frame(30);
        anim.tick(0.02);
        assert_eq!(anim.head.z, 0.0);
        assert_eq!(anim.head.x, 31.0 - 30.0);
    }

    /// 帧到达 120 后应回绕到 0
    #[test]
    fn test_frame_wraps_at_120() {
        let mut anim = SkinAnimation::new();
        anim.run = true;
        anim.set_frame(119);
        // 0.03 秒 -> 推进 3 帧 -> 122 -> 回绕为 0
        anim.tick(0.03);
        assert_eq!(anim.get_frame(), 0);

        // 后半段 frame > 60 的角度公式
        // 注意：set_frame 不清空内部的时间累计，第一次 tick(0.03) 后剩余 count=0.01，
        // 再 tick(0.02) 会累计出 3 帧推进：61 -> 63
        anim.set_frame(61);
        anim.tick(0.02); // -> 63
        assert_eq!(anim.get_frame(), 63);
        assert_eq!(anim.arm.y, 540.0 - 63.0 * 6.0);
        assert_eq!(anim.leg.y, 63.0 * 3.0 - 270.0);
        assert_eq!(anim.cape, 6.0 - (63.0 - 60.0) / 10.0);
    }

    /// reset 应恢复初始状态并解除关闭标记
    #[test]
    fn test_reset() {
        let mut anim = SkinAnimation::new();
        anim.run = true;
        anim.set_frame(100);
        anim.tick(0.05);
        anim.close();
        assert!(!anim.tick(0.01));

        anim.reset();
        assert_eq!(anim.get_frame(), 0);
        assert_eq!(anim.arm, Vec3::new(40.0, 0.0, 0.0));
        // reset 后重新可播放
        anim.run = true;
        assert!(anim.tick(0.01));
    }
}
