//! Minecraft 服务器 MOTD 查询，走 Server List Ping
//!
//! 实现：TCP 连上后发握手包，next_state=1，再发 status request，
//! 读回 JSON 解析出版本 / 在线人数 / 描述 / 图标。
//! 直连失败且主机名不是 IP 字面量时，用 SRV 记录 `_minecraft._tcp.<主机名>`
//! 解析出真实地址重试。

use std::{
    net::IpAddr,
    time::{Duration, Instant},
};

use serde::Deserialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

use mml_names::{
    i18,
    i18_items::error_type::{CoreResult, ErrorType},
};
use crate::error::{serializer_err, socket_err};

/// 连接与读包的超时
const TIMEOUT: Duration = Duration::from_secs(5);

/// 握手用的协议版本，1.16.5 即 754；只影响握手包内容，不影响查询结果
const PROTOCOL_VERSION: i32 = 754;

/// 一次查询的结果状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MotdState {
    /// 查询成功
    #[default]
    Ok,
    /// 连上了但没读到数据
    NoData,
    /// 连接失败，含 SRV 解析失败
    ConnectFail,
    /// 查询过程出错
    Error,
}

/// 服务器 description 的 Chat JSON 结构
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ChatObj {
    /// 这段的文字内容
    pub text: String,
    /// 是否加粗，缺省表示继承父段落
    pub bold: Option<bool>,
    /// 是否斜体，缺省表示继承父段落
    pub italic: Option<bool>,
    /// 是否下划线，缺省表示继承父段落
    pub underlined: Option<bool>,
    /// 是否删除线，缺省表示继承父段落
    pub strikethrough: Option<bool>,
    /// 是否混淆字符，缺省表示继承父段落
    pub obfuscated: Option<bool>,
    /// 颜色名如 `gold`，或 #RRGGBB
    pub color: Option<String>,
    /// 子组件，按顺序拼在后面
    pub extra: Option<Vec<ChatObj>>,
}

/// 服务器版本信息
#[derive(Debug, Clone, Deserialize)]
pub struct ServerVersionObj {
    /// 版本名，如 `1.21.1`
    pub name: String,
    /// 协议号
    pub protocol: i32,
}

/// 在线玩家信息
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ServerPlayersObj {
    /// 人数上限
    pub max: i32,
    /// 当前在线人数
    pub online: i32,
    /// 在线玩家样例，服务器可以不提供
    pub sample: Option<Vec<ServerPlayerObj>>,
}

/// 在线玩家样例里的一项
#[derive(Debug, Clone, Deserialize)]
pub struct ServerPlayerObj {
    /// 玩家名
    pub name: String,
    /// 玩家 uuid 的字符串形式
    pub id: String,
}

/// status 响应 JSON，只取关心的字段
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ServerStatusObj {
    /// 版本信息
    pub version: Option<ServerVersionObj>,
    /// 在线玩家信息
    pub players: Option<ServerPlayersObj>,
    /// description，可能是纯字符串，也可能是 Chat JSON
    pub description: Option<serde_json::Value>,
    /// favicon，`data:image/png;base64,` 前缀的 PNG
    pub favicon: Option<String>,
}

/// 展平后的一段 MOTD 文字，color 已转成 #RRGGBB，供前端直接渲染
#[derive(Debug, Clone)]
pub struct ChatSegment {
    /// 文字内容
    pub text: String,
    /// 颜色，空串表示没指定、由前端继承
    pub color: String,
    /// 是否加粗
    pub bold: bool,
    /// 是否斜体
    pub italic: bool,
    /// 是否下划线
    pub underlined: bool,
    /// 是否删除线
    pub strikethrough: bool,
}

