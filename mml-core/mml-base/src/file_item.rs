//! 文件下载项目定义
//!
//! 定义下载文件相关的核心数据结构，包括哈希类型、下载后处理操作和文件条目。

use std::{
    io::{Seek, SeekFrom},
    path::PathBuf,
};

use mml_sys::path_helper;

use crate::hash_helper::{self, HashType};

/// 文件校验方式，决定下载后如何验证文件是否完整
#[derive(Debug, Clone)]
pub enum FileHash {
    /// 不校验，文件存在即视为有效
    None,
    /// MD5 校验值，十六进制串
    Md5(String),
    /// SHA1 校验值，十六进制串
    Sha1(String),
    /// SHA256 校验值，十六进制串
    Sha256(String),
    /// SHA512 校验值，十六进制串
    Sha512(String),
    /// SHA1 + SHA256 组合校验，两个值依次为 SHA1、SHA256
    Sha1Sha256(String, String),
    /// SHA1 + SHA512 组合校验，两个值依次为 SHA1、SHA512
    Sha1Sha512(String, String),
}

impl Default for FileHash {
    fn default() -> Self {
        FileHash::None
    }
}

/// 下载完成后的后续操作
#[derive(Debug, Clone)]
pub enum LaterRun {
    /// 不执行后续操作
    None,
    /// 解压 native 库，携带目标目录
    UnpackNative(PathBuf),
    /// 将下载到的 zip 解压到存档目录，携带存档目录
    UnpackSave(PathBuf),
    /// 换掉旧文件：新文件搬到最终路径之后，把携带的这个旧文件挪进回收站
    ReplaceOld(PathBuf),
}

impl Default for LaterRun {
    fn default() -> Self {
        LaterRun::None
    }
}

impl FileHash {
    /// 期望值是否为空
    ///
    /// 上游没公布哈希时会声明了算法但值是空串（Fabric 的 loader jar 就没有 `sha256`）。
    /// 这种情况按"没什么可校验"处理，否则文件会被判校验失败、反复重下。
    ///
    /// # 返回值
    ///
    /// 没有任何可校验的值时为 `true`
    pub fn is_empty(&self) -> bool {
        match self {
            FileHash::None => true,
            FileHash::Md5(hash)
            | FileHash::Sha1(hash)
            | FileHash::Sha256(hash)
            | FileHash::Sha512(hash) => hash.is_empty(),
            FileHash::Sha1Sha256(sha1, sha256) => sha1.is_empty() && sha256.is_empty(),
            FileHash::Sha1Sha512(sha1, sha512) => sha1.is_empty() && sha512.is_empty(),
        }
    }

    /// 检查给定校验值是否命中自身记录的哈希，区分大小写
    ///
    /// - `check`: 目标校验值
    ///
    /// # 返回值
    ///
    /// [`FileHash::None`] 恒为 `true`；组合变体命中任一个即为 `true`
    pub fn eq(&self, check: &str) -> bool {
        match self {
            FileHash::None => true,
            FileHash::Md5(hash) => hash == check,
            FileHash::Sha1(hash) => hash == check,
            FileHash::Sha256(hash) => hash == check,
            FileHash::Sha1Sha256(hash1, hash2) => hash1 == check || hash2 == check,
            FileHash::Sha512(hash) => hash == check,
            FileHash::Sha1Sha512(hash1, hash2) => hash1 == check || hash2 == check,
        }
    }

    /// 获取记录的 SHA1 值
    ///
    /// # 返回值
    ///
    /// 变体不含 SHA1 时返回 `None`
    pub fn get_sha1(&self) -> Option<String> {
        match self {
            FileHash::None => None,
            FileHash::Md5(_) => None,
            FileHash::Sha1(sha1) => Some(sha1.clone()),
            FileHash::Sha256(_) => None,
            FileHash::Sha512(_) => None,
            FileHash::Sha1Sha256(sha1, _) => Some(sha1.clone()),
            FileHash::Sha1Sha512(sha1, _) => Some(sha1.clone()),
        }
    }

