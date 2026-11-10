//! 条目查询与单条读取
//!
//! 条目列表、包含判断、顶层目录识别与单条内容读取。

use std::{io::Read, path::Path};

use mml_names::i18_items::error_type::CoreResult;

use super::{ArchiveEntryInfo, BaseArchive};

use crate::archives::ArchiveType;

impl BaseArchive {
    /// 返回检测到的压缩包类型
    ///
    /// # 返回值
    ///
    /// 打开时按文件名后缀判定出的格式
    pub fn archive_type(&self) -> ArchiveType {
        self.archive_type
    }

    /// 返回压缩包在磁盘上的路径
    ///
    /// # 返回值
    ///
    /// 打开时传入的文件路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 返回压缩包内所有条目
    ///
    /// 用于在不解压的情况下遍历压缩包内容
    ///
    /// # 返回值
    ///
    /// 返回打开时缓存的条目列表，增删后已同步刷新
    pub fn entries(&self) -> &[ArchiveEntryInfo] {
        &self.entries
    }

    /// 检查压缩包中是否存在指定名称的条目
    ///
    /// - `name`: 条目名，与包内路径完全匹配
    ///
    /// # 返回值
    ///
    /// 存在同名条目时为 `true`
    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.name == name)
    }

    /// 所有条目共享同一个顶层目录时返回该目录名
    ///
    /// 例如条目都以 `"MyWorld/"` 开头时返回 `Some("MyWorld")`
    ///
    /// # 返回值
    ///
    /// 根目录下直接有文件、或有多个顶层名字时返回 `None`
    pub fn single_top_dir(&self) -> Option<&str> {
        let mut top: Option<&str> = None;
        for entry in self.entries.iter() {
            let trimmed = entry.name.trim_end_matches(['/', '\\']);
            let mut parts = trimmed.split(['/', '\\']);
            let Some(first) = parts.next().filter(|s| !s.is_empty()) else {
                continue;
            };
            // 根目录下的单个文件也占一个首段，但它不是顶层目录
            if parts.next().is_none() && !entry.is_dir {
                return None;
            }
            match top {
                None => top = Some(first),
                Some(name) if name == first => {}
                Some(_) => return None,
            }
        }
        top
    }

    /// 读取压缩包内单个条目的内容到内存
    ///
    /// - `name`: 条目在压缩包内的名称/路径，如 `"subdir/readme.txt"`
    ///
    /// # 返回值
    ///
    /// 返回条目内容的原始字节；条目不存在、是目录或读取失败时返回错误
    pub fn read(&self, name: &str) -> CoreResult<Vec<u8>> {
        self.handle.lock().unwrap().read(name)
    }

    /// 以流式 [`Read`] 接口打开压缩包内的单个文件条目
    ///
    /// 与返回完整 `Vec<u8>` 的 [`read`](Self::read) 不同，此方法返回
    /// `Box<dyn Read>`，可直接传递给任何接受 [`Read`] trait 的函数
    ///
    /// - `name`: 条目在压缩包内的名称/路径，如 `"subdir/readme.txt"`
    ///
    /// # 返回值
    ///
    /// 返回只读流，由调用方负责读完或丢弃；条目不存在、是目录或读取失败时返回错误
    pub fn read_stream(&self, name: &str) -> CoreResult<Box<dyn Read>> {
        self.handle.lock().unwrap().read_stream(name)
    }
}
