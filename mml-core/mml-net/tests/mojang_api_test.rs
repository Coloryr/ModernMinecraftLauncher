//! Mojang API 的离线测试。
//!
//! 覆盖各数据结构对真实 Mojang / minecraft.net API 响应结构的反序列化。
//! 含一个标记 `#[ignore]` 的真实网络测试，即版本清单。
//!
//! 运行网络测试：
//! `cargo test -p mml-net --test mojang_api_test -- --ignored`

use mml_net::mojang_api::{
    ArticleGridObj, DefaultTileObj, ImageObj, MinecraftNewsObj, MinecraftProfileObj,
    MinecraftTokenResObj, UserProfileObj, get_versions,
};

/// Minecraft Profile 接口 `api.minecraftservices.com/minecraft/profile` 的响应
const PROFILE_JSON: &str = r#"{
    "id": "069a79f444e94726a5befca90e38aaf5",
    "name": "Notch",
    "skins": [
        {
            "id": "6a6e65e5-76dd-4c3c-a625-162924514568",
            "state": "ACTIVE",
            "url": "https://textures.minecraft.net/texture/bd8e6a3daf2c36d582d3d75ba43d819e2a196686e6304773b8dbc9a3c97f9",
            "variant": "classic"
        }
    ],
    "capes": []
}"#;

/// 会话服务器档案接口 `sessionserver.mojang.com/session/minecraft/profile/{uuid}` 的响应
const USER_PROFILE_JSON: &str = r#"{
    "id": "069a79f444e94726a5befca90e38aaf5",
    "name": "Notch",
    "properties": [
        {
            "name": "textures",
            "value": "eyJ0aW1lc3RhbXAiOjE2MDAwMDAwMDAwMDB9"
        }
    ]
}"#;

/// Xbox 登录换取 Minecraft Token，即 `login_with_xbox` 的响应
const TOKEN_RES_JSON: &str = r#"{
    "access_token": "eyJhbGciOiJIUzI1NiJ9",
    "token_type": "Bearer",
    "expires_in": 86400
}"#;

/// Minecraft 官方新闻接口的响应，AEM 输出，snake_case 与 imageURL 混合命名
const NEWS_JSON: &str = r#"{
    "article_grid": [
        {
            "default_tile": {
                "title": "Java Edition 1.21",
                "sub_header": "Tricky Trials is here",
                "tile_size": "small",
                "image": {
                    "content_type": "image/png",
                    "imageURL": "https://www.minecraft.net/content/dam/games/minecraft/1_21.png",
                    "alt": "Tricky Trials artwork"
                }
            },
            "primary_category": "news",
            "article_url": "/article/tricky-trials"
        }
    ]
}"#;

/// 玩家档案反序列化
#[test]
fn deserialize_minecraft_profile() {
    let obj: MinecraftProfileObj = serde_json::from_str(PROFILE_JSON).unwrap();

    assert_eq!(obj.id, "069a79f444e94726a5befca90e38aaf5");
    assert_eq!(obj.name, "Notch");
}

/// 会话服务器档案反序列化
#[test]
fn deserialize_user_profile() {
    let obj: UserProfileObj = serde_json::from_str(USER_PROFILE_JSON).unwrap();

    assert_eq!(obj.properties.len(), 1);
    assert_eq!(
        obj.properties[0].value,
        "eyJ0aW1lc3RhbXAiOjE2MDAwMDAwMDAwMDB9"
    );
}

/// Token 响应反序列化，字段与 get_minecraft_token 的校验逻辑对应
#[test]
fn deserialize_token_res() {
    let obj: MinecraftTokenResObj = serde_json::from_str(TOKEN_RES_JSON).unwrap();

    assert_eq!(obj.access_token, "eyJhbGciOiJIUzI1NiJ9");
    assert!(obj.expires_in > 0);
}

/// 新闻响应反序列化，嵌套结构逐层校验
#[test]
fn deserialize_news() {
    let obj: MinecraftNewsObj = serde_json::from_str(NEWS_JSON).unwrap();

    assert_eq!(obj.article_grid.len(), 1);
    let tile: &DefaultTileObj = &obj.article_grid[0].default_tile;
    assert_eq!(tile.title, "Java Edition 1.21");
    assert_eq!(tile.sub_header, "Tricky Trials is here");
    assert_eq!(tile.tile_size, "small");
    let image: &ImageObj = &tile.image;
    assert_eq!(image.content_type, "image/png");
    assert_eq!(
        image.image_url,
        "https://www.minecraft.net/content/dam/games/minecraft/1_21.png"
    );
    assert_eq!(obj.article_grid[0].primary_category, "news");
    assert_eq!(obj.article_grid[0].article_url, "/article/tricky-trials");

    // ArticleGridObj 单独反序列化
    let grid: ArticleGridObj = serde_json::from_str(
        r#"{
            "default_tile": { "title": "t", "sub_header": "", "tile_size": "", "image": { "content_type": "", "imageURL": "", "alt": "" } },
            "primary_category": "",
            "article_url": ""
        }"#,
    )
    .unwrap();
    assert_eq!(grid.default_tile.title, "t");
}

/// Token 响应字段缺失时应用默认值，靠 serde 的 default 属性
#[test]
fn token_res_defaults_on_missing_fields() {
    let obj: MinecraftTokenResObj = serde_json::from_str("{}").unwrap();
    assert_eq!(obj.access_token, "");
    assert_eq!(obj.expires_in, 0);

    // expires_in <= 0 时 get_minecraft_token 应视为失效，报 AuthTokenTimeout
    assert!(obj.expires_in <= 0);
}

/// 获取主版本清单，真实网络请求，默认忽略。
///
/// 运行方式：
/// `cargo test -p mml-net --test mojang_api_test -- --ignored`
#[tokio::test]
#[ignore = "需要真实网络：请求 Mojang 版本清单，cargo test -- --ignored 运行"]
async fn versions_manifest_network() {
    // 初始化配置与 HTTP 客户端，用临时目录，测后清理
    let dir = mml_testutil::temp_dir().join(format!("mml-net-mojang-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时配置目录失败");
    mml_config::init(&dir);
    mml_net::init();

    let data = get_versions(None).await.expect("版本清单请求失败");
    assert!(!data.is_empty(), "版本清单不应为空");

    let json: serde_json::Value = serde_json::from_slice(&data).expect("清单应为合法 JSON");
    assert!(json.get("latest").is_some(), "清单应包含 latest 字段");
    assert!(json.get("versions").is_some(), "清单应包含 versions 字段");

    let _ = std::fs::remove_dir_all(&dir);
}
