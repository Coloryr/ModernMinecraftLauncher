//! 资源管理窗口 DTO —— 实例的模组 / 材质包 / 存档 / 截图 / 服务器 / 光影包 / 结构 / 数据包列表。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 模组条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModItemDto {
    /// 模组稳定标识，即文件路径的 uuid v5，启用 / 禁用 / 删除按此定位
    ///
    /// 注意：启用 / 禁用会改文件名 → 这个 uuid 跟着变，**不能拿它存分组**，见 `key`。
    /// `jar_in_jar` 里的内置模组没有独立文件，这里为空串。
    pub uuid: String,
    /// 文件 SHA1：**自定义分组的键**，与 `gui_setting.json` 的 `Mod.Groups` 一致
    ///
    /// 用内容哈希而不是 uuid：启用 / 禁用只是给文件名加减后缀，而 uuid 是文件路径的 v5 会变，
    /// 而 SHA1 不变 —— 否则一禁用就掉出分组。读不出元数据的坏 jar 没有哈希，为空串，不可归组。
    pub sha1: String,
    /// mods 目录下的文件名，显示与打开文件夹定位用
    pub file: String,
    /// 相对实例的路径，如 `.minecraft\mods\sodium.jar`
    ///
    /// 比文件名多一点定位信息、又不像绝对路径那样长到看不完；
    /// 内置模组 jar-in-jar 没有独立文件，为空串。
    pub path: String,
    /// 是否被禁用
    pub disable: bool,
    /// 是否读取失败，元数据解析出错
    pub fail: bool,
    /// 是否为 Core 模组
    pub core: bool,
    /// 是否是**库**，即内置 jar 里没有模组元数据的那些
    ///
    /// 内置 jar 有一大半是"被别的包当依赖带进来"的库，如 asm / mixinextras，
    /// 它们没有 `mods.toml` / `fabric.mod.json`。以前这类条目在 DTO 层被整条丢掉，
    /// file / modid / name 全空就 `return None`，界面上完全看不到 ——
    /// 用户要求"嵌的内置有可能是库而不是模组，需要体现出来"。
    pub library: bool,
    /// modid，可能为空
    pub mod_id: String,
    /// 显示名，元数据缺失时为空，前端回退文件名
    pub name: String,
    /// 版本号
    pub version: String,
    /// 作者，逗号拼接
    pub author: String,
    /// 描述
    pub description: String,
    /// **支持的加载器**，可能不止一个
    ///
    /// 一个 jar 里可以有多份元数据，例如同时带 `fabric.mod.json` 与 `META-INF/mods.toml`，
    /// 多加载器发布的包就是这么做的，每一份各自报一个加载器 —— 这里汇总去重后全给出来。
    ///
    /// 取值是 `LoaderType::id()`：forge / fabric / quilt / neoforge / optifine /
    /// liteloader / normal / custom；顺序按 `LoaderType` 的声明顺序，不是元数据出现顺序，
    /// 展示才稳定。读不出元数据时为空数组。
    pub loaders: Vec<String>,
    /// 加载侧，`LoadSideType` 小写：client / server / both / unknown
    pub side: String,
    /// 网页链接，即元数据里的 url，可能为空
    pub url: String,
    /// 下载源，取自 `get_source_type(pid, fid)`：curseforge / modrinth；不是从平台下的为空串
    pub source: String,
    /// 项目编号，来自实例的 `online_info.json`，按 SHA1 关联；空串 = 不是从平台下的
    pub project_id: String,
    /// 文件编号，口径同上
    pub file_id: String,
    /// 用户写的备注，即 `gui_setting.json` 的 `Mod.ModName`，没有则为空串
    ///
    /// 键按**去掉禁用后缀的文件名**归一，见 `resource.rs::mod_note_key`：
    /// 启用 / 禁用只给文件名加减 `.disabled`，不归一的话一禁用备注就丢了。
    pub note: String,
    /// 图标 data URL，无图标为空串
    pub icon: String,
    /// 内置模组，jar-in-jar / jarjar，递归
    ///
    /// 它们装在父 jar 里，没有独立文件：前端只展示，不提供启用 / 删除。
    pub jar_in_jar: Vec<ModItemDto>,
    /// 这个包有没有声明前置，右键菜单里「依赖图」靠它置灰
    ///
    /// **前置本身不进前端**：谁依赖谁在后端算（见 AGENTS.md §12.1），
    /// 要看图走 `resource_mods_dep_tree`。只收**当前加载器**那份元数据，与依赖检测同一口径。
    pub has_dependencies: bool,
}

