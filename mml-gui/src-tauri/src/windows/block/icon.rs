//! 方块图标：源图读取 / 裁剪区域应用 / PNG 渲染 / 皮肤绑定
//!
//! 从 `block/mod.rs` 拆出来的，裁剪与缩放那段是本窗口最重的一处 CPU 活。
//! 尺寸常量集中在这里：默认值、上下限、预览上限、源图最大像素数。
//! 命令带分组标记把组键钉回 `block`，值填原始组名，见 AGENTS.md §4。

use tauri::AppHandle;

use mml_base::hash_helper;
use mml_names::names;

use crate::dtos::IconSourceDto;
use crate::err_keys;
use crate::image_manager;

use super::super::main::{core_instance, emit_instance_change};

/// 图标边长的默认值，单位 px
///
/// 图标框是圆角方块，显示最大 84px；256 已足够清晰，需要更锐利时由前端在
/// [`ICON_SIZE_MIN`] ~ [`ICON_SIZE_MAX`] 之间另选，比如给整合包做缩略图。
pub(super) const ICON_SIZE_DEFAULT: u32 = 256;

/// 图标边长下限，再小就糊了
pub(super) const ICON_SIZE_MIN: u32 = 16;

/// 图标边长上限，太大只是浪费磁盘与解码时间
pub(super) const ICON_SIZE_MAX: u32 = 1024;

/// 裁剪预览的最长边，单位 px
///
/// 预览只用于选范围，**不参与最终裁剪**：选区坐标会按「原图 / 预览」比例换算回原图，
/// 裁的仍是原图，清晰度不受影响。降采样是为了不把十几 MB 的原图 PNG 塞进 IPC 与 `<img>`
/// —— 那正是"打开弹窗就卡死"的原因。
pub(super) const PREVIEW_MAX: u32 = 1024;

/// 解码前的像素数上限，防解压炸弹
///
/// 文件头里声明一个 5 万见方的尺寸是几 KB 的事，真去解码却要 10GB 内存。
/// 64MP 已高于常见手机照片的 12~50MP，只挡病态输入。
pub(super) const MAX_SOURCE_PIXELS: u64 = 64 * 1024 * 1024;

/// 把前端给的图标边长夹到合法范围，0 或越界都回落到默认值
///
/// - `size`: 请求的图标边长，单位 px
///
/// # 返回值
///
/// 夹在 [`ICON_SIZE_MIN`] ~ [`ICON_SIZE_MAX`] 之间的边长；`0` 回落到 [`ICON_SIZE_DEFAULT`]
pub(super) fn clamp_icon_size(size: u32) -> u32 {
    if size == 0 {
        ICON_SIZE_DEFAULT
    } else {
        size.clamp(ICON_SIZE_MIN, ICON_SIZE_MAX)
    }
}

/// 读图 → 生成预览 data URL + 原图尺寸；**阻塞**，调用方负责放到阻塞线程上跑
///
/// 只把降采样后的预览编成 PNG：原图直接编 PNG 又慢又大，12MP 照片要几秒、出来几十 MB，
/// 这一步以前跑在主线程上，界面会整片冻住。
///
/// - `path`: 本地图片路径
///
/// # 返回值
///
/// 预览 data URL，形如 `data:image/png;base64,...`，以及原图宽高；读不到 / 解不出 /
/// 超过 [`MAX_SOURCE_PIXELS`] 时返回 `Err`，内容为 i18n 键
pub(super) fn read_icon_preview(path: &str) -> Result<IconSourceDto, String> {
    let bytes = std::fs::read(path).map_err(|_| err_keys::FILE_NOT_FOUND.to_string())?;
    let format = image::guess_format(&bytes).map_err(|_| err_keys::IMAGE_INVALID.to_string())?;

    // 先只读文件头拿尺寸：解压炸弹不必真解码就能挡掉，`into_dimensions` 不读像素
    let (width, height) = image::ImageReader::with_format(std::io::Cursor::new(&bytes), format)
        .into_dimensions()
        .map_err(|_| err_keys::IMAGE_INVALID.to_string())?;
    if u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS {
        return Err(err_keys::IMAGE_INVALID.to_string());
    }

    let image = image::load_from_memory_with_format(&bytes, format)
        .map_err(|_| err_keys::IMAGE_INVALID.to_string())?;
    // thumbnail 先整数抽样再插值，比 resize 快得多；等比缩放到 PREVIEW_MAX 以内
    let preview = if width > PREVIEW_MAX || height > PREVIEW_MAX {
        image.thumbnail(PREVIEW_MAX, PREVIEW_MAX)
    } else {
        image
    };

    let mut out = std::io::Cursor::new(Vec::new());
    preview
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|err| err.to_string())?;
    Ok(IconSourceDto {
        src: format!(
            "data:image/png;base64,{}",
            hash_helper::gen_base64_bytes(out.get_ref())
        ),
        width,
        height,
    })
}

