<script setup lang="ts">
// 标题栏上的自定义音乐入口：播放 / 暂停 + 停止
//
// 只有持有播放器的那个窗口显示（多窗口模式下就是主窗口），其余窗口整块不存在；
// 没配音乐也不显示，标题栏不会多出一枚用不上的按钮。见 lib/music.ts
import { t } from "../lib/i18n";
import { useMusic } from "../lib/music";
import GlyphIcon from "./ui/GlyphIcon.vue";

const { available, playing, active, toggle, stop } = useMusic();
</script>

<template>
  <div v-if="available" class="music-indicator">
    <button class="music-btn" v-tip="playing ? t('music.pause') : t('music.play')"
      :aria-label="playing ? t('music.pause') : t('music.play')" @click="toggle">
      <GlyphIcon :name="playing ? 'pause' : 'play'" :size="14" />
    </button>
    <!-- 停止只在有进度时给：没在播时点它没有意义 -->
    <button v-if="active" class="music-btn" v-tip="t('music.stop')" :aria-label="t('music.stop')"
      @click="stop">
      <GlyphIcon name="stop" :size="14" />
    </button>
  </div>
</template>

<style scoped>
/* 与标题栏那批图标按钮同一套：32 见方、平时透明、悬停浮一层淡底 */
.music-indicator {
  display: flex;
  align-items: center;
  gap: 2px;
}

.music-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--ctl-h);
  height: var(--ctl-h);
  padding: 0;
  border: none;
  border-radius: var(--ctl-radius);
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.music-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}
</style>