/// 模组扫描进度，即事件 `resource-list-mods-progress` 的负载
///
/// 解析一个 jar 要读元数据 / 图标 / 扫 class，几百个包要好几秒 ——
/// 界面靠它显示 `x/x`，否则用户面对空列表不知道是在跑还是卡住了。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModScanProgressDto {
    /// 已完成的文件数
    pub done: usize,
    /// 需要处理的文件总数
    pub total: usize,
}

/// 模组更新的进度，即事件 `resource-mods-update-progress` 的负载
///
/// 一次更新要经过获取、下载、收尾三段，界面按 `stage` 换文案；
/// 没有进度用户都会以为卡死，所以三段都要报。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateProgressDto {
    /// 阶段：`check` 检查更新 / `fetch` 获取下载项 / `download` 下载 / `finish` 收尾写回
    pub stage: String,
    /// 这一阶段已处理的条目数
    pub done: usize,
    /// 这一阶段要处理的条目总数
    pub total: usize,
}

/// 资源窗口的视图偏好，存在**实例**的 `gui_setting.json` 里，跟着实例走
///
/// 与 `gui_config.json` 那份全局界面状态分开：这一份是"这个实例我习惯怎么看"，
/// 换个实例就该换一套，见 `gui_setting::GameViewSettingObj`。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceViewDto {
    /// 左侧分类的显示顺序，元素是分类 id 字符串；空数组 = 没存过，前端补默认顺序
    pub order: Vec<String>,
    /// 上次打开的类别，空串 = 没记过
    pub category: String,
    /// 模组的展示方式：list / table / tree，空串 = 没记过
    pub mod_view: String,
    /// 表格视图的列宽，列 key → 像素宽；空对象 = 没改过，前端用默认宽度
    ///
    /// 用裸 `HashMap`：IPC 生成器只认裸名，`std::collections::HashMap` 会被原样写进 TS。
    pub mod_col_widths: HashMap<String, u32>,
}

/// 启用 / 禁用模组之后的**新身份**
///
/// 这两个操作只是给文件名加减后缀，`.jar` ↔ `.jar.disabled`，元数据一个字都没变 ——
/// 但 `uuid` 是**文件路径的 v5**，展示用的文件名与相对路径也都跟着变。
/// 返回给前端，让它**就地更新那一行**，不必重扫整个 mods 目录 ——
/// 重扫要把每个 jar 的元数据重新解析一遍，几百个包要好几秒。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRenameDto {
    /// 改名后的 uuid，后续启用 / 禁用 / 删除要用它定位
    pub uuid: String,
    /// 改名后的文件名，位于 `mods` 目录下
    pub file: String,
    /// 改名后相对实例的路径，如 `.minecraft\mods\sodium.jar.disabled`
    pub path: String,
}

/// 模组自定义分组，存在实例的 `gui_setting.json` 的 `Mod.Groups` 里
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModGroupDto {
    /// 分组 uuid，**稳定标识**：改名不影响它，也是顺序 / 折叠状态用的键
    pub uuid: String,
    /// 分组名，用户可见，可改
    pub name: String,
    /// 组内模组的 **SHA1** 列表，有序：下发前排过序，底层是无序集合
    pub mods: Vec<String>,
}

