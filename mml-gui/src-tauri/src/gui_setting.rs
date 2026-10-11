//! 实例 GUI 设置，文件 gui_setting.json
//!
//! **归属 GUI，不属于内核**：日志窗口开关、模组分组与备注、方块背景、启动后自开日志窗、
//! 排列顺序 —— 这些都是"界面怎么看"，与启动流程无关，所以内核不读也不写它：
//! `InstanceSettingObj` 里没有对应字段，内核只提供 `get_gui_setting_file()` 定位路径。
//!
//! 存在实例目录下、与 `game.json` 并列的独立文件 `gui_setting.json`。
//!
//! **文件名是 M²L 自己的，与 ColorMC 的 `guisetting.json` 故意不同**：字段名虽然是大写开头、
//! 抄的是 ColorMC 的结构，但 `Mod.Groups` 的形状已经改过：对方是"组名 → SHA1 数组"，M²L 是
//! "分组 uuid → 对象"。若沿用同名文件，从 ColorMC 搬过来的实例会让 M²L 整份解析失败、
//! 回落成默认值，随后又把它覆盖掉 —— 改名之后，ColorMC 那份 `guisetting.json` 只是
//! 一个 M²L 不读的额外文件，迁移不会毁掉它，见 `windows/colormc.rs`。

use std::collections::{HashMap, HashSet};

use mml_base::serialize_tools;
use mml_config::config_save;
use mml_game::launcher::instance_setting_obj::InstanceSettingObj;
use serde::{Deserialize, Serialize};

/// 游戏日志窗口设置，字段与 ColorMC 的 `GameLogSettingObj` 同名
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct GameLogSettingObj {
    /// 自动换行
    #[serde(rename = "WordWrap")]
    pub word_wrap: bool,
    /// 自动下拉，跟随最新日志滚动
    #[serde(rename = "Auto")]
    pub auto: bool,
    /// 显示 None 级日志
    #[serde(rename = "EnableNone")]
    pub enable_none: bool,
    /// 显示 Info 级日志
    #[serde(rename = "EnableInfo")]
    pub enable_info: bool,
    /// 显示 Warn 级日志
    #[serde(rename = "EnableWarn")]
    pub enable_warn: bool,
    /// 显示 Error 级日志
    #[serde(rename = "EnableError")]
    pub enable_error: bool,
    /// 显示 Debug 级日志
    #[serde(rename = "EnableDebug")]
    pub enable_debug: bool,
}

impl Default for GameLogSettingObj {
    /// 默认：换行开、自动下拉关、五个等级全开
    ///
    /// 等级默认全开是刻意的 —— 日志窗口的价值就在于"出事时能看到全部"，
    /// 默认关掉某些等级会让用户以为日志里没有那条记录。
    fn default() -> Self {
        Self {
            word_wrap: true,
            auto: false,
            enable_none: true,
            enable_info: true,
            enable_warn: true,
            enable_error: true,
            enable_debug: true,
        }
    }
}

/// 一个模组自定义分组
///
/// 分组用 **uuid 作键**，见 [`GameModSettingObj::groups`]：
/// - 改名不影响归属，名字可以随便改，键不动；
/// - 分组名允许任意文本，也不会再和"状态分组"这种内部块撞名。原先自建分组用组名作键，
///   得靠 `$` 前缀把状态分组区分开。
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GameModGroupObj {
    /// 分组名，用户可见；可重名检查在 `resource.rs` 里做
    #[serde(rename = "Name")]
    pub name: String,
    /// 该组的模组 **SHA1** 列表
    ///
    /// 用内容哈希而不是文件名 / uuid：启用 / 禁用只改文件名，SHA1 不变，
    /// 详见 `resource.rs` 的说明。
    #[serde(rename = "Mods")]
    pub mods: HashSet<String>,
}

/// 模组显示设置
///
/// **不再与 ColorMC 互通**，用户明确要求：`Groups` 从原来的
/// 「分组名 → SHA1 集合」改成「分组 uuid → [`GameModGroupObj`]」，
/// 分组名挪进对象里当一个普通字段。ColorMC 按名字查 `Groups[name]`，读这份文件会
/// 看不到分组 —— 这是有意为之，换来的是"改名不改键、不会与内部块撞名"。
///
/// 注意 `#[serde(default)]` 的语义：整个 `Mod` 字段缺失时用 [`Default`]，
/// 而**单个**字段缺失时用该字段类型的默认值，`Vec::default()` 就是空表。
/// 所以"已启用默认收起"不能只写在 [`Default`] 里 —— 老文件有 `Mod` 但没有
/// `GroupCollapsed`，那个分支会给出空表。缺字段的兜底见 `resource.rs` 的读取处。
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GameModSettingObj {
    /// 模组自定义分组：**分组 uuid → 分组**，分组含名字与模组 SHA1 列表
    ///
    /// 状态分组即已启用 / 已禁用 / 识别失败这三块，**不在这一列**：它们不是用户数据，
    /// 由 `resource.rs` 用固定 uuid 现拼，见那里的 `STATE_GROUP_*`。
    #[serde(rename = "Groups")]
    pub groups: HashMap<String, GameModGroupObj>,
    /// 备注：模组SHA1 → 用户写的说明
    #[serde(rename = "ModName")]
    pub mod_name: HashMap<String, Option<String>>,
    /// 分组块的显示顺序，存**分组 uuid**，含三个状态分组的固定 uuid
    ///
    /// `Groups` 是 `HashMap`、没有"建立顺序"可言，所以顺序单独存一份。
    #[serde(rename = "GroupOrder")]
    pub group_order: Vec<String>,
    /// **收起**的分组块键集合，存分组 uuid，口径与 `GroupOrder` 一致
    ///
    /// 存"收起的那些"而不是"展开的那些"：默认全展开，空数组即"全部展开"。
    ///
    /// `Option` 是为了区分**两种情况**，两者行为不同，不能都用空表表示：
    /// - `None`：这个字段从来没写过，老文件 / 新实例都走这里 → 用"已启用默认收起"的初值；
    /// - `Some(vec![])`：用户手动把「已启用」展开了 → 尊重用户，保持展开。
    #[serde(rename = "GroupCollapsed")]
    pub group_collapsed: Option<Vec<String>>,
}