/// 服务器查询结果
#[derive(Debug, Clone)]
pub struct ServerMotdObj {
    /// 实际查询的地址，SRV 重定向后与传入值不同
    pub ip: String,
    /// 实际查询的端口
    pub port: u16,
    /// 查询状态
    pub state: MotdState,
    /// 出错时的错误信息
    pub message: String,
    /// 延迟，单位毫秒，从发起连接到读到响应
    pub ping: u64,
    /// 服务器版本信息
    pub version: Option<ServerVersionObj>,
    /// 在线玩家信息
    pub players: Option<ServerPlayersObj>,
    /// 解析出的 description
    pub description: Option<ChatObj>,
    /// favicon 原文
    pub favicon: Option<String>,
}

impl ServerMotdObj {
    /// 建一个初始结果：地址按传入值、状态为成功
    fn new(ip: &str, port: u16) -> Self {
        Self {
            ip: ip.to_string(),
            port,
            state: MotdState::Ok,
            message: String::new(),
            ping: 0,
            version: None,
            players: None,
            description: None,
            favicon: None,
        }
    }
}

/// 查询服务器信息，不抛异常，失败落在 `state` / `message` 里
///
/// - `ip`: 服务器地址，可以是域名或 IP 字面量
/// - `port`: 服务器端口
///
/// # 返回值
///
/// 返回查询结果；失败时 `state` 非 `Ok`，`message` 是本地化错误文本
pub async fn get_server_info(ip: &str, port: u16) -> ServerMotdObj {
    let start = Instant::now();
    let mut info = ServerMotdObj::new(ip, port);

    // 直连；失败且主机名不是 IP 字面量时走 SRV 重定向
    let stream = match connect(&info.ip, info.port).await {
        Ok(s) => s,
        Err(e) if is_host_name(&info.ip) => match srv_lookup(&info.ip).await {
            Some(target) => match connect(&target.host, target.port).await {
                Ok(s) => {
                    info.ip = target.host;
                    info.port = target.port;
                    s
                }
                Err(e2) => {
                    info.state = MotdState::ConnectFail;
                    info.message = i18::get_error(e2);
                    return info;
                }
            },
            None => {
                info.state = MotdState::ConnectFail;
                info.message = i18::get_error(e);
                return info;
            }
        },
        Err(e) => {
            info.state = MotdState::ConnectFail;
            info.message = i18::get_error(e);
            return info;
        }
    };

    match query_status(stream, &info.ip, info.port).await {
        Ok(status) => {
            info.ping = start.elapsed().as_millis() as u64;
            info.version = status.version;
            info.players = status.players;
            info.favicon = status.favicon;
            if let Some(desc) = status.description {
                info.description = Some(chat_from_value(&desc));
            }
        }
        Err(e) => {
            info.state = MotdState::Error;
            info.message = i18::get_error(e);
        }
    }
    info
}

/// 主机名是否 IP 字面量，IP 直连不走 SRV
fn is_host_name(ip: &str) -> bool {
    ip.parse::<IpAddr>().is_err() && !ip.is_empty()
}

/// 连接服务器，带超时
async fn connect(ip: &str, port: u16) -> CoreResult<TcpStream> {
    timeout(TIMEOUT, TcpStream::connect((ip, port)))
        .await
        .map_err(|_| ErrorType::MotdConnectTimeout {
            ip: ip.to_string(),
            port,
        })?
        .map_err(|err| ErrorType::MotdConnectFailed {
            ip: ip.to_string(),
            port,
            error: err.to_string(),
        })
}

