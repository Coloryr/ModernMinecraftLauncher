// 设置窗口 · 皮肤与头像标签
//
// 头像类型 / 偏移、皮肤显示模式的真源在 lib/settings.ts，各自 saveGuiConfig 并刷新图片版本号，
// 这里只补上：分段控件选项、样例预览地址、分组级"恢复默认"。
import { computed } from "vue";
import { t } from "../../../lib/i18n";
import {
  headType,
  headX,
  headY,
  setHeadConfig,
  setSkinDisplay,
  skinDisplay,
  type HeadType,
  type SkinDisplay,
} from "../../../lib/settings";
import { imageBase, imageVersion } from "../../../lib/accountImages";

/** 与 Rust HeadConfig::default / SkinDisplay::default 一致 */
const DEFAULT_HEAD_TYPE: HeadType = "Head2DA";
const DEFAULT_HEAD_X = 15;
const DEFAULT_HEAD_Y = 65;
const DEFAULT_SKIN_DISPLAY: SkinDisplay = "Skin2DA";

/**
 * 皮肤与头像标签的状态与选项
 *
 * # 返回值
 *
 * - `headType` / `headX` / `headY` / `setHeadConfig` / `headTypeOptions` / `onHeadTypeChange`: 头像显示模式与偏移
 * - `skinDisplay` / `skinDisplayOptions` / `onSkinDisplayChange`: 皮肤显示模式
 * - `offsetAdjustable`: 当前头像档位是否需要手调偏移
 * - `imageBase` / `headPreviewUrl` / `skinPreviewUrl`: 样例预览地址，配置变化经版本号重取
 * - `resetGroup`: 分组级"恢复默认"，返回是否处理了该分组
 */
export function useSettingsSkin() {
  /** 设置页头像样例：内置纤细皮肤按当前头像配置渲染，配置变化经 imageVersion 重取 */
  const headPreviewUrl = computed(() =>
    imageBase.value ? `${imageBase.value}/head/preview/0?v=${imageVersion.value}` : "",
  );

  /** 设置页皮肤样例：内置纤细皮肤按当前皮肤显示模式渲染 */
  const skinPreviewUrl = computed(() =>
    imageBase.value ? `${imageBase.value}/skin/preview/0/slim?v=${imageVersion.value}` : "",
  );

  // 选项顺序与文案键沿用原模板：3D 那三档是 3DA / 3DC / 3DB 这个顺序
  const headTypeOptions = computed(() => [
    { value: "Head2DA", label: t("winSettings.headType2DA") },
    { value: "Head2DB", label: t("winSettings.headType2DB") },
    { value: "Head3DA", label: t("winSettings.headType3DA") },
    { value: "Head3DC", label: t("winSettings.headType3DC") },
    { value: "Head3DB", label: t("winSettings.headType3DB") },
  ]);

  const skinDisplayOptions = computed(() => [
    { value: "Skin2DA", label: t("winSettings.skin2da") },
    { value: "Skin2DB", label: t("winSettings.skin2db") },
    { value: "Skin3D", label: t("winSettings.skin3d") },
    { value: "Skin3DD", label: t("winSettings.skin3dd") },
  ]);

  /** 只有 3D B 档需要手调偏移，与原来模板里的 v-if 一致 */
  const offsetAdjustable = computed(() => headType.value === "Head3DB");

  /** 分段控件给的是 string，这里收口成枚举 */
  function onHeadTypeChange(v: string) {
    setHeadConfig(v as HeadType);
  }

  function onSkinDisplayChange(v: string) {
    setSkinDisplay(v as SkinDisplay);
  }

  /** 分组级"恢复默认"：返回是否处理了该分组；皮肤页拆成"头像"与"皮肤"两组 */
  async function resetGroup(id: string): Promise<boolean> {
    switch (id) {
      case "head":
        setHeadConfig(DEFAULT_HEAD_TYPE, DEFAULT_HEAD_X, DEFAULT_HEAD_Y);
        return true;
      case "skin":
        setSkinDisplay(DEFAULT_SKIN_DISPLAY);
        return true;
      default:
        return false;
    }
  }

  return {
    imageBase,
    headPreviewUrl,
    skinPreviewUrl,
    headType,
    headX,
    headY,
    setHeadConfig,
    headTypeOptions,
    onHeadTypeChange,
    skinDisplay,
    skinDisplayOptions,
    onSkinDisplayChange,
    offsetAdjustable,
    resetGroup,
  };
}
