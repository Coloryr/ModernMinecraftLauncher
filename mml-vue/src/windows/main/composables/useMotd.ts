// 主窗口的两处 MOTD 卡片：最上方那张查客户端设置里的全局地址，实例「自定义服务器」
// 里那张查实例自己配的地址。两处都走内核的 Server List Ping。
//
// 卡片外观在 components/MotdCard.vue：图标 / 彩色分段 / 人数-版本-延迟都在那边，
// 这里只管**地址来源、竞态保护与定时刷新**。
import { onUnmounted, ref, watch, type Ref } from "vue";

import { api } from "../../../lib/api";
import type { ClientConfig } from "../../../lib/guiConfig";
import type { InstanceArgsDto, InstanceInfoDto, MotdDto } from "../../../lib/bindings";

/**
 * 主窗口两处 MOTD 卡片的查询与刷新
 *
 * - `clientConfig`: 客户端设置，含 MOTD 开关 / 全局地址 / 自动刷新间隔
 * - `selected`: 当前选中实例，实例那张卡的地址来源
 * - `argsOf`: 取实例启动参数，读其中的服务器地址与端口
 *
 * # 返回值
 *
 * - `motdInfo` / `motdLoading`: 最上方那张卡的结果与加载态
 * - `instMotd` / `instMotdLoading`: 实例那张卡的结果与加载态
 * - `motdCardVisible`: 最上方那张卡是否该显示，由客户端设置的开关 + 地址决定
 * - `refreshMotd` / `refreshInstMotd`: 立刻重查
 * - `restartMotdTimer`: 按客户端设置重建自动刷新定时器
 */
export function useMotd(opts: {
  clientConfig: Ref<ClientConfig>;
  selected: Ref<InstanceInfoDto | null>;
  argsOf: (uuid: string) => InstanceArgsDto;
}) {
  const { clientConfig, selected, argsOf } = opts;

  // ---- 最上方那张卡：查客户端设置里的全局地址 ----

  const motdInfo = ref<MotdDto | null>(null);
  const motdLoading = ref(false);
  // 竞态保护：旧请求晚到不覆盖新结果
  let motdSeq = 0;

  /** 重查；地址为空时什么都不做，卡片由 MainWindow 按开关隐藏 */
  async function refreshMotd() {
    const addr = clientConfig.value.motdServer.trim();
    if (!addr) return;
    // 不因"已有请求在飞"提前返回：地址刚改时那次旧请求会让新地址一直等不到结果
    const seq = ++motdSeq;
    motdLoading.value = true;
    try {
      const dto = await api.getMotd(addr);
      if (seq === motdSeq) motdInfo.value = dto;
    } catch {
      if (seq === motdSeq) motdInfo.value = null;
    } finally {
      if (seq === motdSeq) motdLoading.value = false;
    }
  }

  /**
   * 把实例配的地址补成"主机:端口"
   *
   * 地址里已经写了端口的原样用，不能再补一次 —— 拼成 `主机:端口:端口` 后端解析不出端口，
   * 卡片就一直是空的。裸 IPv6 含多个冒号，补端口时要加方括号。
   *
   * - `ip`: 用户填的服务器地址
   * - `port`: 实例里配的端口，地址自己带端口时不生效
   *
   * # 返回值
   *
   * 交给 MOTD 查询的地址
   */
  function withPort(ip: string, port: number): string {
    const tail = ip.slice(ip.lastIndexOf(":") + 1);
    // 方括号写法 `[::1]` / `[::1]:25565`：看最后一个冒号之后是不是端口
    if (ip.startsWith("[")) {
      return /^\d{1,5}$/.test(tail) ? ip : `${ip}:${port}`;
    }
    // 没有冒号 = 没写端口，补实例里配的那个
    if (!ip.includes(":")) return `${ip}:${port}`;
    // 只有一个冒号且后面是数字，说明已经带了端口
    if (ip.split(":").length === 2 && /^\d{1,5}$/.test(tail)) return ip;
    // 其余是裸 IPv6：按规范加方括号再补端口
    return `[${ip}]:${port}`;
  }

  // ---- 实例「自定义服务器」里那张卡 ----

  const instMotd = ref<MotdDto | null>(null);
  const instMotdLoading = ref(false);
  let instMotdSeq = 0;
  let instMotdTimer: number | null = null;

  /** 重查实例配的那台服务器；没填地址时清空结果，并把在途请求作废 */
  async function refreshInstMotd() {
    const inst = selected.value;
    const ip = inst ? argsOf(inst.uuid).serverIp.trim() : "";
    // 先占新序号：地址被清空或换成别的服务器时，旧请求晚到的结果不再写回
    const seq = ++instMotdSeq;
    if (!ip) {
      instMotd.value = null;
      instMotdLoading.value = false;
      return;
    }
    const port = inst ? argsOf(inst.uuid).serverPort || 25565 : 25565;
    instMotdLoading.value = true;
    try {
      const dto = await api.getMotd(withPort(ip, port));
      if (seq === instMotdSeq) instMotd.value = dto;
    } catch {
      if (seq === instMotdSeq) instMotd.value = null;
    } finally {
      if (seq === instMotdSeq) instMotdLoading.value = false;
    }
  }

  // 切实例或改地址后重查。地址是一边打字一边变的，压 400ms 再发，
  // 否则每敲一个字符都往服务器打一次
  watch(
    () => {
      const inst = selected.value;
      if (!inst) return "";
      const a = argsOf(inst.uuid);
      return inst.uuid + "|" + a.serverIp + "|" + a.serverPort;
    },
    () => {
      if (instMotdTimer !== null) clearTimeout(instMotdTimer);
      instMotdTimer = window.setTimeout(() => {
        instMotdTimer = null;
        void refreshInstMotd();
      }, 400);
    },
    { immediate: true },
  );

  // ---- 最上方那张卡的显隐与自动刷新定时器（由客户端设置驱动）----

  /** 卡片是否该显示；由 MainWindow 按客户端设置的开关与地址同步过来 */
  const motdCardVisible = ref(true);
  let motdTimer: number | null = null;

  /** 按客户端设置的间隔重建定时刷新；卡片不显示时不挂 */
  function restartMotdTimer() {
    if (motdTimer !== null) clearInterval(motdTimer);
    motdTimer = null;
    if (!motdCardVisible.value) return;
    const sec = clientConfig.value.motdInterval;
    if (sec >= 5) {
      motdTimer = window.setInterval(refreshMotd, sec * 1000);
    }
  }

  // 定时器随组件卸载一起清掉
  onUnmounted(() => {
    if (motdTimer !== null) clearInterval(motdTimer);
    if (instMotdTimer !== null) clearTimeout(instMotdTimer);
  });

  return {
    motdInfo,
    motdLoading,
    instMotd,
    instMotdLoading,
    motdCardVisible,
    refreshMotd,
    refreshInstMotd,
    restartMotdTimer,
  };
}