/// 握手 + status request + 读响应
async fn query_status(mut stream: TcpStream, ip: &str, port: u16) -> CoreResult<ServerStatusObj> {
    let mut host_buf = Vec::with_capacity(ip.len() + 5);
    write_varint(&mut host_buf, 0x00); // packet id: handshake
    write_varint(&mut host_buf, PROTOCOL_VERSION);
    write_varint(&mut host_buf, ip.len() as i32);
    host_buf.extend_from_slice(ip.as_bytes());
    host_buf.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut host_buf, 1); // next state: status

    let mut buf = Vec::with_capacity(host_buf.len() + 5);
    write_varint(&mut buf, host_buf.len() as i32);
    buf.extend_from_slice(&host_buf);

    let req = vec![1, 0]; // 帧长 1 + packet id: status request

    async {
        stream.write_all(&buf).await.map_err(socket_err)?;
        stream.write_all(&req).await.map_err(socket_err)?;
        stream.flush().await.map_err(socket_err)?;

        let len = read_varint(&mut stream).await?;
        if len <= 0 {
            return Err(ErrorType::MotdBadResponseLen);
        }
        let mut data = vec![0u8; len as usize];
        stream.read_exact(&mut data).await.map_err(socket_err)?;

        let mut pos = 0;
        let packet_id = read_varint_from(&data, &mut pos);
        if packet_id != 0 {
            return Err(ErrorType::MotdBadPacketId { packet_id });
        }
        let str_len = read_varint_from(&data, &mut pos) as usize;
        if pos + str_len > data.len() {
            return Err(ErrorType::MotdResponseIncomplete);
        }
        let json = std::str::from_utf8(&data[pos..pos + str_len]).map_err(serializer_err)?;
        serde_json::from_str(json).map_err(serializer_err)
    }
    .await
}

/// varint 编码
fn write_varint(buf: &mut Vec<u8>, mut v: i32) {
    loop {
        if v & -128 == 0 {
            buf.push(v as u8);
            return;
        }
        buf.push((v & 0x7F | 128) as u8);
        v = ((v as u32) >> 7) as i32;
    }
}

/// 从流上读 varint
async fn read_varint(stream: &mut TcpStream) -> CoreResult<i32> {
    let mut result = 0i32;
    let mut shift = 0;
    loop {
        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte).await.map_err(socket_err)?;
        result |= ((byte[0] & 0x7F) as i32) << shift;
        if byte[0] & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 32 {
            return Err(ErrorType::MotdVarintTooBig);
        }
    }
}

/// 从缓存读 varint，pos 随之前移
fn read_varint_from(data: &[u8], pos: &mut usize) -> i32 {
    let mut result = 0i32;
    let mut shift = 0;
    while *pos < data.len() {
        let byte = data[*pos];
        *pos += 1;
        result |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 32 {
            break;
        }
    }
    result
}

/// SRV 记录解析出的真实服务器地址
struct SrvTarget {
    /// 目标主机名，已去掉域名结尾的根点
    host: String,
    /// 目标端口，取值 1..=65535
    port: u16,
}

/// SRV 记录解析：`_minecraft._tcp.<主机名>` → 真实地址 + 端口
///
/// - `host`: 待解析的主机名，不含端口
///
/// # 返回值
///
/// 见 [`SrvTarget`]；没有 SRV 记录、目标为空或解析失败时返回 `None`
async fn srv_lookup(host: &str) -> Option<SrvTarget> {
    use hickory_resolver::{Resolver, proto::rr::RData};

    let resolver = Resolver::builder_tokio().ok()?.build().ok()?;
    let name = format!("_minecraft._tcp.{host}.");
    let answer = timeout(TIMEOUT, resolver.srv_lookup(name))
        .await
        .ok()?
        .ok()?;
    let srv = answer.answers().iter().find_map(|r| match &r.data {
        RData::SRV(srv) => Some(srv),
        _ => None,
    })?;
    let target = srv.target.to_string();
    // 去掉结尾根点
    let target = target.strip_suffix('.').unwrap_or(&target).to_string();
    if target.is_empty() {
        return None;
    }
    Some(SrvTarget {
        host: target,
        port: srv.port,
    })
}

/// description 解析：按**形状**取值，各种写法都收得下
///
/// 顶层三种形态，真实服务器上都出现过：
/// - **纯字符串** —— 旧写法，按 `§` 颜色码切段，见 [`chat_from_plain`]；
/// - **对象** —— Chat JSON；
/// - **数组** —— 整段就是一串子组件。
///
/// 不能整份交给 serde：`extra` 里混了裸字符串就会整体失败、退化成空组件，
/// 表现为 MOTD 一个字都不显示而人数照常，真实样本是 2b2t 的 status 响应。
/// 现在逐层按形状走，认不出的部分最多丢那一小段。
fn chat_from_value(value: &serde_json::Value) -> ChatObj {
    // 顶层纯字符串是"旧写法"：里面的 `§` 颜色码要认
    if let Some(text) = value.as_str() {
        return chat_from_plain(text);
    }

    chat_from_component(value)
}

