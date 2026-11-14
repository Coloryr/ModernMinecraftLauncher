//! 添加实例窗口的模型与进度回调：窗口模型、重名确认对话框、进度桥
//!
//! 从 `add/mod.rs` 拆出来的。这一层是"窗口运行态"：`AddWindowModel` 持有取消令牌与关闭保护，
//! 另有重名确认与进度上报两个 `IAdd*Gui` 实现。
//! 重名确认的应答通道是**进程级**的：添加实例与下载整合包两个窗口都可能发起，
//! 而答复只带事件 id，不能绑在某个窗口的模型上，见 `CONFLICT_WAITERS`。
//! `AddWindowModel` 与 `instance_gui` 按原路径再导出，窗口生命周期、下载整合包窗口在用。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use mml_game::GameInstance;
use mml_game::gui_hook::{
    AddInstanceGui, AddModPackGui, AddModPackState, IAddInstanceGui, IAddModPackGui,
};
use tauri::{Emitter, Manager, WebviewWindow};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::dtos::{NameConflictDto, PackProgressDto};
use crate::err_keys;
use crate::listens;
use crate::windows;
use crate::windows::modpack_task::pack_state_id;

/// 添加实例窗口模型：只存取消令牌 / 关闭保护这些运行态，不落盘
pub(crate) struct AddWindowModel {
    /// 进行中安装任务的取消令牌，None 表示当前无任务
    cancel: Mutex<Option<CancellationToken>>,
    /// 关闭保护：查询数据期间为 true，`CloseRequested` 阶段拒绝关闭
    close_guard: AtomicBool,
}

impl Default for AddWindowModel {
    /// 等价于 [`Self::new`]
    fn default() -> Self {
        Self::new()
    }
}

impl AddWindowModel {
    /// 创建空模型，取消令牌与关闭保护都为空
    ///
    /// # 返回值
    ///
    /// 尚未开始任何安装任务的模型
    pub fn new() -> Self {
        Self {
            cancel: Mutex::new(None),
            close_guard: AtomicBool::new(false),
        }
    }

    /// 记录当前安装任务的取消令牌
    ///
    /// 覆盖旧令牌前**先取消它**：同窗口连着起两次安装时，旧任务的令牌会被悄悄换掉，
    /// 之后 `add_cancel` 取到的只有新令牌 —— 旧任务就再也取消不掉，只能等它跑完。
    ///
    /// - `token`: 新任务的取消令牌
    pub(crate) fn set_cancel(&self, token: CancellationToken) {
        if let Some(old) = self.cancel.lock().unwrap().replace(token) {
            old.cancel();
        }
    }

    /// 取走当前取消令牌，取后为 None
    ///
    /// # 返回值
    ///
    /// 有进行中的任务时返回它的取消令牌，否则 `None`
    pub(crate) fn take_cancel(&self) -> Option<CancellationToken> {
        self.cancel.lock().unwrap().take()
    }

    /// 开关关闭保护
    ///
    /// - `enabled`: 是否开启关闭保护
    pub fn set_close_guard(&self, enabled: bool) {
        self.close_guard.store(enabled, Ordering::Release);
    }

    /// 查询关闭保护
    ///
    /// # 返回值
    ///
    /// 关闭保护是否开启
    pub fn close_guard(&self) -> bool {
        self.close_guard.load(Ordering::Acquire)
    }
}

/// 取添加实例窗口模型
///
/// 按 kind 取而不是按调用方窗口：单窗口模式下"添加实例"只是主窗口里的一页，
/// 命令的调用方窗口是主窗口，按窗口取会拿到主窗口模型。
/// 模型由页面挂载时的 `window_ensure_model` 建立、切走时 `window_drop_model` 释放。
///
/// - `_window`: 调用方窗口，这里不参与取值，只为与其它取模型的函数保持同一签名
///
/// # 返回值
///
/// 该窗口的模型；缺失时返回 `err.modelMissing`
pub(super) fn model(_window: &WebviewWindow) -> Result<Arc<Mutex<AddWindowModel>>, String> {
    windows::model_for_kind("add").ok_or_else(|| err_keys::MODEL_MISSING.to_string())
}

/// 重名确认对话框事件 id 自增
pub(super) static DIALOG_ID: AtomicU32 = AtomicU32::new(1);

/// 一条还在等用户答复的重名确认
struct ConflictWaiter {
    /// 弹窗所在窗口的标签；答复必须来自同一窗口
    label: String,
    /// 应答发送端
    tx: oneshot::Sender<bool>,
}

