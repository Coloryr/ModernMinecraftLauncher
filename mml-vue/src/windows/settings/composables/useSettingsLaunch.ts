// 设置窗口 · 游戏启动标签：内存 / JVM / 参数 / 启动命令，外加游戏窗口与游戏标题
//
// 原来的保存模型保留：整份 `run` + `win` 一起防抖 500ms 落盘。本文件补了三件事：
// 1. 内存冲突（min > max）不再只是弹个 toast 就完事 —— 露出 `memoryConflict` 给界面标红，
//    并把保存状态标成失败，避免"界面显示非法值、磁盘还是旧值"却毫无提示；
// 2. 冲突提示每轮只弹一次；原来防抖每 500ms 就再弹一遍；
// 3. 关窗前把还没到时的防抖改动立刻保存；原来在 500ms 内关窗会丢改动。
import { computed, onUnmounted, ref, watch } from "vue";
import { t, tErr } from "../../../lib/i18n";
import { showToast } from "../../../lib/toast";
import { commands } from "../../../lib/bindings";
import type { RunArgSettingDto, WindowSettingDto } from "../../../lib/bindings";
import { useSaveState } from "./useSaveState";
import { loadDefaults } from "./useSettingsDefaults";

/** 连续输入合并窗口，单位毫秒 */
const SAVE_DEBOUNCE = 500;

export interface EnvLine {
  key: string;
  value: string;
}

/** 把 `KEY=VALUE` 多行文本拆成逐条草稿，与 core 的 splitn(2,'=') 语义一致 */
function parseEnvLines(text: string): EnvLine[] {
  return text
    .split("\n")
    .filter((l) => l.trim())
    .map((l) => {
      const i = l.indexOf("=");
      return i < 0
        ? { key: l.trim(), value: "" }
        : { key: l.slice(0, i).trim(), value: l.slice(i + 1) };
    });
}

/**
 * 游戏启动标签的状态与保存
 *
 * # 返回值
 *
 * - `run` / `win`: 启动参数与游戏窗口设置，两者一起防抖 500ms 落盘
 * - `envLines` / `addEnvLine` / `removeEnvLine` / `commitEnvLines`: JVM 环境变量的逐行编辑
 * - `gcOptions` / `memoryConflict` / `saveState`: 界面选项、内存冲突标记与保存状态
 * - `load` / `applyLaunch` / `saveNow` / `resetGroup`: 加载、落盘与分组级恢复默认
 */