/// 读本地图片的**预览图**与原图尺寸，供"修改图标"的裁剪界面用
///
/// 与资源图标同一套编码 `data:image/png;base64,`，前端可直接放进 `<img src>`。
/// 只认 `image` crate 已启用特性的格式，即 png / jpeg / webp，解不出来的按无效图片处理。
///
/// 必须是 `async` 命令 + 阻塞线程池：同步命令会直接在**主线程**上解码 / 编码，
/// 大图会让界面完全没响应，这个坑踩过一次。
///
/// - `path`: 本地图片路径
///
/// # 返回值
///
/// 预览 data URL 与原图宽高；读不到 / 解不出时返回 `Err`，内容为 i18n 键
#[gui_macros::ipc_group("block")]
#[gui_macros::command]
pub async fn block_read_icon_source(path: String) -> Result<IconSourceDto, String> {
    tauri::async_runtime::spawn_blocking(move || read_icon_preview(&path))
        .await
        .map_err(|err| err.to_string())?
}

/// 按选区裁出图标 PNG；**阻塞**，调用方负责放到阻塞线程上跑
///
/// - `path`: 本地图片路径
/// - `x` / `y`: 选区左上角，坐标按**原图像素**算，仅 `w` / `h` 非 0 时有效
/// - `w` / `h`: 选区宽高，单位**原图像素**；任一为 `0` 表示不裁，整图等比缩放到 `size` 见方、
///   四周补透明，整图都保留
/// - `size`: 输出图标边长，单位 px，调用方已夹到合法范围
///
/// # 返回值
///
/// 图标 PNG 字节；读不到 / 解不出图片时返回 `Err`，内容为 i18n 键
pub(super) fn render_icon_png(
    path: &str,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    size: u32,
) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|_| err_keys::FILE_NOT_FOUND.to_string())?;
    let image = image::load_from_memory(&bytes).map_err(|_| err_keys::IMAGE_INVALID.to_string())?;
    let (img_w, img_h) = (image.width(), image.height());

    let icon = if w == 0 || h == 0 {
        // 不裁，即"使用原图"：等比缩放进方框，四周补透明，整图都保留
        let scaled = image.resize(size, size, image::imageops::FilterType::Lanczos3);
        let mut canvas = image::RgbaImage::new(size, size);
        let ox = (size - scaled.width()) / 2;
        let oy = (size - scaled.height()) / 2;
        image::imageops::overlay(&mut canvas, &scaled.to_rgba8(), ox as i64, oy as i64);
        image::DynamicImage::ImageRgba8(canvas)
    } else {
        // 选区按原图边界夹紧：前端已夹过，这里兜底，避免越界 panic
        let cx = x.min(img_w.saturating_sub(1));
        let cy = y.min(img_h.saturating_sub(1));
        let cw = w.min(img_w - cx);
        let ch = h.min(img_h - cy);
        // 选区：等比缩放后居中裁掉多余的边 → 正好是正方形图标
        image.crop_imm(cx, cy, cw, ch).resize_to_fill(
            size,
            size,
            image::imageops::FilterType::Lanczos3,
        )
    };

    let mut out = std::io::Cursor::new(Vec::new());
    icon.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|err| err.to_string())?;
    Ok(out.into_inner())
}

/// 按选区把本地图片裁成实例图标：写实例目录下的 `icon.png`，并把实例配置的 `Icon` 指过去
///
/// - `app`: 窗口句柄，写完广播实例变更事件
/// - `uuid`: 实例 uuid
/// - `path`: 本地图片路径
/// - `x`: 选区左上角横坐标，按**原图像素**算
/// - `y`: 选区左上角纵坐标，按**原图像素**算
/// - `w`: 选区宽度，单位**原图像素**；为 `0` 表示不裁，整图等比缩放到 `size` 见方
/// - `h`: 选区高度，单位**原图像素**；为 `0` 表示不裁，整图等比缩放到 `size` 见方
/// - `size`: 图标边长，单位 px；`0` 或越界都回落到 [`ICON_SIZE_DEFAULT`]，
///   夹在 [`ICON_SIZE_MIN`] ~ [`ICON_SIZE_MAX`] 之间
///
/// 解码 / 裁剪 / 缩放都用 Lanczos3，放阻塞线程池，不占 async 运行时；
/// 写盘前先把终图编码好，不持锁 `await`，std 锁跨 await 会破坏 `Send`。
/// `icon.png` 就是 [`InstanceSettingObj::get_icon_file`] 的默认名，
/// 这里同时把配置里的 `Icon` 显式指过去，实例目录里一眼能看出图标文件是谁。
/// 参数是 IPC 契约，见 AGENTS.md §4：`bindings.ts` 由 Rust 源码生成，不能合并成结构体。
///
/// # 返回值
///
/// 成功写入返回 `Ok(true)`；图片无效、实例不存在或写盘失败时返回 `Err`，内容为 i18n 键
#[allow(clippy::too_many_arguments)]
#[gui_macros::ipc_group("block")]
#[gui_macros::command]
pub async fn block_set_icon_area(
    app: AppHandle,
    uuid: String,
    path: String,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    size: u32,
) -> Result<bool, String> {
    let size = clamp_icon_size(size);
    let png =
        tauri::async_runtime::spawn_blocking(move || render_icon_png(&path, x, y, w, h, size))
            .await
            .map_err(|err| err.to_string())??;

    let Some(core) = core_instance(&uuid) else {
        return Err(err_keys::GAME_NOT_FOUND.to_string());
    };
    let dest = core.instance.read().unwrap().get_icon_file_or_default();
    tokio::fs::write(&dest, &png)
        .await
        .map_err(|err| err.to_string())?;

    // 上传图片 = 图标二选一里的"图片"那一支：Icon 指到 icon.png，并把方块 ID 清掉
    {
        let inst = core.instance.read().unwrap().clone();
        crate::gui_setting::clear_block(&inst);
    }
    {
        let mut inst = core.instance.write().unwrap();
        if inst.icon.as_deref() != Some(names::ICON_FILE) {
            inst.icon = Some(names::ICON_FILE.to_string());
            inst.save();
        }
    }

    image_manager::clear_instance_image(&core.uuid);
    // 专用类型：图标换 / 清了才发它，界面里只有图标订阅（普通的 edit 不再触发重拉图）
    emit_instance_change(&app, "icon");
    Ok(true)
}

