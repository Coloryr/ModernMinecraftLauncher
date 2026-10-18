// 打开过的 NBT 文件：树、SHA1 与未保存的改动都留在这一份会话里
//
// 切到别的文件再切回来不该重新读盘 —— 那等于把文件关掉再打开一遍，还没落盘的改动也跟着没了。
// 会话按「实例 + 来源 + 文件 + 区块」存，活到窗口的 JS 环境结束：多窗口模式就是这个窗口关掉，
// 单窗口模式是整个应用退出。
//
// 后端另有一份"真的打开着"的会话（句柄 + 解析结果，见 windows/nbt_session.rs）：这边每扔掉
// 一份会话，都要顺手把它关掉，否则那个文件句柄会一直留着。两边的键与生命周期因此必须一致。
//
// 清的理由只有两种：盘上那一份可能已经变了（刷新 / 换来源 / 换实例），或者会话留太多。
// 两种情况都**不扔有未保存改动的那些** —— 那是用户还没落盘的东西。
import type { NbtEditDto } from "../../../lib/bindings";
import { closeConfigNbt } from "../../../lib/api";
import type { NbtTreeNode } from "./nbtEdit";

/** 一个打开着的 NBT 文件 */
export interface NbtSession {
  /** 实例 uuid，收尾时要按它关掉后端那份会话 */
  uuid: string;
  /** 来源，空串为实例本身 */
  source: string;
  /** 来源内的相对路径 */
  file: string;
  /** 区域文件里的区块表项下标；普通 NBT 文件为 null */
  chunk: number | null;
  /** 工作树；还没读到就是 null */
  tree: NbtTreeNode | null;
  /** 打开时那一份文件字节的 SHA1，保存时交给后端核对 */
  sha1: string;
  /** 待保存的改动，按发生顺序；路径相对"当时"的树 */
  ops: NbtEditDto[];
  /** 有没有未保存的改动 */
  dirty: boolean;
  /** 已经从盘上读到过内容 */
  loaded: boolean;
  /** 手动定过的展开状态，键见 nbtEdit 的 `pathKey`；没记过的按默认档（前两层展开） */
  expanded: Map<string, boolean>;
}

/** 同时留着的文件上限；超出先扔最早打开、且没有未保存改动的那一份 */
const MAX_SESSIONS = 8;

/** 会话表，键见 [`sessionKey`]；Map 的顺序就是打开顺序 */
const sessions = new Map<string, NbtSession>();

/**
 * 会话的键：同一个文件在不同实例 / 来源下是两份，区域文件里不同区块也各算一份
 *
 * 后端那份会话按「绝对路径 # 表项下标」存（见 windows/nbt_session.rs），这里必须同口径 ——
 * 少一段，一个区块的会话就会被另一个区块拿去用。
 *
 * - `uuid` / `source` / `file`: 三段定位
 * - `chunk`: 区域文件的区块表项下标；普通 NBT 文件传 null 或不传
 *
 * # 返回值
 *
 * 该文件的会话键
 */
export function sessionKey(
  uuid: string,
  source: string,
  file: string,
  chunk?: number | null,
): string {
  const base = `${uuid}|${source}|${file}`;
  return chunk === undefined || chunk === null ? base : `${base}#${chunk}`;
}

/** 取一份空白会话 */
function blankSession(
  uuid: string,
  source: string,
  file: string,
  chunk: number | null,
): NbtSession {
  return {
    uuid,
    source,
    file,
    chunk,
    tree: null,
    sha1: "",
    ops: [],
    dirty: false,
    loaded: false,
    expanded: new Map(),
  };
}

/**
 * 取某个文件的会话，没有就现建一份
 *
 * - `uuid` / `source` / `file`: 与 [`sessionKey`] 的三段一致
 * - `chunk`: 区域文件的区块表项下标；普通 NBT 文件传 null 或不传
 *
 * # 返回值
 *
 * 该文件的会话；同一个文件多次调用给的是同一个对象，所以改动天然留在会话里
 */
export function openSession(
  uuid: string,
  source: string,
  file: string,
  chunk?: number | null,
): NbtSession {
  const key = sessionKey(uuid, source, file, chunk);
  const found = sessions.get(key);
  if (found) return found;

  const session = blankSession(uuid, source, file, chunk ?? null);
  sessions.set(key, session);
  trimSessions();
  return session;
}

/** 扔掉一份会话，并把后端那份打开着的会话一并关掉 */
function dropSession(key: string, session: NbtSession): Promise<void> {
  sessions.delete(key);
  return closeConfigNbt(session.uuid, session.source, session.file, session.chunk ?? undefined).catch(() => {
    // 关不掉也就是个句柄留着；文件本身没动过，不值得打断用户
  });
}

/** 超出上限时扔掉最早打开、且没有未保存改动的那一份；全都有改动就让它先超着 */
function trimSessions() {
  if (sessions.size <= MAX_SESSIONS) return;
  for (const [key, session] of sessions) {
    if (!session.dirty) {
      void dropSession(key, session);
      return;
    }
  }
}

/**
 * 清掉没有未保存改动的会话，连后端的文件句柄一起关
 *
 * 刷新（重列文件）与换来源 / 换实例时调用：这些时候盘上那一份可能已经不是缓存里的样子，
 * 再打开应当重新读。**要等关完再往下走** —— 关是异步的，抢在它前面打开同一个文件的话，
 * 后到的关闭会把刚建好的会话又摘掉。
 *
 * # 返回值
 *
 * 全部关完（或关失败）后 resolve
 */
export async function clearCleanSessions() {
  const clean = [...sessions].filter(([, session]) => !session.dirty);
  await Promise.all(clean.map(([key, session]) => dropSession(key, session)));
}

/**
 * 关掉全部会话：配置窗口没了，打开过的文件都该放手
 *
 * 有未保存改动的也一并关 —— 窗口都走了，那份树留着也没人能用。改动因此会丢，
 * 与"关窗口前没点保存"是同一回事。
 */
export function closeSessions() {
  for (const [key, session] of [...sessions]) dropSession(key, session);
}
