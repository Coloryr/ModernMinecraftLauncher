// 设置窗口 · 网络与下载标签
//
// 保存模型（本文件是本次重构的重点）：
// - **非代理项**即改即存：下载源 / 线程 / 校验开关 / DNS，与原来一致；
// - **代理项改成"草稿 + 显式保存"**：原来代理字段直接改 `network` 对象，而任一处
//   `applyNetwork()` 都会把整个 DTO 提交，于是"只改本地、没点保存"的代理地址会被别的开关
//   顺带写盘。现在草稿独立，只有点「保存」才合并进 `network` 并落盘。
import { computed, reactive, ref } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { commands } from "../../../lib/bindings";
import type { NetworkSettingDto } from "../../../lib/bindings";
import { useSaveState } from "./useSaveState";
import { loadDefaults } from "./useSettingsDefaults";

/** 代理草稿：只在这些字段里改，点保存才落到 network */
export interface ProxyDraft {
  workProxy: string;
  loginProxy: string;
  workProxyType: string;
  loginProxyType: string;
  proxyIp: string;
  proxyPort: number;
  proxyUser: string;
  proxyPassword: string;
}

const EMPTY_DRAFT: ProxyDraft = {
  workProxy: "Auto",
  loginProxy: "Auto",
  workProxyType: "Http",
  loginProxyType: "Http",
  proxyIp: "",
  proxyPort: 0,
  proxyUser: "",
  proxyPassword: "",
};

/**
 * 网络与下载标签的状态与保存
 *
 * # 返回值
 *
 * - `network`: 已落盘的网络设置，非代理项直接改它
 * - `proxy`: 代理草稿，与 `proxyDirty` / `proxyDetailVisible` 一起驱动保存按钮
 * - `dnsLines` / `commitDns` / `commitDnsLine` / `setDnsLine` / `removeDnsLine` / `addDnsLine`: DoH 地址逐行编辑
 * - `sourceOptions` / `proxyModeOptions` / `proxyTypeOptions`: 分段控件的选项
 * - `load` / `applyNetwork` / `saveProxy` / `saveState` / `resetGroup`: 加载、落盘与分组级恢复默认
 */