/// 组件 → Chat，递归
///
/// 与顶层那条的区别：**这里的字符串是字面量**，不再解释 `§` 颜色码 ——
/// 组件树里的字符串只是文字，游戏也是这么处理的，`§` 只在顶层纯字符串那一支生效。
fn chat_from_component(value: &serde_json::Value) -> ChatObj {
    match value {
        serde_json::Value::String(text) => ChatObj {
            text: text.clone(),
            ..Default::default()
        },
        serde_json::Value::Array(list) => ChatObj {
            extra: Some(list.iter().map(chat_from_component).collect()),
            ..Default::default()
        },
        serde_json::Value::Object(map) => chat_from_object(map),
        _ => ChatObj::default(),
    }
}

/// 组件对象 → Chat
///
/// 逐字段取、不认识的字段直接忽略，比"整份交给 serde"宽容，理由见 [`chat_from_value`]。
///
/// `translate` 这类要查语言表的组件启动器翻不出来，手里没有客户端语言表，
/// 按资源包简介那套口径退：有 `fallback` 用它，没有就把 key 原样显示出来。
fn chat_from_object(map: &serde_json::Map<String, serde_json::Value>) -> ChatObj {
    let flag = |key: &str| map.get(key).and_then(|value| value.as_bool());
    let string = |key: &str| {
        map.get(key)
            .and_then(|value| value.as_str())
            .map(str::to_string)
    };

    let text = match string("text") {
        Some(text) => text,
        None => match string("translate") {
            Some(key) => string("fallback").unwrap_or(key),
            None => string("fallback").unwrap_or_default(),
        },
    };

    ChatObj {
        text,
        color: string("color"),
        bold: flag("bold"),
        italic: flag("italic"),
        underlined: flag("underlined"),
        strikethrough: flag("strikethrough"),
        obfuscated: flag("obfuscated"),
        extra: map
            .get("extra")
            .and_then(|value| value.as_array())
            .map(|list| list.iter().map(chat_from_component).collect()),
    }
}

/// 纯字符串带 § 颜色码时转 Chat，算法同 ColorMC 的 StringToChar：
/// 切成带样式的段落，换行保留在文字里
fn chat_from_plain(text: &str) -> ChatObj {
    let mut root = ChatObj {
        text: String::new(),
        extra: Some(Vec::new()),
        ..Default::default()
    };
    let extra = root.extra.as_mut().unwrap();

    let mut color: Option<String> = None;
    let mut bold = false;
    let mut italic = false;
    let mut underlined = false;
    let mut strikethrough = false;
    let mut current = String::new();

    let mut chars = text.chars().peekable();
    let flush = |current: &mut String,
                 extra: &mut Vec<ChatObj>,
                 color: &Option<String>,
                 bold: bool,
                 italic: bool,
                 underlined: bool,
                 strikethrough: bool| {
        if !current.is_empty() {
            extra.push(ChatObj {
                text: std::mem::take(current),
                color: color.clone(),
                bold: Some(bold),
                italic: Some(italic),
                underlined: Some(underlined),
                strikethrough: Some(strikethrough),
                obfuscated: None,
                extra: None,
            });
        }
    };

    while let Some(c) = chars.next() {
        if c != '§' {
            current.push(c);
            continue;
        }
        let Some(code) = chars.next() else { break };
        if code == 'r' {
            // 重置全部样式
            flush(
                &mut current,
                extra,
                &color,
                bold,
                italic,
                underlined,
                strikethrough,
            );
            color = None;
            bold = false;
            italic = false;
            underlined = false;
            strikethrough = false;
            continue;
        }
        if let Some(hex) = code_color(code) {
            flush(
                &mut current,
                extra,
                &color,
                bold,
                italic,
                underlined,
                strikethrough,
            );
            color = Some(hex.to_string());
            // 颜色码会重置字体样式
            bold = false;
            italic = false;
            underlined = false;
            strikethrough = false;
            continue;
        }
        flush(
            &mut current,
            extra,
            &color,
            bold,
            italic,
            underlined,
            strikethrough,
        );
        match code {
            'l' | 'L' => bold = true,
            'o' | 'O' => italic = true,
            'n' | 'N' => underlined = true,
            'm' | 'M' => strikethrough = true,
            'k' | 'K' => {} // 混淆字符按普通文字展示
            _ => current.push('§'),
        }
        if !matches!(
            code,
            'l' | 'L' | 'o' | 'O' | 'n' | 'N' | 'm' | 'M' | 'k' | 'K'
        ) {
            current.push(code);
        }
    }
    flush(
        &mut current,
        extra,
        &color,
        bold,
        italic,
        underlined,
        strikethrough,
    );
    root
}

