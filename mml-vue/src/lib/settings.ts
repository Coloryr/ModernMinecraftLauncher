// 界面设置：侧栏位置 / 收起状态 / 列表显示模式 / 皮肤与头像显示
// 持久化到本地存储 + gui_config.json
import { ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import {
  loadGuiConfig,
  normalizeHeadType,
  normalizeSkinDisplay,
  normalizeViewMode,
  saveGuiConfig,
  type HeadType,
  type SidebarSide,
  type SkinDisplay,
  type ViewMode,
} from "./guiConfig";
import {
  KEYS,
  readFlag,
  readNumber,
  readString,
  writeFlag,
  writeNumber,
  writeString,
} from "./storage";
import { bumpImageVersion } from "./accountImages";
import { isTauri } from "../windows/windowManager";
import { SkinConfigChange } from "./listens";
export type { SidebarSide, ViewMode, SkinDisplay, HeadType } from "./guiConfig";

export const sidebarSide = ref<SidebarSide>(
  readString(KEYS.sidebarSide) === "Right" ? "Right" : "Left",
);

// 侧栏是否收起：默认收起，只有显式存过 "0" 才是展开
export const sidebarCollapsed = ref(readFlag(KEYS.sidebarCollapsed, true));

// 实例列表显示模式：默认列表；用户改过后用用户的值
export const viewMode = ref<ViewMode>(normalizeViewMode(readString(KEYS.viewMode)));

/**
 * 设置侧栏在左还是在右
 *
 * - `side`: 侧栏位置
 */
export function setSidebarSide(side: SidebarSide) {
  sidebarSide.value = side;
  writeString(KEYS.sidebarSide, side);
  saveGuiConfig({ mainWindow: { sidebarSide: side } });
}

/**
 * 设置侧栏是否收起
 *
 * - `v`: 是否收起
 */
export function setSidebarCollapsed(v: boolean) {
  sidebarCollapsed.value = v;
  writeFlag(KEYS.sidebarCollapsed, v);
  saveGuiConfig({ mainWindow: { sidebarCollapsed: v } });
}

/**
 * 设置实例列表显示模式
 *
 * - `v`: 显示模式
 */
export function setViewMode(v: ViewMode) {
  viewMode.value = v;
  writeString(KEYS.viewMode, v);
  saveGuiConfig({ mainWindow: { viewMode: v } });
}

/** 当前选中实例 uuid（空串 = 未选中） */
export const selectedInstance = ref(readString(KEYS.selectedInstance));

function mirrorSelected(uuid: string) {
  writeString(KEYS.selectedInstance, uuid);
}

/**
 * 启动引导：从 gui_config 恢复选中实例，只同步内存 + 本地存储，不回写配置文件
 *
 * - `uuid`: 选中实例 uuid，空串 = 未选中
 */
export function restoreSelectedInstance(uuid: string) {
  selectedInstance.value = uuid;
  mirrorSelected(uuid);
}

/**
 * 选中实例变化：写本地存储 + gui_config.json
 *
 * uuid 未变则直接返回，避免无谓的 IPC 与落盘。
 *
 * - `uuid`: 选中实例 uuid，空串 = 未选中
 */
export function setSelectedInstance(uuid: string) {
  if (selectedInstance.value === uuid) return;
  selectedInstance.value = uuid;
  mirrorSelected(uuid);
  saveGuiConfig({ mainWindow: { selectedInstance: uuid } });
}

// ---- 皮肤显示模式：账户界面皮肤预览的 2D TypeA / 2D TypeB / 3D 模型 ----

export const skinDisplay = ref<SkinDisplay>(
  normalizeSkinDisplay(readString(KEYS.skinDisplay)),
);

/**
 * 设置皮肤显示模式
 *
 * - `v`: 显示模式
 */
export function setSkinDisplay(v: SkinDisplay) {
  skinDisplay.value = v;
  writeString(KEYS.skinDisplay, v);
  saveGuiConfig({ skinDisplay: v });
  bumpImageVersion();
}

/**
 * 启动恢复：只同步内存与镜像，不回写 gui_config.json
 *
 * - `v`: 显示模式
 */
export function restoreSkinDisplay(v: SkinDisplay) {
  skinDisplay.value = v;
  writeString(KEYS.skinDisplay, v);
}

// ---- 头像显示模式：渲染端按 head 配置出图，改完自动生效 ----

export const headType = ref<HeadType>(normalizeHeadType(readString(KEYS.headType)));
// 3D 旋转模式默认角度：x 15 / y 65（没存过或存了非法值时用默认）
export const headX = ref(readAngle(KEYS.headX, 15));
export const headY = ref(readAngle(KEYS.headY, 65));

function readAngle(key: (typeof KEYS)["headX"] | (typeof KEYS)["headY"], dflt: number): number {
  return readNumber(key, dflt);
}

/**
 * 设置头像渲染模式，3D 旋转模式附带 X / Y 角度
 *
 * - `type`: 头像渲染模式
 * - `x`: 3D 旋转 X 角度，缺省用当前值
 * - `y`: 3D 旋转 Y 角度，缺省用当前值
 */
export function setHeadConfig(type: HeadType, x = headX.value, y = headY.value) {
  headType.value = type;
  headX.value = x;
  headY.value = y;
  writeString(KEYS.headType, type);
  writeNumber(KEYS.headX, x);
  writeNumber(KEYS.headY, y);
  saveGuiConfig({ head: { headType: type, x, y } });
  bumpImageVersion();
}

/**
 * 启动恢复：只同步内存与镜像，不回写 gui_config.json
 *
 * - `type`: 头像渲染模式
 * - `x`: 3D 旋转 X 角度
 * - `y`: 3D 旋转 Y 角度
 */
export function restoreHeadConfig(type: HeadType, x: number, y: number) {
  headType.value = type;
  headX.value = x;
  headY.value = y;
  writeString(KEYS.headType, type);
  writeNumber(KEYS.headX, x);
  writeNumber(KEYS.headY, y);
}

// ---- 跨窗口同步 ----
// 每个窗口是独立 JS 上下文，ref 不会自动同步：设置窗口改了皮肤 / 头像模式后，
// 后端在 saveGuiConfig 时广播 skin-config-change，本窗口重读配置并让所有
// 账户图片带新版本号重取：后端渲染缓存键含这两项配置，取到的就是新模式渲染图
if (isTauri()) {
  void listen(SkinConfigChange, async () => {
    const cfg = await loadGuiConfig();
    if (!cfg) return;
    restoreSkinDisplay(cfg.skinDisplay);
    restoreHeadConfig(cfg.head.headType, cfg.head.x, cfg.head.y);
    bumpImageVersion();
  });
}