/// 材质包条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackItemDto {
    /// resourcepacks 目录下的文件名
    pub file: String,
    /// 简介
    pub description: String,
    /// 版本号
    pub pack_format: i64,
    /// 最小版本
    pub min_format: i64,
    /// 最大版本号
    pub max_format: i64,
    /// 是否读取失败
    pub fail: bool,
    /// 是否启用，由 options.txt 的 `resourcePacks` 判定，见 `mml_game::game_resourcepacks`
    pub enable: bool,
    /// 图标 data URL，无图标为空串
    pub icon: String,
}

/// 存档条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveItemDto {
    /// saves 目录下的目录名，删除 / 备份按此定位
    pub dir: String,
    /// 世界名字
    pub level_name: String,
    /// 上次游玩时间，Unix 毫秒，未知为 0
    pub last_played: i64,
    /// 游戏类型：0 生存 / 1 创造 / 2 冒险
    pub game_type: i32,
    /// 极限模式
    pub hard_core: bool,
    /// 难度：0 和平 / 1 简单 / 2 普通 / 3 困难
    pub difficulty: u8,
    /// 是否损坏，即 level.dat 解析失败
    pub broken: bool,
    /// 地图种子
    ///
    /// 它在 level.dat 里是 i64，超过 JS 的安全整数范围，跨 IPC 一律走字符串：
    /// 当数字传会在前端被四舍五入，再存回来就悄悄改了用户的种子。
    pub seed: String,
    /// 图标 data URL，无图标为空串
    pub icon: String,
    /// 已有几个备份
    ///
    /// 由后端从备份索引里数出来，一次 JSON 读取；前端据此决定**要不要显示「还原」按钮** ——
    /// 没有备份的存档给个点不动的还原按钮没有意义。
    pub backups: u32,
}

/// 「编辑存档」提交的新信息，见 `resource_save_edit`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEditDto {
    /// 游戏模式：0 生存 / 1 创造 / 2 冒险 / 3 旁观，越界由后端夹到合法区间
    pub game_type: i32,
    /// 难度：0 和平 / 1 简单 / 2 普通 / 3 困难，越界由后端夹到合法区间
    pub difficulty: u8,
    /// 是否极限模式
    pub hard_core: bool,
    /// 世界名字，去空白后不能为空，否则后端返回 `err.saveNameEmpty`
    pub level_name: String,
    /// 地图种子，十进制整数、可带负号
    ///
    /// 与 [`SaveItemDto::seed`] 同一口径走字符串：i64 当数字传会被前端丢精度。
    /// 解析不出来时后端返回 `err.saveSeed`。
    pub seed: String,
}

/// 一个存档备份
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupDto {
    /// 备份文件名，还原时原样传回
    pub file: String,
    /// 体积，单位字节；文件不在时为 0
    pub size: u64,
    /// 落盘时间，Unix 毫秒；取不到时为 0
    pub time: i64,
}

/// 截图条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotItemDto {
    /// 文件名，预览 / 删除按此定位
    pub name: String,
}

/// 服务器条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerItemDto {
    /// 服务器名，与 ip 一起构成定位键
    pub name: String,
    /// 服务器地址
    pub ip: String,
    /// 是否接受服务器资源包
    pub accept_textures: bool,
    /// 图标 data URL，无图标为空串
    pub icon: String,
}

/// 光影包条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderItemDto {
    /// shaderpacks 目录下的文件名
    pub file: String,
    /// 显示名，从语言文件解析，缺失时回退文件名
    pub name: String,
    /// 描述
    pub comment: String,
    /// 是否为当前启用的光影包，由 options.txt 的 shaderPack 键判定
    pub selected: bool,
}