/// § 颜色码 → #RRGGBB
fn code_color(code: char) -> Option<&'static str> {
    Some(match code {
        '0' => "#000000",
        '1' => "#0000AA",
        '2' => "#00AA00",
        '3' => "#00AAAA",
        '4' => "#AA0000",
        '5' => "#AA00AA",
        '6' => "#FFAA00",
        '7' => "#AAAAAA",
        '8' => "#555555",
        '9' => "#5555FF",
        'a' | 'A' => "#55FF55",
        'b' | 'B' => "#55FFFF",
        'c' | 'C' => "#FF5555",
        'd' | 'D' => "#FF55FF",
        'e' | 'E' => "#FFFF55",
        'f' | 'F' => "#FFFFFF",
        _ => return None,
    })
}

/// 把 Chat 树展平成可直接渲染的文字段，子段落继承父段落的样式
///
/// 起始颜色用**空串**表示"没指定颜色"，而不是 `#FFFFFF`：MOTD 里没写颜色码时前端应当
/// **继承所在处的文字色**，写死白色会让浅色主题下这段文字白底白字看不见。
/// 空串这个约定由前端的 `lib/motd.ts::motdSegStyle` 处理：为空则不写 `color`。
///
/// - `chat`: 解析出的 description
///
/// # 返回值
///
/// 返回按顺序排列的文字段，可直接交给前端渲染
pub fn chat_to_segments(chat: &ChatObj) -> Vec<ChatSegment> {
    let mut out = Vec::new();
    flatten_chat(chat, "", false, false, false, false, &mut out);
    out
}

/// 展平的递归实现，样式逐层向下继承
#[allow(clippy::too_many_arguments)]
fn flatten_chat(
    chat: &ChatObj,
    color: &str,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
    out: &mut Vec<ChatSegment>,
) {
    let color = chat
        .color
        .as_deref()
        .map_or(color, |c| color_hex(c).unwrap_or(c));
    let bold = chat.bold.unwrap_or(bold);
    let italic = chat.italic.unwrap_or(italic);
    let underlined = chat.underlined.unwrap_or(underlined);
    let strikethrough = chat.strikethrough.unwrap_or(strikethrough);

    if !chat.text.is_empty() {
        out.push(ChatSegment {
            text: chat.text.clone(),
            color: color.to_string(),
            bold,
            italic,
            underlined,
            strikethrough,
        });
    }
    if let Some(extra) = &chat.extra {
        for child in extra {
            flatten_chat(child, color, bold, italic, underlined, strikethrough, out);
        }
    }
}