    /// 获取记录的 SHA512 值
    ///
    /// # 返回值
    ///
    /// 变体不含 SHA512 时返回 `None`
    pub fn get_sha512(&self) -> Option<String> {
        match self {
            FileHash::None => None,
            FileHash::Md5(_) => None,
            FileHash::Sha1(_) => None,
            FileHash::Sha256(_) => None,
            FileHash::Sha512(sha512) => Some(sha512.clone()),
            FileHash::Sha1Sha256(_, _) => None,
            FileHash::Sha1Sha512(_, sha512) => Some(sha512.clone()),
        }
    }
}

/// 一个待下载的文件条目，描述下载目标、来源与校验方式
#[derive(Debug, Clone)]
pub struct FileItemObj {
    /// 文件名，仅作显示与标识
    pub name: String,
    /// 文件落盘的完整路径
    pub file: PathBuf,
    /// 下载地址
    pub url: String,
    /// 校验方式，决定下载后如何验证完整性
    pub hash: FileHash,
    /// 下载完成后的后续操作
    pub later: LaterRun,
}

impl Default for FileItemObj {
    fn default() -> Self {
        Self {
            name: Default::default(),
            file: Default::default(),
            url: Default::default(),
            hash: Default::default(),
            later: Default::default(),
        }
    }
}