/// 结构文件条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchematicItemDto {
    /// schematics 目录下的文件名，删除按此定位
    pub file: String,
    /// 名称
    pub name: String,
    /// 作者
    pub author: String,
    /// 描述
    pub description: String,
    /// 类型标签：Minecraft / Litematic / WorldEdit / Create
    pub type_name: String,
    /// 宽
    pub width: i32,
    /// 高
    pub height: i32,
    /// 长
    pub length: i32,
    /// 方块总数
    pub block_count: u64,
    /// 方块种类数
    pub block_types: u32,
    /// 是否解析失败
    pub fail: bool,
}

/// 结构文件里的一种方块
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchematicBlockDto {
    /// 方块 ID，形如 `minecraft:stone`
    pub id: String,
    /// 该方块出现的次数
    pub count: u64,
}

/// 结构文件详情，比列表条目多一份逐种方块的数量
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchematicDetailDto {
    /// schematics 目录下的文件名，打开目录按此定位
    pub file: String,
    /// 名称
    pub name: String,
    /// 作者，只有 litematic 会填
    pub author: String,
    /// 描述，只有 litematic 会填
    pub description: String,
    /// 类型标签：Minecraft / Litematic / WorldEdit / Create
    pub type_name: String,
    /// 宽
    pub width: i32,
    /// 高
    pub height: i32,
    /// 长
    pub length: i32,
    /// 方块总数
    pub block_count: u64,
    /// 方块种类数
    pub block_types: u32,
    /// 逐种方块的数量，按数量从多到少排序
    pub blocks: Vec<SchematicBlockDto>,
    /// 是否解析失败，失败时其余字段是默认值、`blocks` 为空
    pub fail: bool,
}

/// 数据包条目，存档子页
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataPackItemDto {
    /// NBT 里的登记名，形如 file/xxx，toggle / 删除按此定位
    pub name: String,
    /// datapacks 目录下的文件或目录名
    pub file: String,
    /// 描述，取自 pack.mcmeta
    pub description: String,
    /// 格式版本号
    pub pack_format: i64,
    /// 是否启用，null = 未登记进 level.dat
    pub enable: Option<bool>,
}

/// 拖拽导入到资源目录的结果，资源窗口拖入文件时返回
///
/// `mode = "check"` 时只填 `conflicts`，预检且不落盘；前端据此弹「覆盖 / 跳过 / 重命名」；
/// 用户选完之后再带真正的 mode 回来导入，那时 `imported` / `skipped` 才有内容。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDropDto {
    /// 目标位置已经存在的同名项
    pub conflicts: Vec<String>,
    /// 实际导入的名字，重命名过的是新名字
    pub imported: Vec<String>,
    /// 被跳过的名字，冲突时选了"跳过"
    pub skipped: Vec<String>,
}

/// 模组依赖检测发现的问题种类
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DepProblemKindDto {
    /// 缺失：装着的模组里没有这个前置
    Missing,
    /// 版本不符：装着了，但没有一个版本落在要求的范围内
    Mismatch,
    /// 前置被禁用：只有禁用状态的包提供它
    Disabled,
}

/// 模组依赖检测发现的一条问题
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepProblemDto {
    /// 出问题的前置 modid
    pub modid: String,
    /// 问题种类
    pub kind: DepProblemKindDto,
    /// 要求的版本范围，`*` 表示不限
    pub required: String,
    /// 已装着的版本，多个用 `, ` 连接；缺失时为空串
    pub installed: String,
    /// 有哪些模组要求它
    pub required_by: Vec<String>,
}

/// 重复安装的一个 modid
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDuplicateDto {
    /// 重复的 modid
    pub modid: String,
    /// 提供它的顶层文件，按文件名排序
    pub files: Vec<ModDuplicateFileDto>,
}

/// 重复安装里涉及的一个文件
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDuplicateFileDto {
    /// 文件名，不含目录
    pub file: String,
    /// 该文件声明的版本；没写时为空串
    pub version: String,
}

