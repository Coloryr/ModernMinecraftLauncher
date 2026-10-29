//! 子窗口摆放
//!
//! 按父窗口的可用矩形算落点：居中、贴角或按锚点偏移，
//! 越界的落点先收进屏幕再交给建窗。

/// 子窗口贴角时与锚点窗口边缘留的间距，单位逻辑像素
pub(super) const CHILD_GAP: f64 = 32.0;

/// 逻辑像素的宽高；摆放计算全用逻辑单位
pub(crate) struct SizeF {
    /// 宽
    pub(crate) width: f64,
    /// 高
    pub(crate) height: f64,
}

/// 逻辑像素的矩形，工作区与锚点窗口外框都用它
pub(crate) struct RectF {
    /// 左边缘
    pub(crate) x: f64,
    /// 上边缘
    pub(crate) y: f64,
    /// 宽
    pub(crate) width: f64,
    /// 高
    pub(crate) height: f64,
}

/// 算出的窗口落点，逻辑像素
pub(crate) struct Placement {
    /// 左边缘
    pub(crate) x: f64,
    /// 上边缘
    pub(crate) y: f64,
}

/// 锚点窗口的四个角，按这个顺序试
#[derive(Clone, Copy)]
pub(super) enum Corner {
    /// 左上
    LeftTop,
    /// 左下
    LeftBottom,
    /// 右上
    RightTop,
    /// 右下
    RightBottom,
}

/// 子窗口没有历史几何时的落点：叠在锚点窗口某个角的**内侧**
///
/// 依次试左上 / 左下 / 右上 / 右下，取第一个整窗都落在工作区内的；四个都放不下就用左上
/// 夹进工作区 —— 宁可贴边也不能把窗口开到屏幕外。间距见 `CHILD_GAP`，
/// 子窗口比锚点窗口大时会从那个角探出去。
///
/// - `anchor`: 锚点窗口的外框，逻辑像素
/// - `child`: 子窗口尺寸，逻辑像素
/// - `area`: 锚点所在显示器的工作区，逻辑像素，已排掉任务栏
///
/// # 返回值
///
/// 见 [`Placement`]
pub(crate) fn child_placement(anchor: RectF, child: SizeF, area: RectF) -> Placement {
    const ORDER: [Corner; 4] = [
        Corner::LeftTop,
        Corner::LeftBottom,
        Corner::RightTop,
        Corner::RightBottom,
    ];

    let mut fallback = None;
    for corner in ORDER {
        let placement = corner_placement(corner, &anchor, &child);
        if fits(&placement, &child, &area) {
            return placement;
        }
        fallback.get_or_insert(placement);
    }

    let placement = fallback.unwrap_or(Placement {
        x: area.x,
        y: area.y,
    });
    // 子窗口比工作区还大时 min > max，clamp 会 panic，所以下限反过来兜一层
    Placement {
        x: placement
            .x
            .clamp(area.x, (area.x + area.width - child.width).max(area.x)),
        y: placement
            .y
            .clamp(area.y, (area.y + area.height - child.height).max(area.y)),
    }
}

/// 屏幕中心：主窗口没有历史几何时的落点
///
/// - `area`: 目标显示器的工作区，逻辑像素
/// - `child`: 窗口尺寸，逻辑像素
///
/// # 返回值
///
/// 见 [`Placement`]
pub(crate) fn center_placement(area: RectF, child: SizeF) -> Placement {
    Placement {
        x: area.x + (area.width - child.width) / 2.0,
        y: area.y + (area.height - child.height) / 2.0,
    }
}

/// 某个角的内侧落点
pub(super) fn corner_placement(corner: Corner, anchor: &RectF, child: &SizeF) -> Placement {
    let left = anchor.x + CHILD_GAP;
    let right = anchor.x + anchor.width - child.width - CHILD_GAP;
    let top = anchor.y + CHILD_GAP;
    let bottom = anchor.y + anchor.height - child.height - CHILD_GAP;

    match corner {
        Corner::LeftTop => Placement { x: left, y: top },
        Corner::LeftBottom => Placement { x: left, y: bottom },
        Corner::RightTop => Placement { x: right, y: top },
        Corner::RightBottom => Placement {
            x: right,
            y: bottom,
        },
    }
}

/// 整窗是否落在工作区内
pub(super) fn fits(placement: &Placement, child: &SizeF, area: &RectF) -> bool {
    placement.x >= area.x
        && placement.y >= area.y
        && placement.x + child.width <= area.x + area.width
        && placement.y + child.height <= area.y + area.height
}

#[cfg(test)]
mod placement_tests {
    use super::*;

    /// 一个工作区矩形
    fn area() -> RectF {
        RectF {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    /// 主窗口居中：工作区与窗口都算一半
    #[test]
    fn center_uses_work_area() {
        let p = center_placement(
            area(),
            SizeF {
                width: 400.0,
                height: 300.0,
            },
        );
        assert_eq!((p.x, p.y), (760.0, 390.0));
    }

    /// 锚点在屏幕中间时走左上，间距是 CHILD_GAP
    #[test]
    fn child_prefers_left_top() {
        let anchor = RectF {
            x: 600.0,
            y: 300.0,
            width: 800.0,
            height: 400.0,
        };
        let p = child_placement(
            anchor,
            SizeF {
                width: 400.0,
                height: 300.0,
            },
            area(),
        );
        assert_eq!((p.x, p.y), (600.0 + CHILD_GAP, 300.0 + CHILD_GAP));
    }

    /// 左上会探出工作区时依次退：这个例子里只有右下放得下
    #[test]
    fn child_falls_back_to_a_corner_that_fits() {
        let anchor = RectF {
            x: 1500.0,
            y: 800.0,
            width: 400.0,
            height: 250.0,
        };
        let child = SizeF {
            width: 400.0,
            height: 300.0,
        };

        let p = child_placement(anchor, child, area());
        // 右下：右边 = 锚点右边 − 子窗口宽 − 间距，下边同理（这里下边会被夹到工作区里）
        assert!(p.x + 400.0 <= 1920.0, "右边不能探出工作区");
        assert!(p.y + 300.0 <= 1080.0, "下边不能探出工作区");
        assert_eq!(p.x, 1500.0 + 400.0 - 400.0 - CHILD_GAP);
    }

    /// 工作区比窗口还小时夹进工作区，不 panic、也不开到屏幕外
    #[test]
    fn child_clamps_into_work_area() {
        let small = RectF {
            x: 100.0,
            y: 100.0,
            width: 800.0,
            height: 600.0,
        };
        let p = child_placement(
            RectF {
                x: 200.0,
                y: 200.0,
                width: 300.0,
                height: 200.0,
            },
            SizeF {
                width: 1000.0,
                height: 800.0,
            },
            small,
        );
        assert_eq!((p.x, p.y), (100.0, 100.0));
    }
}
