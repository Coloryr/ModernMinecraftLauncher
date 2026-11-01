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

