//! API 端点 URL 常量
//!
//! 本模块集中定义了启动器使用的所有外部 API 端点和资源下载 URL。
//! 通过将 URL 集中管理，便于维护和统一替换镜像源。

// ============================================================================
// 第三方认证服务
// ============================================================================

/// LittleSkin 皮肤站官方地址
pub const LITTLE_SKIN_URL: &str = "https://littleskin.cn/";

/// 统一通行证 Nide8 认证服务器地址
pub const NIDE8_URL: &str = "https://auth.mc-user.com:233/";
/// 统一通行证 JAR 下载地址
pub const NIDE8_JAR_URL: &str = "https://login.mc-user.com:233/index/jar";

// ============================================================================
// Microsoft OAuth 2.0 端点
// ============================================================================

/// Microsoft 设备授权端点
pub const OAUTH_CODE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
/// Microsoft Token 端点
pub const OAUTH_TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
/// Xbox Live 认证端点
pub const XBOX_LIVE: &str = "https://user.auth.xboxlive.com/user/authenticate";
/// XSTS 安全令牌服务端点
pub const XSTS: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";

// ============================================================================
// Java 运行时下载
// ============================================================================

/// Adoptium Eclipse Temurin API 地址
pub const ADOPTIUM_URL: &str = "https://api.adoptium.net/";

/// Foojay Disco API 地址，v3.0
pub const FOOJAY: &str = "https://api.foojay.io/disco/v3.0/";

// ============================================================================
// 镜像源
// ============================================================================

/// BMCLAPI 国内镜像源，bangbang93 维护
pub const BMCLAPI: &str = "https://bmclapi2.bangbang93.com/";

// ============================================================================
// Mojang 官方 API 端点
// ============================================================================

/// Mojang 启动器元数据
pub const MOJANG_META: &str = "https://launchermeta.mojang.com/";
/// Mojang 启动器资源
pub const MOJANG_LAUNCHER: &str = "https://launcher.mojang.com/";
/// Mojang Piston 数据
pub const MOJANG_PISTON_DATA: &str = "https://piston-data.mojang.com/";
/// Mojang Piston 元数据
pub const MOJANG_PISTON_META: &str = "https://piston-meta.mojang.com/";

/// 所有 Mojang 官方域名的列表，用于镜像源替换
pub const MOJANG: [&str; 4] = [
    MOJANG_META,
    MOJANG_LAUNCHER,
    MOJANG_PISTON_DATA,
    MOJANG_PISTON_META,
];

/// Minecraft 运行库下载地址
pub const MINECRAFT_LIBRARIES: &str = "https://libraries.minecraft.net/";
/// Minecraft 资源文件下载地址
pub const MINECRAFT_RESOURCES: &str = "https://resources.download.minecraft.net/";

// ============================================================================
// 模组加载器 Maven 仓库
// ============================================================================

/// Minecraft Forge Maven 仓库
pub const FORGE: &str = "https://maven.minecraftforge.net/";
/// NeoForge Maven 仓库
pub const NEOFORGE: &str = "https://maven.neoforged.net/";

/// Fabric Maven 仓库
pub const FABRIC: &str = "https://maven.fabricmc.net/";
/// Fabric 元数据 API
pub const FABRIC_META: &str = "https://meta.fabricmc.net/";

/// Quilt Maven 仓库
pub const QUILT: &str = "https://maven.quiltmc.org/";
/// Quilt 元数据 API
pub const QUILT_META: &str = "https://meta.quiltmc.org/";

// ============================================================================
// 其他工具
// ============================================================================

/// Authlib-Injector 项目地址
pub const AUTHLIB: &str = "https://authlib-injector.yushi.moe/";

/// OptiFine 官网地址
pub const OPTIFINE: &str = "https://optifine.net/";

/// Maven Central 仓库
pub const MAVEN: &str = "https://repo1.maven.org/maven2/";
/// 阿里云 Maven 镜像仓库
pub const MAVEN_ALIYUN: &str = "https://maven.aliyun.com/repository/public/";

/// LiteLoader 下载地址
pub const LITELOADER: &str = "https://dl.liteloader.com/";

// ============================================================================
// Minecraft 服务 API
// ============================================================================

/// Minecraft 玩家档案 API
pub const MINECRAFT_SERVICES: &str = "https://api.minecraftservices.com/minecraft/profile";
/// Minecraft 会话服务器，用于查皮肤 / 披风
pub const MINECRAFT_SESSION_SERVER: &str =
    "https://sessionserver.mojang.com/session/minecraft/profile";
/// Minecraft 玩家名查 UUID，无需鉴权，查不到返回 204
pub const MINECRAFT_PROFILE_API: &str = "https://api.mojang.com/users/profiles/minecraft";
/// Minecraft Xbox 登录认证
pub const MINECRAFT_SERVICES_XBOX: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
/// Minecraft 官方新闻
pub const MINECRAFT_NEWS: &str = "https://www.minecraft.net/content/minecraftnet/language-masters/zh-hans/jcr:content/root/container/image_grid_a_copy_64.articles.page-";

// ============================================================================
// CurseForge API
// ============================================================================

/// CurseForge 文件下载 CDN
pub const CURSEFORGE_DOWNLOAD: &str = "https://edge.forgecdn.net/";
/// CurseForge REST API
pub const CURSEFORGE: &str = "https://api.curseforge.com/v1/";

// ============================================================================
// Modrinth API
// ============================================================================

/// Modrinth 官网地址
pub const MODRINTH: &str = "https://modrinth.com/";
/// Modrinth REST API
pub const MODRINTH_API: &str = "https://api.modrinth.com/v2/";
/// Modrinth 文件下载 CDN
pub const MODRINTH_DOWNLOAD: &str = "https://cdn.modrinth.com/";

// ============================================================================
// chunkbase API
// ============================================================================

/// Chunkbase 种子地图页面地址
pub const CHUNKBASE: &str = "https://www.chunkbase.com/apps/seed-map#";

// ============================================================================
// OpenFrp API
// ============================================================================

/// OpenFrp 开发者 API
pub const OPENFRP: &str = "https://of-dev-api.bfsea.com/api";
/// OpenFrp 软件下载配置地址
pub const OPENFRP_DOWNLOAD: &str = "https://console.openfrp.net/web/commonQuery/get?key=software";

// ============================================================================
// SakuraFrp API
// ============================================================================

/// SakuraFrp 樱花内网穿透 API
pub const SAKURA_FRP: &str = "https://api.natfrp.com/v4/";

/// IBM Semeru 下载页接口，响应里带 HTML 与内嵌 JS
pub const OPENJ9: &str =
    "https://developer.ibm.com/middleware/v1/contents/static/semeru-runtime-downloads";

/// Azul Zulu 包列表接口，只取社区版
pub const ZULU: &str = "https://www.azul.com/wp-admin/admin-ajax.php?action=bundles&endpoint=community&use_stage=false&include_fields=java_version%2Copenjdk_build_number%2Crelease_status%2Csupport_term%2Cos%2Carch%2Chw_bitness%2Cabi%2Cbundle_type%2Cjavafx%2Clatest%2Cext%2Cname%2Csha256_hash%2Curl%2Ccpu_gen%2Cfeatures";

// ============================================================================
// 日志上传
// ============================================================================

/// mclo.gs 日志上传接口：表单字段 `content`，响应里带可分享的网址
pub const MCLO_PUSH: &str = "https://api.mclo.gs/1/log";
