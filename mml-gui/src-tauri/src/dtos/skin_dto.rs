use serde::{Deserialize, Serialize};

/// 账户的皮肤 / 披风纹理列表，由 `account_get_textures` 返回
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TexturesDto {
    /// 皮肤列表，该账户没有则为空数组
    pub skins: Vec<TextureItemDto>,
    /// 披风列表，该账户没有则为空数组
    pub capes: Vec<TextureItemDto>,
}

/// 单个皮肤 / 披风纹理
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TextureItemDto {
    /// 纹理名，OAuth 档案取纹理 id，其他账户类型用账户 uuid
    pub name: String,
    /// 贴图内容 SHA1，经 `mml-image` 的 sha1 型 URI 取图
    pub sha1: String,
    /// 皮肤型号：`slim` 为纤细，空串为经典；披风恒为空串
    pub model: String,
    /// 是否为该账户当前选中的纹理
    pub active: bool,
}
