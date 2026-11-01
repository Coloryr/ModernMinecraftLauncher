//! 通用事件系统
//!
//! 提供基于回调的发布-订阅事件机制，支持带参数和无参数两种模式。
//! 使用 `RwLock<HashMap<u64, Box<dyn Fn>>>` 存储回调，原子自增 ID 管理。

use std::{
    collections::HashMap,
    sync::{
        RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

/// 带参数的通用事件，回调存于 HashMap，调用顺序不保证与注册顺序一致
pub struct EventArgHandler<E> {
    /// 回调表，键为自增编号
    handlers: RwLock<HashMap<u64, Box<dyn Fn(&E) + Send + Sync>>>,
    /// 下一个待分配的回调编号
    index: AtomicU64,
}

impl<E> EventArgHandler<E> {
    /// 创建不带任何回调的事件处理器
    ///
    /// # 返回值
    ///
    /// 回调表为空，编号从 0 开始分配
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            index: AtomicU64::new(0),
        }
    }

    /// 添加事件处理器
    ///
    /// - `handler`: 事件触发时调用的回调
    ///
    /// # 返回值
    ///
    /// 返回处理器编号，用于 [`remove_handel`](Self::remove_handel) 移除
    pub fn add_handler<F>(&self, handler: F) -> u64
    where
        F: Fn(&E) + Send + Sync + 'static,
    {
        let id = self.gen_next_id();
        self.handlers.write().unwrap().insert(id, Box::new(handler));

        id
    }

    /// 移除事件处理器，编号不存在时什么都不做
    ///
    /// - `id`: 处理器编号
    pub fn remove_handel(&self, id: u64) {
        self.handlers.write().unwrap().remove(&id);
    }

    /// 触发事件，调用全部回调，顺序为 HashMap 迭代顺序
    ///
    /// - `event`: 事件参数
    pub fn emit(&self, event: E) {
        for (_, handler) in self.handlers.read().unwrap().iter() {
            handler(&event);
        }
    }

    /// 取下一个未使用的回调编号
    fn gen_next_id(&self) -> u64 {
        self.index.fetch_add(1, Ordering::SeqCst)
    }
}

/// 不带参数的通用事件，回调存于 HashMap，调用顺序不保证与注册顺序一致
pub struct EventHandler {
    /// 回调表，键为自增编号
    handlers: RwLock<HashMap<u64, Box<dyn Fn() + Send + Sync>>>,
    /// 下一个待分配的回调编号
    index: AtomicU64,
}

impl EventHandler {
    /// 创建不带任何回调的事件处理器
    ///
    /// # 返回值
    ///
    /// 回调表为空，编号从 0 开始分配
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            index: AtomicU64::new(0),
        }
    }

    /// 添加事件处理器
    ///
    /// - `handler`: 事件触发时调用的回调
    ///
    /// # 返回值
    ///
    /// 返回处理器编号，用于 [`remove_handle`](Self::remove_handle) 移除
    pub fn add_handler<F>(&self, handler: F) -> u64
    where
        F: Fn() + Send + Sync + 'static,
    {
        let id = self.gen_next_id();
        self.handlers.write().unwrap().insert(id, Box::new(handler));

        id
    }

    /// 移除事件处理器，编号不存在时什么都不做
    ///
    /// - `id`: 处理器编号
    pub fn remove_handle(&self, id: u64) {
        self.handlers.write().unwrap().remove(&id);
    }

    /// 触发事件，调用全部回调，顺序为 HashMap 迭代顺序
    pub fn emit(&self) {
        for (_, handler) in self.handlers.read().unwrap().iter() {
            handler();
        }
    }

    /// 取下一个未使用的回调编号
    fn gen_next_id(&self) -> u64 {
        self.index.fetch_add(1, Ordering::SeqCst)
    }
}

#[cfg(test)]
#[path = "../tests/unit/events_tests.rs"]
mod tests;
