//! 皮肤渲染：2D / 3D 全身图、原始贴图、头像渲染选型
//!
//! 从 `image_manager/mod.rs` 拆出来的。纹理一律按**内容 SHA1** 取，与游戏 assets objects
//! 同款布局，渲染缓存键也用 sha1，所以同一份贴图的渲染结果跨账户共享。

use mml_auth::auths;
use mml_game::launcher_path::assets_path;
use mml_game::player_skin::{self, TextureFile};
use mml_net::mojang_api;
use mml_skin::{SkinType, skin_type_checker};
use mml_skin_draw::{
    Draw2DType, Draw3DType, DrawHeadType, DrawSkinType, draw_head, draw_skin,
};
use mml_sys::path_helper;
use tauri::UriSchemeResponder;
use tiny_skia::{Pixmap, Point};

use crate::dtos::account_dto::auth_type_from_str;
use crate::gui_config::{self, HeadType, SkinConfig, SkinDisplay};

use super::head::send_skin_preview;
use super::push_texture_url;

use super::respond::{send_bad, send_png};
use super::{PairImageKey, SKINRAW_IMAGE, SKIN_IMAGE, TEXTURE_URLS};

/// 皮肤类 URI 的账户型请求，解析出账户并取回皮肤 / 披风纹理
///
/// URI 形如 `skin*/cape*/<账户类型>/<uuid>`。账户不存在返回 None；皮肤 / 披风某一项
/// 可能没有，离线账户两个都没有。下载结果由 player_skin 层按内容 SHA1 缓存到本地
/// 皮肤目录，同时把源 URL 登记进 [`TEXTURE_URLS`]，sha1 型请求在文件被清后能回源。
///
/// - `uri`: 协议路径片段，第 2 段是账户类型，第 3 段是账户 uuid
///
/// # 返回值
///
/// 账户不存在返回 `None`
pub(super) async fn resolve_skin_files(uri: &[&str]) -> Option<SkinFiles> {
    let user_type = auth_type_from_str(uri.get(1)?);
    let user = auths::get(uri.get(2)?, user_type)?;
    let res = player_skin::download_skin(&user).await;
    if let Some(tex) = &res.skin {
        push_texture_url(&tex.sha1, &tex.url);
    }
    if let Some(tex) = &res.cape {
        push_texture_url(&tex.sha1, &tex.url);
    }
    Some(SkinFiles {
        is_new_slim: res.is_new_slim,
        skin: res.skin,
        cape: res.cape,
    })
}

