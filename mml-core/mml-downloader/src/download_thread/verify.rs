//! 下载内容的校验
//!
//! 把读流出错统一转成 `StreamError`，并支持 MD5、SHA1、SHA256、SHA512
//! 以及 SHA1+SHA256、SHA1+SHA512 组合校验。

use super::*;

/// 读流出错转 StreamError
///
/// - `err`: 底层读流错误
///
/// # 返回值
///
/// 包装后的 `ErrorType::StreamError`
pub(crate) fn stream_err(err: impl std::fmt::Display) -> ErrorType {
    ErrorType::StreamError(ErrorData::from_error(err))
}

/// 文件哈希不匹配转 DownloadFileHashError
///
/// - `file`: 被校验的文件路径
/// - `now`: 实际算出的哈希值
/// - `hash`: 期望的哈希值
///
/// # 返回值
///
/// 包装后的 `ErrorType::DownloadFileHashError`
fn hash_error(file: PathBuf, now: String, hash: String) -> ErrorType {
    ErrorType::DownloadFileHashError(DownloadFileHashErrorData { file, now, hash })
}

/// 校验文件哈希
///
/// 支持 MD5、SHA1、SHA256、SHA512 及组合校验：SHA1+SHA256、SHA1+SHA512，
/// 比较时忽略大小写。
///
/// - `file`: 文件路径
/// - `hash`: 期望的哈希值
/// - `stream`: 文件读取流
///
/// # 返回值
///
/// 校验通过返回 `Ok(())`；不匹配或哈希计算失败时返回相应错误
pub(crate) fn check_hash<R: Read + Seek>(
    file: &PathBuf,
    hash: &FileHash,
    stream: &mut R,
) -> CoreResult<()> {
    // 期望值全空 = 上游没公布哈希（Fabric 的 loader jar 就是这样）：不校验，
    // 否则每次下载都判失败、反复重下，最后整个文件检查以 DownloadFileFail 收场
    if hash.is_empty() {
        return Ok(());
    }

    match hash {
        FileHash::None => Ok(()),
        FileHash::Md5(md5) => match hash_helper::gen_hash_from_reader(HashType::Md5, stream) {
            Ok(hash) => {
                if hash.eq_ignore_ascii_case(md5) {
                    Ok(())
                } else {
                    Err(hash_error(file.clone(), hash.clone(), md5.clone()))
                }
            }
            Err(err) => Err(err),
        },
        FileHash::Sha1(sha1) => match hash_helper::gen_hash_from_reader(HashType::Sha1, stream) {
            Ok(hash) => {
                if hash.eq_ignore_ascii_case(sha1) {
                    Ok(())
                } else {
                    Err(hash_error(file.clone(), hash.clone(), sha1.clone()))
                }
            }
            Err(err) => Err(err),
        },
        FileHash::Sha256(sha256) => {
            match hash_helper::gen_hash_from_reader(HashType::Sha256, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha256) {
                        Ok(())
                    } else {
                        Err(hash_error(file.clone(), hash.clone(), sha256.clone()))
                    }
                }
                Err(err) => Err(err),
            }
        }
        FileHash::Sha1Sha256(sha1, sha256) => {
            let sha1 = match hash_helper::gen_hash_from_reader(HashType::Sha1, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha1) {
                        Ok(())
                    } else {
                        Err(hash_error(file.clone(), hash.clone(), sha1.clone()))
                    }
                }
                Err(err) => Err(err),
            };

            if sha1.is_err() {
                return sha1;
            }

            stream
                .seek(SeekFrom::Start(0))
                .map_err(|err| stream_err(err))?;

            let sha256 = match hash_helper::gen_hash_from_reader(HashType::Sha256, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha256) {
                        Ok(())
                    } else {
                        Err(hash_error(file.clone(), hash.clone(), sha256.clone()))
                    }
                }
                Err(err) => Err(err),
            };

            if sha256.is_err() {
                return sha256;
            }

            Ok(())
        }
        FileHash::Sha512(sha512) => {
            match hash_helper::gen_hash_from_reader(HashType::Sha512, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha512) {
                        Ok(())
                    } else {
                        Err(hash_error(file.clone(), hash.clone(), sha512.clone()))
                    }
                }
                Err(err) => Err(err),
            }
        }
        FileHash::Sha1Sha512(sha1, sha512) => {
            let sha1 = match hash_helper::gen_hash_from_reader(HashType::Sha1, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha1) {
                        Ok(())
                    } else {
                        Err(hash_error(file.clone(), hash.clone(), sha1.clone()))
                    }
                }
                Err(err) => Err(err),
            };

            if sha1.is_err() {
                return sha1;
            }

            stream
                .seek(SeekFrom::Start(0))
                .map_err(|err| stream_err(err))?;

            let sha512 = match hash_helper::gen_hash_from_reader(HashType::Sha512, stream) {
                Ok(hash) => {
                    if hash.eq_ignore_ascii_case(sha512) {
                        Ok(())
                    } else {
                        Err(hash_error(file.clone(), hash.clone(), sha512.clone()))
                    }
                }
                Err(err) => Err(err),
            };

            if sha512.is_err() {
                return sha512;
            }

            Ok(())
        }
    }
}