/// 实例的**界面视图**设置，跟着实例走的那部分
///
/// 只放"换实例就该换一套"的东西：资源窗口的左侧分类顺序 / 上次打开的类别 / 模组展示方式。
/// 全局性的界面设置如主题、颜色、字体、语言、窗口模式**不在这里** ——
/// 它们与实例无关，放在 `gui_config.json`，也就是跨窗口那份"界面状态"里。
///
/// 这是本启动器自己的扩展字段，ColorMC 的结构里没有，存在 `Gui` 之下 ——
/// 文件名与对方不同，见文件头，所以这份数据只在 M²L 这边有意义。
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GameViewSettingObj {
    /// 资源窗口：左侧分类的显示顺序，由用户拖出来
    ///
    /// 存分类 id 字符串，如 saves / mods。**没存过就是空数组** ——
    /// 排序口径由前端 `useResourceView` 负责，新增分类补末尾、未知分类丢弃，
    /// 后端只当它是"一串字符串"保管，不认识具体分类。
    #[serde(rename = "ResourceOrder")]
    pub resource_order: Vec<String>,
    /// 资源窗口：上次打开的类别；空串表示没记过，前端用默认值
    #[serde(rename = "ResourceCategory")]
    pub resource_category: String,
    /// 资源窗口：模组的展示方式，取值 list / table / tree；空串表示没记过
    #[serde(rename = "ResourceModView")]
    pub resource_mod_view: String,
    /// 资源窗口：**表格视图的列宽**，键是列 key，值是像素宽
    ///
    /// 与 `resource_order` 同理：后端只当它是"一串键值对"保管，不认识有哪几列，
    /// 也不做范围校验，那是前端 `MOD_COLS` / `normalizeColWidths` 的事，新增列只改前端。
    #[serde(rename = "ResourceModColWidths")]
    pub resource_mod_col_widths: HashMap<String, u32>,
}

/// 实例 GUI 设置，字段与 ColorMC 的 `GameGuiSettingObj` 同名
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GameGuiSettingObj {
    /// 日志设置
    #[serde(rename = "Log")]
    pub log: GameLogSettingObj,
    /// 模组显示设置
    #[serde(rename = "Mod")]
    pub mods: GameModSettingObj,
    /// 显示方块：实例图标的方块 ID，与 `InstanceSettingObj.icon` 二选一
    #[serde(rename = "Block")]
    pub block: Option<String>,
    /// 自动打开日志窗口，游戏启动后生效
    #[serde(rename = "LogAutoShow")]
    pub log_auto_show: bool,
    /// 界面视图设置，跟实例走的那部分，见 [`GameViewSettingObj`]
    #[serde(rename = "Gui")]
    pub view: GameViewSettingObj,
}

/// 读取某实例的 GUI 设置
///
/// 文件不存在 / 解析失败时返回默认值而不是报错：GUI 设置是**可选**的附属数据，
/// 老实例、手改坏的、别的启动器建的目录都可能没有这个文件，
/// 不该因为它缺了就影响实例的使用。
///
/// - `instance`: 实例配置，用它的目录定位文件
///
/// # 返回值
///
/// 该实例的 GUI 设置；文件缺失或解析失败时是默认值
pub fn load(instance: &InstanceSettingObj) -> GameGuiSettingObj {
    let file = instance.get_gui_setting_file();
    if !file.exists() || !file.is_file() {
        return GameGuiSettingObj::default();
    }

    serialize_tools::json_from_file::<GameGuiSettingObj>(&file).unwrap_or_default()
}

/// 保存某实例的 GUI 设置，异步落盘
///
/// - `instance`: 实例配置，用它的目录定位文件
/// - `obj`: 待保存的设置
pub fn save(instance: &InstanceSettingObj, obj: &GameGuiSettingObj) {
    config_save::save(instance.uuid, obj, instance.get_gui_setting_file());
}

/// 读-改-写：把方块 ID 设为 `block`，并按二选一规则清掉 `Icon`
///
/// 图标与方块 ID 二选一：
/// - 设方块 ID → 写 `Block`、清 `InstanceSettingObj.icon`，不再用 icon.png；
/// - 上传图片 → 写 `icon.png`、清 `Block`，见 [`clear_block`]。
///
/// 两步必须一起做：只写一个会让图标读取走错分支。
///
/// - `instance`: 实例配置，用它定位文件并回写 icon
/// - `block`: 方块 ID
pub fn set_block(instance: &InstanceSettingObj, block: String) {
    let mut obj = load(instance);
    obj.block = Some(block);
    save(instance, &obj);
}

/// 清掉方块 ID，上传自定义图标图片时调用
///
/// - `instance`: 实例配置，用它定位文件
pub fn clear_block(instance: &InstanceSettingObj) {
    let mut obj = load(instance);
    if obj.block.is_some() {
        obj.block = None;
        save(instance, &obj);
    }
}

/// 该实例的图标是否应当由方块 ID 现渲染
///
/// 给 `image_manager` 的图标取图分支用。
///
/// - `instance`: 实例配置
///
/// # 返回值
///
/// 设了方块 ID 返回它，否则 `None`
pub fn block_icon_id(instance: &InstanceSettingObj) -> Option<String> {
    load(instance).block
}