/// 模组更新检查的整份结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateCheckDto {
    /// 参与检查的模组数，即项目号与文件号都非空、带在线信息的那些
    pub checked: u32,
    /// 查询失败的个数，网络或接口出错
    ///
    /// 失败不等于"已是最新"：这部分没查出来，界面要如实说出来，
    /// 否则用户会以为它们都是最新的。
    pub failed: u32,
    /// 有新版本的条目
    pub updates: Vec<ModUpdateDto>,
}

/// 模组更新检查里的一条可用更新
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateDto {
    /// 当前模组条目的 uuid，文件路径的 v5，用来与列表那一行对齐
    pub uuid: String,
    /// 当前文件名，禁用状态下带 `.disabled` 后缀
    pub file: String,
    /// 是否被禁用
    pub disable: bool,
    /// 显示名，元数据缺失时为空，前端回退文件名
    pub name: String,
    /// 当前版本号，元数据里读到的；没写时为空串
    pub version: String,
    /// 下载源：curseforge / modrinth
    pub source: String,
    /// 项目编号，后续按它下新文件
    pub project_id: String,
    /// 当前文件编号
    pub file_id: String,
    /// 新文件的编号
    pub new_file_id: String,
    /// 新版本号；CurseForge 没有版本号字段，取文件的显示名
    pub new_version: String,
    /// 新文件名
    pub new_file: String,
    /// 新文件发布时间，ISO 8601 字符串
    pub date: String,
}

/// 一个项目的全部更新记录，即实例 `mod_update.json` 里的一项
///
/// 由 `resource_mods_update_log` 读出；项目之间按各自最新一条的时间倒序
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateProjectDto {
    /// 项目编号，即表键
    pub project_id: String,
    /// 模组名字；没记过时为空串，界面回退项目编号
    pub name: String,
    /// 该项目的更新历史，**新的在前**
    pub update: Vec<ModUpdateRecordDto>,
}

/// 一条模组更新记录，即实例目录 `mod_update.json` 里的一项里的一条
///
/// 记录本身在后端换文件成功时追加，见 `InstanceSettingObj::append_mod_update`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateRecordDto {
    /// 换上去的文件名，即平台上的文件名
    pub file: String,
    /// 换之前的版本号；取不到时为空串
    pub old_version: String,
    /// 换之后的版本号
    pub version: String,
    /// 换之前的文件号；取不到时为空串
    pub old_file_id: String,
    /// 换之后的文件号
    pub file_id: String,
    /// 更新时间，epoch 毫秒
    pub time: i64,
    /// 换上去那份文件的 SHA1
    pub sha1: String,
}

/// 「下载更新」勾选的一条：把这个模组换成平台上的新文件
///
/// 定位要用的三件事 + 更新记录要记的名字与两个版本号；列表里其余显示字段不用回传。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateApplyDto {
    /// 当前条目 uuid，文件路径的 v5，用来定位要替换掉的旧文件
    pub uuid: String,
    /// 下载源：curseforge / modrinth
    pub source: String,
    /// 项目编号
    pub project_id: String,
    /// 新文件编号
    pub new_file_id: String,
    /// 模组名字，即界面上显示的那个，只进更新记录；为空时后端保留原有名字
    pub name: String,
    /// 换之前的版本号，即界面上显示的那个，只进更新记录
    pub old_version: String,
    /// 换之后的版本号，即平台上的版本号 / 显示名，只进更新记录
    pub new_version: String,
}

