//! 游戏实例日志相关
//! 包括运行日志和过往日志处理
use std::{
    collections::VecDeque,
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, RwLock},
    time::{Duration, SystemTime},
};

use chrono::{DateTime, FixedOffset, Local};
use encoding_rs_io::DecodeReaderBytesBuilder;
use flate2::read::MultiGzDecoder;
use mml_names::names;
use mml_sys::path_helper;
use regex::Regex;

use crate::launcher::{LogEncoding, instance_setting_obj::InstanceSettingObj};

static REGEX_LOG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(.*?)\] \[(.*?)(?:\/(.*?))?\]:? \[(.*?)\](?: (.*))?").unwrap());
static REGEX_LOG_OLD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(.*?)\] \[(.*?)(?:\/(.*?))?\]:?").unwrap());

/// 取一个捕获组；可选组没参与匹配时返回空串
///
/// - `captures`: 日志正则的捕获结果
/// - `index`: 组号
fn group<'a>(captures: &'a regex::Captures<'_>, index: usize) -> &'a str {
    captures.get(index).map_or("", |m| m.as_str())
}

/// 日志级别，由日志行里的级别文本解析得出
#[derive(Clone)]
pub enum LogLevel {
    /// 未识别，解析不出级别时用
    None,
    /// 信息
    Info,
    /// 警告
    Warn,
    /// 错误
    Error,
    /// 调试
    Debug,
}

impl LogLevel {
    /// 级别文本，与原版日志一致
    ///
    /// # 返回值
    ///
    /// 返回级别名；`None` 返回空串
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::None => "",
            LogLevel::Info => "Info",
            LogLevel::Warn => "Warn",
            LogLevel::Error => "Error",
            LogLevel::Debug => "Debug",
        }
    }
}

/// 单条已解析的游戏日志
#[derive(Clone)]
pub struct GameLogObj {
    /// 整行原始日志文本
    pub log: String,
    /// 日志行里的时间戳文本，原样保留
    pub time: String,
    /// 线程名
    pub thread: String,
    /// 解析出的级别
    pub level: LogLevel,
    /// 日志分类，如 `minecraft/advancements`
    pub category: String,
}

/// 日志面板里的一行内容，游戏输出与启动器自己的消息共用
#[derive(Clone)]
pub enum GameLog {
    /// 不匹配日志格式的原始输出
    Text(String),
    /// 解析成结构的游戏日志
    GameLog(GameLogObj),
    /// 运行库输出，值是运行库文件路径
    RuntimeLib(PathBuf),
    /// 日志已重定向到文件
    JavaRedirect,
    /// Java 切换回启动器查找
    JavaLocalRedirect,
    /// 登录用时
    LoginTime(Duration),
    /// 服务器包检查用时
    ServerPackCheckTime(Duration),
    /// 检查游戏文件用时
    CheckGameFileTime(Duration),
    /// 文件下载用时
    DownloadFileTime(Duration),
    /// 启动参数
    LaunchArgs(String),
    /// Java 路径
    JavaPath(PathBuf),
    /// 启动用时
    LaunchTime(Duration),
    /// 启动前执行用时
    CmdPreTime(Duration),
    /// 启动后执行用时
    CmdPostTime(Duration),
}

/// 带时间戳的一条日志
#[derive(Clone)]
pub struct GameLogItemObj {
    /// 入库时间，本地时区带偏移
    pub time: DateTime<FixedOffset>,
    /// 这一行的内容
    pub log: GameLog,
}

/// 游戏运行日志处理
pub struct InstanceRuntimeLog {
    /// 已入库的日志，最多保留 10000 条
    pub logs: RwLock<Arc<VecDeque<GameLogItemObj>>>,
    /// 日志来源文件，从文件读入时才有值
    pub file: Option<PathBuf>,
}

impl InstanceRuntimeLog {
    /// 创建空的日志处理器
    ///
    /// # 返回值
    ///
    /// 返回新建的日志处理器
    pub fn new() -> Self {
        Self {
            logs: RwLock::new(Arc::new(VecDeque::new())),
            file: None,
        }
    }