impl FileItemObj {
    /// 按 [`FileHash`] 校验磁盘上的文件是否完整
    ///
    /// # 返回值
    ///
    /// 文件不存在、读取失败或校验不通过时为 `false`；[`FileHash::None`] 下文件存在即为 `true`
    pub fn check_hash(&self) -> bool {
        if self.file.exists() && self.file.is_file() {
            // 上游没给哈希（空串）时没什么可校验，文件在就算完整
            if self.hash.is_empty() {
                return true;
            }
            if let Ok(mut stream) = path_helper::open_read(&self.file) {
                match &self.hash {
                    FileHash::None => true,
                    FileHash::Md5(md5) => {
                        if let Ok(hash) =
                            hash_helper::gen_hash_from_reader(HashType::Md5, &mut stream)
                        {
                            hash.eq_ignore_ascii_case(md5)
                        } else {
                            false
                        }
                    }
                    FileHash::Sha1(sha1) => {
                        if let Ok(hash) =
                            hash_helper::gen_hash_from_reader(HashType::Sha1, &mut stream)
                        {
                            hash.eq_ignore_ascii_case(sha1)
                        } else {
                            false
                        }
                    }
                    FileHash::Sha256(sha256) => {
                        if let Ok(hash) =
                            hash_helper::gen_hash_from_reader(HashType::Sha256, &mut stream)
                        {
                            hash.eq_ignore_ascii_case(sha256)
                        } else {
                            false
                        }
                    }
                    FileHash::Sha1Sha256(sha1, sha256) => {
                        if let Ok(hash) =
                            hash_helper::gen_hash_from_reader(HashType::Sha1, &mut stream)
                            && hash.eq_ignore_ascii_case(sha1)
                        {
                            stream.seek(SeekFrom::Start(0)).unwrap();
                            if let Ok(hash) =
                                hash_helper::gen_hash_from_reader(HashType::Sha256, &mut stream)
                            {
                                hash.eq_ignore_ascii_case(sha256)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    FileHash::Sha512(sha512) => {
                        if let Ok(hash) =
                            hash_helper::gen_hash_from_reader(HashType::Sha512, &mut stream)
                        {
                            hash.eq_ignore_ascii_case(sha512)
                        } else {
                            false
                        }
                    }
                    FileHash::Sha1Sha512(sha1, sha512) => {
                        if let Ok(hash) =
                            hash_helper::gen_hash_from_reader(HashType::Sha1, &mut stream)
                            && hash.eq_ignore_ascii_case(sha1)
                        {
                            stream.seek(SeekFrom::Start(0)).unwrap();
                            if let Ok(hash) =
                                hash_helper::gen_hash_from_reader(HashType::Sha512, &mut stream)
                            {
                                hash.eq_ignore_ascii_case(sha512)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                }
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "hello world" 的常用哈希值
    const HELLO_MD5: &str = "5eb63bbbe01eeed093cb22bb8f5acdc3";
    const HELLO_SHA1: &str = "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed";
    const HELLO_SHA256: &str = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

    /// FileHash::eq 匹配逻辑
    #[test]
    fn test_file_hash_eq() {
        assert!(FileHash::None.eq("anything"));
        assert!(FileHash::Md5(HELLO_MD5.into()).eq(HELLO_MD5));
        assert!(!FileHash::Md5(HELLO_MD5.into()).eq("wrong"));
        assert!(FileHash::Sha1(HELLO_SHA1.into()).eq(HELLO_SHA1));
        assert!(FileHash::Sha256(HELLO_SHA256.into()).eq(HELLO_SHA256));
        assert!(FileHash::Sha512("abc".into()).eq("abc"));
        // 组合哈希任一匹配即可
        assert!(FileHash::Sha1Sha256(HELLO_SHA1.into(), HELLO_SHA256.into()).eq(HELLO_SHA1));
        assert!(FileHash::Sha1Sha256(HELLO_SHA1.into(), HELLO_SHA256.into()).eq(HELLO_SHA256));
        assert!(!FileHash::Sha1Sha256(HELLO_SHA1.into(), HELLO_SHA256.into()).eq("wrong"));
        assert!(FileHash::Sha1Sha512(HELLO_SHA1.into(), "x".into()).eq(HELLO_SHA1));
    }

    /// get_sha1 只在包含 SHA1 的变体上返回
    #[test]
    fn test_get_sha1() {
        assert_eq!(FileHash::None.get_sha1(), None);
        assert_eq!(FileHash::Md5(HELLO_MD5.into()).get_sha1(), None);
        assert_eq!(FileHash::Sha256(HELLO_SHA256.into()).get_sha1(), None);
        assert_eq!(
            FileHash::Sha1(HELLO_SHA1.into()).get_sha1(),
            Some(HELLO_SHA1.to_string())
        );
        assert_eq!(
            FileHash::Sha1Sha256(HELLO_SHA1.into(), HELLO_SHA256.into()).get_sha1(),
            Some(HELLO_SHA1.to_string())
        );
        assert_eq!(
            FileHash::Sha1Sha512(HELLO_SHA1.into(), "x".into()).get_sha1(),
            Some(HELLO_SHA1.to_string())
        );
    }

    /// 在临时目录创建内容为 "hello world" 的文件
    fn make_file() -> PathBuf {
        let dir = mml_testutil::temp_dir().join(format!(
            "mml_base_file_item_test_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("hello.txt");
        std::fs::write(&file, b"hello world").unwrap();
        file
    }

    /// 构造文件项
    fn make_item(file: PathBuf, hash: FileHash) -> FileItemObj {
        FileItemObj {
            name: "hello.txt".to_string(),
            file,
            url: String::new(),
            hash,
            later: LaterRun::None,
        }
    }

    /// check_hash 校验逻辑
    #[test]
    fn test_check_hash() {
        let file = make_file();

        // 不校验
        assert!(make_item(file.clone(), FileHash::None).check_hash());
        // MD5 正确，大小写不敏感
        assert!(make_item(file.clone(), FileHash::Md5(HELLO_MD5.into())).check_hash());
        assert!(make_item(file.clone(), FileHash::Md5(HELLO_MD5.to_uppercase())).check_hash());
        // MD5 错误
        assert!(!make_item(file.clone(), FileHash::Md5("deadbeef".into())).check_hash());
        // SHA1 / SHA256 正确
        assert!(make_item(file.clone(), FileHash::Sha1(HELLO_SHA1.into())).check_hash());
        assert!(make_item(file.clone(), FileHash::Sha256(HELLO_SHA256.into())).check_hash());
        // 组合哈希两者都对
        assert!(
            make_item(
                file.clone(),
                FileHash::Sha1Sha256(HELLO_SHA1.into(), HELLO_SHA256.into())
            )
            .check_hash()
        );
        // 组合哈希 SHA1 对但 SHA256 错
        assert!(
            !make_item(
                file.clone(),
                FileHash::Sha1Sha256(HELLO_SHA1.into(), "bad".into())
            )
            .check_hash()
        );
        // 组合哈希 SHA1 错
        assert!(
            !make_item(file.clone(), FileHash::Sha1Sha512("bad".into(), "x".into())).check_hash()
        );

        // 文件不存在
        assert!(!make_item(file.join("no_such_file"), FileHash::None).check_hash());

        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    /// LaterRun / FileHash 的 Default 实现
    #[test]
    fn test_defaults() {
        assert!(matches!(FileHash::default(), FileHash::None));
        assert!(matches!(LaterRun::default(), LaterRun::None));
    }
}