/// 依赖图里的一个节点，同一个 modid 只有一个
///
/// 口径与内核依赖检测（`mods_dep_test`）一致：装没装、缺没缺、是游戏 / 加载器提供
/// 还是模组提供，都在后端算好；前端只负责画。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDepNodeDto {
    /// 唯一键：modid 小写；模组没有 modid 的极少数情况退回文件名
    pub key: String,
    /// 显示名：有条目用条目名，否则用 modid
    pub label: String,
    /// 原始大小写的 modid
    pub mod_id: String,
    /// 已装版本，多份提供者用 `, ` 连接；虚拟前置是它的版本；都没有时为空串
    pub version: String,
    /// 图标 data URL，没有时为空串
    pub icon: String,
    /// 是不是被选中的那个模组，即图的根
    pub root: bool,
    /// 由内置 jar（jar-in-jar）提供
    pub nested: bool,
    /// 由游戏本体 / 加载器 / 运行环境提供，即"虚拟前置"
    pub r#virtual: bool,
    /// 装了：有模组提供，或由游戏 / 加载器提供
    pub installed: bool,
    /// 缺失：有强制前置指向它，但既没有模组提供、也不是虚拟前置
    pub missing: bool,
    /// 只有禁用状态的包提供它
    pub disabled: bool,
    /// 必装：至少有一条强制边指向它；只有可选边指向它时为假
    pub mandatory: bool,
    /// 指向它的边声明的版本要求，去重后按出现顺序；不限版本的不收
    pub required: Vec<String>,
    /// 依赖检测出的问题种类，没有时为空
    ///
    /// 与「依赖检测」同一个口径，节点靠它决定打勾还是打感叹号。
    pub problem: Option<DepProblemKindDto>,
}

/// 依赖图里的一条连线，同一对节点只会有一条
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDepEdgeDto {
    /// 唯一 id，前端靠它认边
    pub id: String,
    /// 起点节点键，即"谁需要"
    pub from: String,
    /// 终点节点键，即"需要谁"
    pub to: String,
    /// 推荐安装（非强制）
    pub optional: bool,
    /// 声明里的版本要求原文，不限版本是 `*`
    pub required: String,
    /// 环上的回边：终点是这次遍历里的上游祖先
    pub backward: bool,
    /// 由**加载器指向 minecraft** 的合成边，不是模组声明的
    pub loader: bool,
}

/// 某个模组的依赖图
///
/// 只有节点与连线，**不带坐标**：布局是这一屏的事，由前端用 dagre 算。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDepTreeDto {
    /// 节点
    pub nodes: Vec<ModDepNodeDto>,
    /// 连线
    pub edges: Vec<ModDepEdgeDto>,
}

/// 整合包更新检查的结果
///
/// 实例里记着装的是哪一份（`pid` / `fid`），拿它与平台上的最新文件比；
/// 比得上就是最新，比不上就把新文件的信息带回去，前端据此问一句要不要更新。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModPackUpdateDto {
    /// 有没有新版本
    pub has_update: bool,
    /// 下载源：curseforge / modrinth
    pub source: String,
    /// 整合包项目编号
    pub project_id: String,
    /// 现在装着的那份文件编号
    pub file_id: String,
    /// 平台上的最新文件编号；接口没返回文件时为空串
    pub new_file_id: String,
    /// 新版本名；CurseForge 取文件显示名，Modrinth 取版本号
    pub new_version: String,
    /// 新文件名
    pub new_file: String,
    /// 新文件发布时间，ISO 8601 字符串
    pub date: String,
}

/// 自动标记的进度，即事件 `resource-mods-auto-mark-progress` 的负载
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModAutoMarkProgressDto {
    /// 已完成数
    pub done: u32,
    /// 总数
    pub total: u32,
}

/// 自动标记的结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModAutoMarkDto {
    /// 这次要标记的条目数
    pub total: u32,
    /// 认出来并写好在线信息的条数
    pub marked: u32,
    /// 没认出来的条数：不在 Modrinth 上，或哈希对不上
    pub unknown: u32,
    /// 读文件失败的条数
    pub failed: u32,
}

/// 更新下载的结果，即事件 `resource-mods-update-done` 的负载
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateDoneDto {
    /// 替换成功的个数
    pub ok: u32,
    /// 失败的个数：平台查询、下载或写盘出错
    pub failed: u32,
}