    /// 从文件读取日志
    ///
    /// - `file`: 日志文件路径，支持 .log / .txt / .log.gz
    /// - `encoding`: 日志编码
    ///
    /// # 返回值
    ///
    /// 返回日志处理器，读取失败时内容为空
    pub fn from_file<P: AsRef<Path>>(file: P, encoding: LogEncoding) -> Self {
        let mut log = Self {
            logs: RwLock::new(Arc::new(VecDeque::new())),
            file: Some(file.as_ref().to_path_buf()),
        };

        if let Ok(stream) = path_helper::open_read(&file) {
            let is_gz = file
                .as_ref()
                .file_name()
                .map(|n| n.to_string_lossy().ends_with(names::LOG_GZ_DOT_EXT))
                .unwrap_or(false);

            let reader: Box<BufReader<dyn Read>> = if is_gz {
                // 必须用 read 版解码器：write 版的 Read 实现直接回吐原始压缩字节
                let gz = MultiGzDecoder::new(stream);
                Box::new(BufReader::new(gz))
            } else {
                Box::new(BufReader::new(stream))
            };

            match encoding {
                LogEncoding::UTF8 => {
                    let reader = BufReader::new(reader);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            log.add_game_log(&line);
                        }
                    }
                }
                LogEncoding::GBK => {
                    let transcoder = DecodeReaderBytesBuilder::new()
                        .encoding(Some(encoding_rs::GBK))
                        .build(reader);
                    let reader = BufReader::new(transcoder);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            log.add_game_log(&line);
                        }
                    }
                }
            }
        }

        log
    }

    /// 清空所有日志
    pub fn clear(&mut self) {
        let mut logs = self.logs.write().unwrap();
        *logs = Arc::new(VecDeque::new());
    }

    /// 添加游戏输出的日志
    ///
    /// - `log`: 单行日志文本
    ///
    /// # 返回值
    ///
    /// 返回解析并入库的日志项
    pub fn add_game_log(&mut self, log: &str) -> GameLogItemObj {
        let temp = log.trim();
        let obj = if let Some(captures) = REGEX_LOG.captures(temp)
            && captures.len() == 6
        {
            GameLog::GameLog(GameLogObj {
                level: get_level(group(&captures, 3)),
                log: String::from(log),
                thread: String::from(group(&captures, 2)),
                category: String::from(group(&captures, 4)),
                time: String::from(group(&captures, 1)),
            })
        } else if let Some(captures) = REGEX_LOG_OLD.captures(temp)
            && captures.len() == 4
        {
            GameLog::GameLog(GameLogObj {
                level: get_level(group(&captures, 3)),
                log: String::from(log),
                thread: String::from(group(&captures, 2)),
                category: Default::default(),
                time: String::from(group(&captures, 1)),
            })
        } else {
            GameLog::Text(String::from(log))
        };

        self.add_log_item(obj)
    }

    /// 解析单行游戏日志
    ///
    /// 与 [`InstanceRuntimeLog::add_game_log`] 同一套正则；解析不出的行
    /// thread / category / time 为空、级别为 `None`。GUI 据此填充筛选字段。
    ///
    /// - `log`: 单行日志文本
    ///
    /// # 返回值
    ///
    /// 返回解析出的日志对象
    pub fn parse_game_log_line(log: &str) -> GameLogObj {
        let temp = log.trim();
        if let Some(captures) = REGEX_LOG.captures(temp)
            && captures.len() == 6
        {
            GameLogObj {
                level: get_level(group(&captures, 3)),
                log: String::from(log),
                thread: String::from(group(&captures, 2)),
                category: String::from(group(&captures, 4)),
                time: String::from(group(&captures, 1)),
            }
        } else if let Some(captures) = REGEX_LOG_OLD.captures(temp)
            && captures.len() == 4
        {
            GameLogObj {
                level: get_level(group(&captures, 3)),
                log: String::from(log),
                thread: String::from(group(&captures, 2)),
                category: Default::default(),
                time: String::from(group(&captures, 1)),
            }
        } else {
            GameLogObj {
                level: LogLevel::None,
                log: String::from(log),
                thread: Default::default(),
                category: Default::default(),
                time: Default::default(),
            }
        }
    }

    /// 添加日志
    ///
    /// - `log`: 日志内容
    ///
    /// # 返回值
    ///
    /// 返回入库的日志项
    pub fn add_log_item(&mut self, log: GameLog) -> GameLogItemObj {
        let mut logs = self.logs.write().unwrap();
        let logs = Arc::make_mut(&mut logs);
        let item = GameLogItemObj {
            time: Local::now().fixed_offset(),
            log: log,
        };

        if logs.len() > 10000 {
            logs.pop_front();
        }
        logs.push_back(item.clone());

        item
    }
}