/// 颜色名 → #RRGGBB；已是 # 开头的原样返回
fn color_hex(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "black" => "#000000",
        "dark_blue" => "#0000AA",
        "dark_green" => "#00AA00",
        "dark_aqua" => "#00AAAA",
        "dark_red" => "#AA0000",
        "dark_purple" => "#AA00AA",
        "gold" => "#FFAA00",
        "gray" => "#AAAAAA",
        "dark_gray" => "#555555",
        "blue" => "#5555FF",
        "green" => "#55FF55",
        "aqua" => "#55FFFF",
        "red" => "#FF5555",
        "light_purple" => "#FF55FF",
        "yellow" => "#FFFF55",
        "white" => "#FFFFFF",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// varint 编码后再解回来应与原值一致
    #[test]
    fn test_write_read_varint() {
        for v in [0, 1, 127, 128, 255, 754, 65535, 0x7FFFFFFF] {
            let mut buf = Vec::new();
            write_varint(&mut buf, v);
            let mut pos = 0;
            assert_eq!(read_varint_from(&buf, &mut pos), v);
            assert_eq!(pos, buf.len());
        }
    }

    /// § 颜色码切段：文字、颜色与样式都要对上
    #[test]
    fn test_chat_from_plain() {
        let chat = chat_from_plain("§aHello§r world\n§lBold");
        let segs = chat_to_segments(&chat);
        let text: String = segs.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "Hello world\nBold");
        let first = &segs[0];
        assert_eq!(first.color, "#55FF55");
        assert!(segs.iter().any(|s| s.bold));
    }

    /// 对象形态：子段落继承父段落颜色，也能覆盖加粗
    #[test]
    fn test_chat_from_value_object() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"text":"A","color":"gold","extra":[{"text":"B","bold":true}]}"#,
        )
        .unwrap();
        let chat = chat_from_value(&v);
        let segs = chat_to_segments(&chat);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].color, "#FFAA00");
        // 子段落继承颜色，覆盖加粗
        assert_eq!(segs[1].color, "#FFAA00");
        assert!(segs[1].bold);
    }

    /// `extra` 里混**裸字符串**
    ///
    /// 真实样本就是 2b2t 的 `{"text":"","extra":[{"text":"2B "},"\n",{"text":"2T "}, …]}`：
    /// 以前整份交给 serde，遇到字符串元素就整体失败、`unwrap_or_default()` 成空组件 ——
    /// MOTD 一个字都不显示，而同一张卡片上人数照常，看着就是"连不上但有人数"。
    #[test]
    fn test_chat_from_value_extra_with_plain_strings() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"text":"","extra":[{"bold":true,"color":"gray","text":"2B "},{"color":"gold","text":"Updated"},"\n",{"color":"green","text":"2b2t.org"}]}"#,
        )
        .unwrap();

        let segs = chat_to_segments(&chat_from_value(&v));
        let text: String = segs.iter().map(|s| s.text.as_str()).collect();

        assert_eq!(text, "2B Updated\n2b2t.org");
        // 颜色名照旧翻译，子段落继承父段落的样式
        assert!(segs.iter().any(|s| s.color == "#AAAAAA"));
        assert!(segs.iter().any(|s| s.bold));
    }

    /// 顶层写成**数组**：整段就是一串子组件，数组里的裸字符串是字面量
    #[test]
    fn test_chat_from_value_array() {
        let v: serde_json::Value =
            serde_json::from_str(r#"[{"text":"A","color":"red"},"B",{"text":"C"}]"#).unwrap();

        let segs = chat_to_segments(&chat_from_value(&v));
        let text: String = segs.iter().map(|s| s.text.as_str()).collect();

        assert_eq!(text, "ABC");
        assert_eq!(segs[0].color, "#FF5555");
    }

    /// `translate`：没有 `text` 时退到 `fallback`，连 `fallback` 都没有就把 key 原样显示
    #[test]
    fn test_chat_from_value_translate() {
        let flatten = |json: &str| -> String {
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            chat_to_segments(&chat_from_value(&v))
                .iter()
                .map(|s| s.text.as_str())
                .collect()
        };

        assert_eq!(
            flatten(r#"{"translate":"some.key","fallback":"兜底文字"}"#),
            "兜底文字"
        );
        assert_eq!(flatten(r#"{"translate":"some.key"}"#), "some.key");
    }
}