export function useSettingsNetwork() {
  /** 已落盘的网络设置；非代理项直接改它再 applyNetwork */
  const network = ref<NetworkSettingDto | null>(null);
  /** DoH 地址逐行草稿（失焦 / 回车才落盘） */
  const dnsLines = ref<string[]>([]);
  /** 代理草稿 */
  const proxy = reactive<ProxyDraft>({ ...EMPTY_DRAFT });

  const { state, track, markError } = useSaveState();

  // ---------- 选项 ----------

  const sourceOptions = computed(() => [
    { value: "Offical", label: t("winSettings.sourceOffical") },
    { value: "Bmclapi", label: t("winSettings.sourceBmclapi") },
  ]);
  const proxyModeOptions = computed(() => [
    { value: "Auto", label: t("winSettings.proxyAuto") },
    { value: "None", label: t("winSettings.proxyNone") },
    { value: "User", label: t("winSettings.proxyUser") },
  ]);
  const proxyTypeOptions = [
    { value: "Http", label: "HTTP" },
    { value: "Sock4", label: "SOCKS4" },
    { value: "Sock5", label: "SOCKS5" },
  ];

  // ---------- 加载 ----------

  async function load() {
    const dto = await commands.settings.getNetwork().catch(() => null);
    network.value = dto;
    if (dto) {
      dnsLines.value = [...dto.dns.https];
      syncDraftFromDto(dto);
    }
  }

  function syncDraftFromDto(dto: NetworkSettingDto) {
    proxy.workProxy = dto.workProxy;
    proxy.loginProxy = dto.loginProxy;
    proxy.workProxyType = dto.workProxyType;
    proxy.loginProxyType = dto.loginProxyType;
    proxy.proxyIp = dto.proxyIp;
    proxy.proxyPort = dto.proxyPort;
    proxy.proxyUser = dto.proxyUser;
    proxy.proxyPassword = dto.proxyPassword;
  }

  /** 任一路走手动代理才需要填详情 */
  const proxyDetailVisible = computed(
    () => proxy.workProxy === "User" || proxy.loginProxy === "User",
  );

  /** 代理草稿与已落盘值有差异，驱动"未保存"提示与保存按钮可用态 */
  const proxyDirty = computed(() => {
    const dto = network.value;
    if (!dto) return false;
    return (
      dto.workProxy !== proxy.workProxy ||
      dto.loginProxy !== proxy.loginProxy ||
      dto.workProxyType !== proxy.workProxyType ||
      dto.loginProxyType !== proxy.loginProxyType ||
      dto.proxyIp !== proxy.proxyIp ||
      dto.proxyPort !== proxy.proxyPort ||
      dto.proxyUser !== proxy.proxyUser ||
      dto.proxyPassword !== proxy.proxyPassword
    );
  });

  // ---------- 保存 ----------

  /** 非代理项即改即存：成功不提示（免得每次切换都打扰），失败才报 */
  async function applyNetwork() {
    if (!network.value) return;
    try {
      await track(() => commands.settings.saveNetwork(network.value!));
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /** 显式保存代理：草稿合并进 network 再落盘，成功后提示已生效、在途请求已中断 */
  async function saveProxy() {
    const dto = network.value;
    if (!dto) return;
    dto.workProxy = proxy.workProxy;
    dto.loginProxy = proxy.loginProxy;
    dto.workProxyType = proxy.workProxyType;
    dto.loginProxyType = proxy.loginProxyType;
    dto.proxyIp = proxy.proxyIp;
    dto.proxyPort = proxy.proxyPort;
    dto.proxyUser = proxy.proxyUser;
    dto.proxyPassword = proxy.proxyPassword;
    try {
      // 保存后后端会重建 HTTP 客户端并中断所有在途请求（卡住的请求会立刻失败），
      // 所以这里明确告诉用户"已生效、之前的请求已中断"
      await track(() => commands.settings.saveNetwork(dto));
      showToast(t("winSettings.proxyApplied"));
    } catch (e) {
      showToast(tErr(e));
    }
  }

  // ---------- DNS 逐行编辑 ----------

  /** 草稿 → network（过滤空行） */
  function commitDns() {
    if (!network.value) return;
    network.value.dns.https = dnsLines.value.map((s) => s.trim()).filter((s) => s.length > 0);
    dnsLines.value = [...network.value.dns.https];
  }

  function setDnsLine(i: number, v: string) {
    dnsLines.value[i] = v;
  }

  function removeDnsLine(i: number) {
    dnsLines.value.splice(i, 1);
    commitDns();
    void applyNetwork();
  }

  function addDnsLine() {
    commitDns();
    dnsLines.value.push("");
    void applyNetwork();
  }

  /** 失焦 / 回车：把该行落到 network 并保存 */
  function commitDnsLine() {
    commitDns();
    void applyNetwork();
  }

  // ---------- 恢复默认 ----------

  /** 这几个分组的默认值都在 Rust，见 HttpObj / DnsObj / GameCheckObj 的 Default */

  async function resetGroup(id: string): Promise<boolean> {
    const d = await loadDefaults();
    if (!d || !network.value) return false;
    const n = network.value;
    switch (id) {
      case "download":
        n.source = d.network.source;
        n.downloadThread = d.network.downloadThread;
        n.checkFile = d.network.checkFile;
        n.autoDownload = d.network.autoDownload;
        break;
      case "proxy":
        n.workProxy = d.network.workProxy;
        n.loginProxy = d.network.loginProxy;
        n.workProxyType = d.network.workProxyType;
        n.loginProxyType = d.network.loginProxyType;
        n.proxyIp = d.network.proxyIp;
        n.proxyPort = d.network.proxyPort;
        n.proxyUser = d.network.proxyUser;
        n.proxyPassword = d.network.proxyPassword;
        // 草稿跟着归位，否则界面显示的还是被重置掉的旧代理
        syncDraftFromDto(n);
        break;
      case "dns":
        n.dns.enable = d.network.dns.enable;
        n.dns.https = [...d.network.dns.https];
        n.dns.httpProxy = d.network.dns.httpProxy;
        dnsLines.value = [...d.network.dns.https];
        break;
      case "gameCheck":
        n.check = { ...d.network.check };
        break;
      default:
        return false;
    }
    await applyNetwork();
    return true;
  }

  return {
    network,
    dnsLines,
    proxy,
    proxyDetailVisible,
    proxyDirty,
    sourceOptions,
    proxyModeOptions,
    proxyTypeOptions,
    load,
    applyNetwork,
    saveProxy,
    commitDns,
    commitDnsLine,
    setDnsLine,
    removeDnsLine,
    addDnsLine,
    saveState: state,
    resetGroup,
    markError,
  };
}