/// 按日志文本解析级别
///
/// - `level`: 日志级别文本，大小写不敏感
///
/// # 返回值
///
/// 返回对应的日志级别；无法识别返回 `None`
fn get_level(level: &str) -> LogLevel {
    match level.to_lowercase().as_str() {
        "info" => LogLevel::Info,
        "warn" => LogLevel::Warn,
        "error" => LogLevel::Error,
        "fatal" => LogLevel::Error,
        "debug" => LogLevel::Debug,
        _ => LogLevel::None,
    }
}

impl InstanceSettingObj {
    /// 获取游戏日志文件列表
    ///
    /// # 返回值
    ///
    /// 返回日志目录与崩溃报告目录下的日志文件列表
    pub fn get_log_files(&self) -> Vec<PathBuf> {
        /// 判断是不是日志文件，含 .log / .txt / .log.gz
        fn is_log_file(path: &Path) -> bool {
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case(names::LOG_EXT)
                    || ext.eq_ignore_ascii_case(names::TXT_EXT)
                {
                    return true;
                }
            }
            path.file_name()
                .map(|n| n.to_string_lossy().ends_with(names::LOG_GZ_DOT_EXT))
                .unwrap_or(false)
        }

        let mut list = Vec::new();
        let dir = self.get_logs_path();
        if dir.exists() && dir.is_dir() {
            for item in path_helper::get_all_files(dir).iter() {
                if is_log_file(item) {
                    list.push(item.clone());
                }
            }
        }

        let dir = self.get_crash_path();
        if dir.exists() && dir.is_dir() {
            for item in path_helper::get_all_files(dir).iter() {
                if is_log_file(item) {
                    list.push(item.clone());
                }
            }
        }

        list
    }

    /// 获取最新的崩溃日志
    ///
    /// - `sec`: 与最新修改时间的最大差值，单位秒；不传时按 5 秒算
    ///
    /// # 返回值
    ///
    /// 返回最新的崩溃日志文件；目录不存在或文件太旧返回 `None`
    pub fn get_last_crash_report(&self, sec: Option<u32>) -> Option<PathBuf> {
        let dir = self.get_crash_path();
        if !dir.exists() || !dir.is_dir() {
            return None;
        }

        let sec = sec.unwrap_or(5);
        let files = path_helper::get_last_written_file(dir);
        if let Ok(Some(path)) = files
            && let Ok(meta) = fs::metadata(&path)
            && let Ok(time) = meta.modified()
        {
            let now = SystemTime::now();

            let duration = now.duration_since(time);
            if let Ok(duration) = duration {
                if duration.as_secs_f64() > sec as f64 {
                    return None;
                } else {
                    return Some(path.clone());
                }
            }
        }

        None
    }

    /// 读取日志
    ///
    /// - `path`: 相对日志目录的文件名
    ///
    /// # 返回值
    ///
    /// 返回日志处理器；文件不存在返回 `None`
    pub fn read_log<P: AsRef<Path>>(&self, path: P) -> Option<InstanceRuntimeLog> {
        let path = self.get_logs_path().join(path.as_ref());

        if path.exists() && path.is_file() {
            Some(InstanceRuntimeLog::from_file(path, self.encoding))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// .log.gz 要先解压再逐行解析：把压缩字节当文本读时一行都解析不出来
    #[test]
    fn reads_gzip_log() {
        let dir = mml_testutil::temp_dir().join(format!("mml-log-gz-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("2026-01-01-1.log.gz");
        let file = fs::File::create(&path).unwrap();
        let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        encoder
            .write_all(b"[12:00:00] [main/INFO]: hello gz\n")
            .unwrap();
        encoder.finish().unwrap();

        let log = InstanceRuntimeLog::from_file(&path, LogEncoding::UTF8);
        let logs = log.logs.read().unwrap();
        assert_eq!(logs.len(), 1);
        // 存的是整行原文，解析成哪种变体不重要；关键是不能是压缩字节
        let text = match &logs[0].log {
            GameLog::GameLog(obj) => &obj.log,
            GameLog::Text(s) => s,
            _ => panic!("gzip 日志不应解析成启动器消息"),
        };
        assert_eq!(text, "[12:00:00] [main/INFO]: hello gz");

        drop(logs);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 没有 `/级别` 的行：可选组不参与匹配，按下标取会 panic
    #[test]
    fn parses_line_without_level() {
        let line = "[12:00:00] [main]: hello";
        let parsed = InstanceRuntimeLog::parse_game_log_line(line);
        assert_eq!(parsed.thread, "main");
        assert_eq!(parsed.time, "12:00:00");
        assert_eq!(parsed.log, line);
        assert!(matches!(parsed.level, LogLevel::None));
    }
}