/// 把方块/物品设为实例图标：**记方块 ID**，不再复制贴图
///
/// 图标与方块 ID 二选一，见 `InstanceSettingObj::get_icon_file`：这里把 `Block` 写上、
/// `Icon` 清空，之后 `mml-image://instance/<uuid>` 按 ID 现渲染。
/// 这样图标会跟随方块渲染结果更新，换材质包 / 重新渲染后自动变，
/// 而不是像以前那样定格一张 PNG。
///
/// - `app`: 窗口句柄，写完广播实例变更事件
/// - `uuid`: 实例 uuid
/// - `id`: 方块 / 物品 id，形如 `minecraft:stone`
///
/// # 返回值
///
/// 成功写入返回 `Ok(true)`；该 id 没有可用贴图或实例不存在时返回 `Err`，内容为 i18n 键
#[gui_macros::ipc_group("block")]
#[gui_macros::command]
pub async fn block_set_icon(app: AppHandle, uuid: String, id: String) -> Result<bool, String> {
    // 方块优先，未作为方块出现的 id 回退物品贴图
    let Some(_) = mml_tex_draw::get_block_path(&id).or_else(|| mml_tex_draw::get_item_path(&id))
    else {
        return Err(err_keys::FILE_NOT_FOUND.to_string());
    };

    let Some(core) = core_instance(&uuid) else {
        return Err(err_keys::GAME_NOT_FOUND.to_string());
    };

    // 写方块 ID 并清空 Icon：两步必须一起做，只写一个会让图标读取走错分支
    {
        let inst = core.instance.read().unwrap().clone();
        crate::gui_setting::set_block(&inst, id.clone());
    }
    {
        let mut inst = core.instance.write().unwrap();
        if inst.icon.is_some() {
            inst.icon = None;
            inst.save();
        }
    }

    image_manager::clear_instance_image(&core.uuid);
    emit_instance_change(&app, "icon");
    Ok(true)
}

/// 按用户名或 UUID 添加皮肤方块：拉取该玩家的皮肤并渲染成头颅图标
///
/// 同名玩家重复添加即覆盖，等于刷新图标。
///
/// - `input`: 玩家名或 uuid
///
/// # 返回值
///
/// 新皮肤方块的 id；输入解析不出玩家或皮肤下载 / 渲染失败时返回 `Err`，内容为 i18n 键
#[gui_macros::ipc_group("block")]
#[gui_macros::command]
pub async fn block_skin_add(input: String) -> Result<String, String> {
    let skin = mml_game::player_skin::fetch_skin_by_input(&input)
        .await
        .map_err(|e| e.to_string())?;

    // 锁内只取路径，文件读取在 drop 后，std 锁跨 await 破坏 Send 同理，这里没锁
    let data = tokio::fs::read(&skin.path)
        .await
        .map_err(|e| e.to_string())?;
    mml_tex_draw::add_skin_block(&skin.name, &data).map_err(|e| e.to_string())
}

/// 删除皮肤方块，名字就是皮肤方块的显示名
///
/// - `name`: 皮肤方块的显示名，即添加时用的玩家名
///
/// # 返回值
///
/// 成功删除返回 `Ok(())`；没有这个皮肤方块时返回 `Err`，内容为 i18n 键
#[gui_macros::ipc_group("block")]
#[gui_macros::command]
pub fn block_skin_remove(name: String) -> Result<(), String> {
    mml_tex_draw::remove_skin_block(&name).map_err(|e| e.to_string())
}