/// 判断片段是否为内容 SHA1，即 40 位十六进制
///
/// sha1 型与账户型 URI 的区分依据：账户类型关键字与账户 uuid 都不是 40 位十六进制
///
/// - `s`: 待判定的 URI 片段
///
/// # 返回值
///
/// 是内容 SHA1 返回 `true`
pub(super) fn is_sha1(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// 按内容 sha1 取纹理 PNG 字节
///
/// 本地皮肤目录有文件直接读；没有则按 [`TEXTURE_URLS`] 登记的源 URL 重新下载，
/// 下载结果仍按内容 sha1 落盘，与请求的 sha1 一致才返回
///
/// - `sha1`: 纹理内容 SHA1
///
/// # 返回值
///
/// 本地没有且回源失败返回 `None`
pub(super) async fn load_texture_bytes(sha1: &str) -> Option<Vec<u8>> {
    let file = assets_path::get_skin_object(sha1);
    if file.is_file() {
        return path_helper::read_byte(&file).ok();
    }

    let url = TEXTURE_URLS.read().unwrap().get(sha1).cloned()?;
    let data = mojang_api::get_assets(&url).await.ok()?;
    let saved = assets_path::save_skin_object(&data).ok()?;
    (saved == sha1).then_some(data)
}

/// 皮肤全身渲染图，URI 形如 `skin/...`
///
/// 片段形态，首段 `skin` 已剥：
/// - `skin/<sha1>`、`skin/<sha1>/<skin_type>`：sha1 型，40 位十六进制，
///   直接按内容取皮肤——皮肤窗口等已知具体贴图的场景用
/// - `skin/<账户类型>/<uuid>[/<skin_type>]`：账户型，解析该账户**当前选中**的皮肤
///   后转 sha1 型渲染——账户列表预览用
/// - `skin/preview/<uuid>/<skin_type>`：账户型的特例，设置页样例，先判 preview
///
/// 渲染风格由后端读 gui_config 的 `skin` 决定，取值 Skin2DA / Skin2DB /
/// Skin3D / Skin3DD / Skin3DRotate，前端不选风格。皮肤型号段取值 `auto` 自动检测且可省略、
/// `old` 1.7 旧版、`new` 1.8 新版、`slim` 纤细——这是皮肤版型信息，不是渲染风格
///
/// - `uri`: 协议路径片段，首段 `skin` 已剥
/// - `res`: 响应回写句柄
pub(super) async fn load_skin_image(uri: &[&str], res: UriSchemeResponder) {
    // sha1 型：2-3 段且第 2 段是 40 位十六进制
    if (uri.len() == 2 || uri.len() == 3) && is_sha1(uri[1]) {
        let skin_type = uri.get(2).copied().unwrap_or("auto");
        if parse_skin_type(skin_type).is_none() {
            send_bad(res);
            return;
        }
        send_skin_display(uri[1], skin_type, None, res).await;
        return;
    }

    if uri.len() != 3 && uri.len() != 4 {
        send_bad(res);
        return;
    }
    let skin_type = uri.get(3).copied().unwrap_or("auto");
    if parse_skin_type(skin_type).is_none() {
        send_bad(res);
        return;
    }

    // 设置页样例预览 `skin/preview/<uuid>/<skin_type>`：内置样例皮肤按当前配置渲染
    if uri[1] == "preview" {
        send_skin_preview(skin_type, res);
        return;
    }

    let Some(files) = resolve_skin_files(uri).await else {
        send_bad(res);
        return;
    };
    let Some(tex) = files.skin else {
        send_bad(res);
        return;
    };
    send_skin_display(&tex.sha1, skin_type, Some(files.is_new_slim), res).await;
}

/// 皮肤全身渲染，sha1 入口；账户型请求解析出当前皮肤后也转调这里
///
/// `server_slim` 为账户型解析得到的会话服务器纤细标记，是权威值，用于自动型号纠偏；
/// sha1 型请求传 `None`，自动型号仅按贴图检测
///
/// - `sha1`: 皮肤贴图内容 SHA1
/// - `skin_type`: 皮肤型号段，取值 `auto` / `old` / `new` / `slim`
/// - `server_slim`: 会话服务器的纤细标记，`None` 表示没有权威值
/// - `res`: 响应回写句柄
pub(super) async fn send_skin_display(
    sha1: &str,
    skin_type: &str,
    server_slim: Option<bool>,
    res: UriSchemeResponder,
) {
    let skin = gui_config::get().skin;
    // 旋转档的角度参与渲染，一并进缓存键，改角度就是新键
    let key = PairImageKey {
        sha1: sha1.to_string(),
        config: format!(
            "{:?}/{}/{}/{skin_type}",
            skin.display, skin.pitch, skin.yaw
        ),
    };

    {
        let image = SKIN_IMAGE.read().unwrap().get(&key).cloned();
        if let Some(data) = image {
            send_png(res, data);
            return;
        }
    }

    let Some(bytes) = load_texture_bytes(sha1).await else {
        send_bad(res);
        return;
    };
    let Ok(bitmap) = Pixmap::decode_png(&bytes) else {
        send_bad(res);
        return;
    };
    let st = match parse_skin_type(skin_type).flatten() {
        Some(st) => Some(st),
        None => {
            // 自动检测：带服务器纤细标记时用于纠偏——服务器说是纤细就直接按纤细；
            // 说不是而贴图检测出纤细，多半是手臂宽度含糊导致误判，按普通新版处理
            let detected = skin_type_checker::get_skin_type(&bitmap);
            Some(match server_slim {
                Some(true) => SkinType::NewSlim,
                Some(false) if detected == SkinType::NewSlim => SkinType::New,
                _ => detected,
            })
        }
    };
    let Some(image) = render_skin_display(&bitmap, st, &skin) else {
        send_bad(res);
        return;
    };
    let Ok(data) = image.encode_png() else {
        send_bad(res);
        return;
    };

    SKIN_IMAGE.write().unwrap().insert(key, data.clone());

    send_png(res, data);
}

/// 按皮肤显示模式选渲染方式：2D 展开 / 2D 大图 / 3D 等距 / 3D 旋转
///
/// - `bitmap`: 皮肤贴图
/// - `st`: 皮肤型号，`None` 表示不指定
/// - `skin`: 皮肤显示设置，含显示模式与 3D 旋转角度
///
/// # 返回值
///
/// 渲染失败返回 `None`
pub(super) fn render_skin_display(
    bitmap: &Pixmap,
    st: Option<SkinType>,
    skin: &SkinConfig,
) -> Option<Pixmap> {
    match skin.display {
        SkinDisplay::Skin2DA => draw_skin(bitmap, DrawSkinType::Type2D(Draw2DType::TypeA), st),
        SkinDisplay::Skin2DB => draw_skin(bitmap, DrawSkinType::Type2D(Draw2DType::TypeB), st),
        SkinDisplay::Skin3D => draw_skin(bitmap, DrawSkinType::Type3D(Draw3DType::TypeAUp), st),
        SkinDisplay::Skin3DD => draw_skin(bitmap, DrawSkinType::Type3D(Draw3DType::TypeADown), st),
        SkinDisplay::Skin3DRotate => draw_skin(
            bitmap,
            DrawSkinType::Type3D(Draw3DType::TypeBRotate(Point::from_xy(skin.pitch, skin.yaw))),
            st,
        ),
    }
}

/// URI 里的皮肤类型段转成 [`SkinType`]
///
/// - `s`: 皮肤型号段
///
/// # 返回值
///
/// `auto` 转成 `Some(None)`，表示交给自动检测；段非法返回 `None`
pub(super) fn parse_skin_type(s: &str) -> Option<Option<SkinType>> {
    match s {
        "auto" => Some(None),
        "old" => Some(Some(SkinType::Old)),
        "new" => Some(Some(SkinType::New)),
        "slim" => Some(Some(SkinType::NewSlim)),
        _ => None,
    }
}

/// 原始皮肤贴图 PNG，URI 形如 `skinraw/<sha1>`，供前端 skinview3d 做 3D 渲染
///
/// - `uri`: 协议路径片段，首段 `skinraw` 已剥
/// - `res`: 响应回写句柄
pub(super) async fn load_skinraw_image(uri: &[&str], res: UriSchemeResponder) {
    if uri.len() != 2 {
        send_bad(res);
        return;
    }

    {
        let image = SKINRAW_IMAGE.read().unwrap().get(uri[1]).cloned();
        if let Some(data) = image {
            send_png(res, data);
            return;
        }
    }

    let Some(data) = load_texture_bytes(uri[1]).await else {
        send_bad(res);
        return;
    };

    SKINRAW_IMAGE
        .write()
        .unwrap()
        .insert(uri[1].to_string(), data.clone());

    send_png(res, data);
}

/// 按配置的头像类型渲染头像
///
/// - `bitmap`: 皮肤贴图
///
/// # 返回值
///
/// 渲染失败返回 `None`
pub(super) fn gen_head_image(bitmap: &Pixmap) -> Option<Pixmap> {
    let config = gui_config::get().head;

    match config.head_type {
        HeadType::Head3DA => draw_head(bitmap, DrawHeadType::Type3D(Draw3DType::TypeADown)),
        HeadType::Head3DB => draw_head(
            bitmap,
            DrawHeadType::Type3D(Draw3DType::TypeBRotate(Point::from_xy(config.x, config.y))),
        ),
        HeadType::Head3DC => draw_head(bitmap, DrawHeadType::Type3D(Draw3DType::TypeAUp)),
        HeadType::Head2DB => draw_head(bitmap, DrawHeadType::Type2D(Draw2DType::TypeB)),
        HeadType::Head2DA => draw_head(bitmap, DrawHeadType::Type2D(Draw2DType::TypeA)),
    }
}

/// 账户皮肤与披风的本地缓存纹理
pub(super) struct SkinFiles {
    /// 当前选中的皮肤；字段给同级的 head / cape 模块读，故 pub(super)
    pub(super) skin: Option<TextureFile>,
    /// 当前选中的披风；字段给同级的 cape 模块读，故 pub(super)
    pub(super) cape: Option<TextureFile>,
    /// 会话服务器 metadata 的纤细标记，是权威值；离线等查不到档案时为 false
    pub(super) is_new_slim: bool,
}