/// 进程级的重名确认应答表：添加实例与下载整合包两个窗口都可能发起，
/// 答复只带事件 id，不能绑在某个窗口的模型上
static CONFLICT_WAITERS: LazyLock<Mutex<HashMap<u32, ConflictWaiter>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 答复一条重名确认；id 不存在或不是本窗口发起时忽略
///
/// - `label`: 答复方窗口标签
/// - `id`: 对话框事件 id
/// - `answer`: 用户是否同意
pub(crate) fn answer_conflict(label: &str, id: u32, answer: bool) {
    let mut waiters = CONFLICT_WAITERS.lock().unwrap();
    if waiters.get(&id).is_some_and(|waiter| waiter.label == label)
        && let Some(waiter) = waiters.remove(&id)
    {
        let _ = waiter.tx.send(answer);
    }
}

/// 窗口关闭：把发往它的待答确认全部按拒绝收尾，别让安装 future 永久 pending
///
/// - `label`: 被销毁窗口的标签
pub(crate) fn reject_conflicts_of(label: &str) {
    let mut waiters = CONFLICT_WAITERS.lock().unwrap();
    let ids: Vec<u32> = waiters
        .iter()
        .filter(|(_, waiter)| waiter.label == label)
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        if let Some(waiter) = waiters.remove(&id) {
            let _ = waiter.tx.send(false);
        }
    }
}

/// 实例重名确认对话框事件：kind 取 overwrite 覆盖 / rename 自动改名
///
/// - `window`: 目标窗口，事件只发给它自己
/// - `dto`: 对话框内容，含事件 id、确认类型与冲突的实例名
#[gui_macros::emit]
pub(super) fn emit_add_name_conflict(window: &WebviewWindow, dto: NameConflictDto) {
    let _ = window.emit_to(window.label(), listens::ADD_NAME_CONFLICT, dto);
}

/// 实例创建重名确认：向发起安装的页面发事件弹窗，等待用户在对话框上答复
pub(super) struct AddInstanceDialogGui {
    /// 弹窗与等待答复所属的窗口
    window: WebviewWindow,
    /// 发起确认的页面 kind，见 [`NameConflictDto::owner`]
    owner: &'static str,
}

impl AddInstanceDialogGui {
    /// 弹出确认框并等待答复；窗口已关闭时视为拒绝
    ///
    /// - `kind`: 确认类型，`overwrite` 覆盖 / `rename` 自动改名
    /// - `name`: 冲突的实例名
    ///
    /// # 返回值
    ///
    /// 用户同意返回 `true`；拒绝或窗口已关闭返回 `false`
    async fn ask(&self, kind: &str, name: &str) -> bool {
        // 窗口在注册等待前就没了：没人能答，按拒绝收尾，别把安装 future 挂在通道上
        if self
            .window
            .app_handle()
            .get_webview_window(self.window.label())
            .is_none()
        {
            return false;
        }
        let id = DIALOG_ID.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        CONFLICT_WAITERS.lock().unwrap().insert(
            id,
            ConflictWaiter {
                label: self.window.label().to_string(),
                tx,
            },
        );
        // 这条确认挡着安装流程：把窗口提到前台，别让用户在一堆窗口里找
        let _ = self.window.unminimize();
        let _ = self.window.set_focus();
        emit_add_name_conflict(
            &self.window,
            NameConflictDto {
                id,
                kind: kind.to_string(),
                name: name.to_string(),
                owner: self.owner.to_string(),
            },
        );
        rx.await.unwrap_or(false)
    }
}

#[async_trait]
impl IAddInstanceGui for AddInstanceDialogGui {
    /// 是否同意覆盖重名实例
    ///
    /// - `obj`: 与之重名的已有实例
    ///
    /// # 返回值
    ///
    /// 用户同意覆盖返回 `true`
    async fn overwrite(&self, obj: GameInstance) -> bool {
        let name = obj.read().unwrap().name.clone();
        self.ask("overwrite", &name).await
    }

    /// 是否同意自动修改名字
    ///
    /// - `name`: 自动改名后的新实例名
    ///
    /// # 返回值
    ///
    /// 用户同意改名返回 `true`
    async fn name_replace(&self, name: &str) -> bool {
        self.ask("rename", name).await
    }
}