export function useSettingsLaunch() {
  const run = ref<RunArgSettingDto | null>(null);
  const win = ref<WindowSettingDto | null>(null);
  /** JVM 环境变量逐条编辑，键 / 值两框；落盘时合成回 run.jvmEnv */
  const envLines = ref<EnvLine[]>([]);

  const { state, track, markError } = useSaveState();

  /** 初次加载完成前不触发自动保存 */
  let loaded = false;
  let timer: number | null = null;
  /** 本轮冲突是否已提示过（避免防抖反复弹） */
  let conflictNotified = false;

  // ---------- 选项 ----------

  const gcOptions = computed(() => [
    { value: "Auto", label: t("winSettings.gcAuto") },
    { value: "G1GC", label: t("winSettings.gcG1gc") },
    { value: "ZGC", label: t("winSettings.gcZgc") },
    { value: "None", label: t("winSettings.gcNone") },
  ]);

  // ---------- 加载 ----------

  async function load() {
    const launch = await commands.settings.getLaunch().catch(() => null);
    if (!launch) return;
    run.value = launch.run;
    win.value = launch.window;
    envLines.value = parseEnvLines(launch.run.jvmEnv);
    loaded = true;
  }

  // ---------- 校验 ----------

  /** 最小内存大于最大内存：界面据此标红，保存也会被拦住 */
  const memoryConflict = computed(() => {
    const r = run.value;
    return !!r && r.minMemory > r.maxMemory;
  });

  // ---------- 保存 ----------

  /** 把逐条编辑的环境变量合成回 run.jvmEnv（保存前调用） */
  function commitEnvLines() {
    if (!run.value) return;
    const merged = envLines.value
      .filter((l) => l.key.trim())
      .map((l) => `${l.key.trim()}=${l.value}`)
      .join("\n");
    if (run.value.jvmEnv !== merged) run.value.jvmEnv = merged;
  }

  async function applyLaunch() {
    if (!run.value || !win.value) return;
    if (memoryConflict.value) {
      // 不落盘：状态标失败 + 只在进入冲突时提示一次（界面另有红框标记）
      markError();
      if (!conflictNotified) {
        conflictNotified = true;
        showToast(t("winSettings.memoryConflict"));
      }
      return;
    }
    conflictNotified = false;
    try {
      commitEnvLines();
      await track(() => commands.settings.saveLaunch(run.value!, win.value!));
    } catch (e) {
      showToast(tErr(e));
    }
  }

  /** 立刻保存（关窗 / 需要马上生效时用） */
  async function saveNow() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    await applyLaunch();
  }

  watch(
    [run, win, envLines],
    () => {
      if (!loaded) return;
      if (timer !== null) clearTimeout(timer);
      timer = window.setTimeout(() => {
        timer = null;
        void applyLaunch();
      }, SAVE_DEBOUNCE);
    },
    { deep: true },
  );

  // ---------- 环境变量逐行编辑 ----------

  function addEnvLine() {
    envLines.value.push({ key: "", value: "" });
  }

  function removeEnvLine(i: number) {
    envLines.value.splice(i, 1);
  }

  // ---------- 恢复默认 ----------

  /** 这几个分组的默认值都在 Rust，见 RunArgObj::new / WindowSettingObj::new */

  async function resetGroup(id: string): Promise<boolean> {
    const d = await loadDefaults();
    if (!d) return false;
    switch (id) {
      case "gameWindow":
        if (!win.value) return false;
        win.value.fullScreen = d.window.fullScreen;
        win.value.width = d.window.width;
        win.value.height = d.window.height;
        break;
      case "memory":
        if (!run.value) return false;
        run.value.minMemory = d.run.minMemory;
        run.value.maxMemory = d.run.maxMemory;
        break;
      case "jvm":
        if (!run.value) return false;
        run.value.gcMode = d.run.gcMode;
        run.value.colorasm = d.run.colorasm;
        run.value.removeJvmArg = d.run.removeJvmArg;
        run.value.jvmArgs = d.run.jvmArgs;
        run.value.jvmEnv = d.run.jvmEnv;
        // 逐行草稿跟着重建，否则界面还显示旧的环境变量
        envLines.value = parseEnvLines(d.run.jvmEnv);
        break;
      case "gameArgs":
        if (!run.value) return false;
        run.value.removeGameArg = d.run.removeGameArg;
        run.value.gameArgs = d.run.gameArgs;
        break;
      case "launchCmd":
        if (!run.value) return false;
        run.value.launchPreRun = d.run.launchPreRun;
        run.value.preRunWithGame = d.run.preRunWithGame;
        run.value.launchPostRun = d.run.launchPostRun;
        run.value.preRunArg = d.run.preRunArg;
        run.value.postRunArg = d.run.postRunArg;
        break;
      case "gameTitle":
        if (!win.value) return false;
        win.value.editTitle = d.window.editTitle;
        win.value.gameTitle = d.window.gameTitle;
        win.value.randomTitle = d.window.randomTitle;
        win.value.cycleTitle = d.window.cycleTitle;
        win.value.titleDelay = d.window.titleDelay;
        break;
      default:
        return false;
    }
    // 直接落盘（不等防抖），恢复默认要立即生效
    await saveNow();
    return true;
  }

  onUnmounted(() => {
    // 关窗前把还在防抖里的改动落盘，原来 500ms 内关窗会丢
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
      void applyLaunch();
    }
  });

  return {
    run,
    win,
    envLines,
    gcOptions,
    memoryConflict,
    load,
    applyLaunch,
    saveNow,
    commitEnvLines,
    addEnvLine,
    removeEnvLine,
    saveState: state,
    resetGroup,
  };
}