/// 构建重名确认回调，跟随调用方窗口
///
/// - `window`: 弹窗与等待答复所属的窗口
/// - `owner`: 发起确认的页面 kind，`add` 或 `add_modpack`
///
/// # 返回值
///
/// 交给内核安装流程的重名确认回调
pub(crate) fn instance_gui(window: &WebviewWindow, owner: &'static str) -> AddInstanceGui {
    Some(Arc::new(AddInstanceDialogGui {
        window: window.clone(),
        owner,
    }) as Arc<dyn IAddInstanceGui>)
}

/// 整合包安装进度事件，跟随添加实例窗口
///
/// - `window`: 目标窗口，事件只发给它自己
/// - `dto`: 进度快照
#[gui_macros::emit]
pub(super) fn emit_add_pack_progress(window: &WebviewWindow, dto: PackProgressDto) {
    let _ = window.emit_to(window.label(), listens::ADD_PACK_PROGRESS, dto);
}

/// 整合包安装进度回调：把安装状态 / 进度转发为前端事件，供弹窗显示
pub(super) struct PackProgressGui {
    /// 进度事件的目标窗口
    window: WebviewWindow,
    /// 当前进度快照，各回调分别更新字段后整体发出
    dto: Mutex<PackProgressDto>,
    /// 上一次子进度事件的时间；子进度最多 100ms 发一次
    last_sub: Mutex<Instant>,
}

impl PackProgressGui {
    /// 更新进度快照并立即发事件
    ///
    /// - `f`: 就地改快照字段的闭包
    fn update(&self, f: impl FnOnce(&mut PackProgressDto)) {
        let mut dto = self.dto.lock().unwrap();
        f(&mut dto);
        emit_add_pack_progress(&self.window, dto.clone());
    }

    /// 更新子进度快照；100ms 内只发一次
    ///
    /// `force` 用于"这一批刚开始"（0 / 总数）：它紧跟在上一条子进度之后发，
    /// 被节流吞掉的话界面要等到第一个文件解压完才知道有子进度
    ///
    /// - `force`: 无视节流立即发出
    /// - `f`: 就地改快照字段的闭包
    fn update_sub(&self, force: bool, f: impl FnOnce(&mut PackProgressDto)) {
        let mut dto = self.dto.lock().unwrap();
        f(&mut dto);

        drop(dto);
        let mut last = self.last_sub.lock().unwrap();
        if !force && last.elapsed().as_millis() < 100 {
            return;
        }
        *last = Instant::now();
        drop(last);
        emit_add_pack_progress(&self.window, self.dto.lock().unwrap().clone());
    }
}

impl IAddModPackGui for PackProgressGui {
    /// 设置安装阶段
    ///
    /// - `state`: 内核给出的安装阶段
    fn set_state(&self, state: AddModPackState) {
        let id = pack_state_id(state);
        self.update(|dto| dto.state = id.into());
    }

    /// 设置主进度
    ///
    /// - `value`: 已完成数量
    /// - `all`: 总数量；`None` 表示未知，按 0 发给前端
    fn set_now(&self, value: usize, all: Option<usize>) {
        self.update(|dto| {
            dto.now = value as u32;
            dto.total = all.unwrap_or(0) as u32;
        });
    }

    /// 设置子任务说明文字
    ///
    /// - `text`: 说明文字；`None` 表示清空
    fn set_sub_text(&self, text: Option<String>) {
        self.update_sub(false, |dto| dto.sub_text = text);
    }

    /// 设置子任务进度
    ///
    /// - `value`: 已完成数量
    /// - `all`: 总数量；`None` 表示未知，按 0 发给前端
    fn set_sub_now(&self, value: usize, all: Option<usize>) {
        // 0 / 总数 = 这一批刚开始，必须立刻让界面看到子进度块
        let start = value == 0 && all.is_some_and(|total| total > 0);
        self.update_sub(start, |dto| {
            dto.sub_now = value as u32;
            dto.sub_total = all.unwrap_or(0) as u32;
        });
    }
}

/// 构建安装进度回调，跟随调用方窗口
///
/// - `window`: 进度事件的目标窗口
///
/// # 返回值
///
/// 交给内核安装流程的进度回调，初始阶段为 `downloadPack`
pub(super) fn pack_gui(window: &WebviewWindow) -> AddModPackGui {
    Some(Arc::new(PackProgressGui {
        window: window.clone(),
        last_sub: Mutex::new(Instant::now()),
        dto: Mutex::new(PackProgressDto {
            state: "downloadPack".into(),
            now: 0,
            total: 0,
            sub_text: None,
            sub_now: 0,
            sub_total: 0,
        }),
    }) as Arc<dyn IAddModPackGui>)
}
