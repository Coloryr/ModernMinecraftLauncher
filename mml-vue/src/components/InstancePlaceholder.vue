<script setup lang="ts">
// 实例没有自定义图标时的占位图：**全部像素画**
//
// 每个图标就是一张字符网格 + 调色板，渲染时按行合并成色块；没有颜色的字符表示透明。
// - 原版：草方块，16×16 纯平色（只有横向色带，没有噪点）
// - Forge：铁砧；LiteLoader：鸡；自定义：开口扳手；Cleanroom：六边形 R 标，都是 24×24 手画
// - Fabric：官方是 13×14，按原图重采样成 16×16 后清掉混色；保留官方米色，
//   外轮廓用暖棕而不是近黑，浅底上不至于像描了一圈墨
// - NeoForge：官方标志本身就是像素画，按官方 18×16 像素格原样描
// - Quilt：3×3 拼布；OptiFine：红底 OF，都是 16×16
import { computed } from "vue";

/** 一张像素画：调色板 + 逐行字符 */
interface PixelArt {
  /** 字符到颜色的映射；没有映射的字符表示透明 */
  palette: Record<string, string>;
  /** 逐行字符，每行等长 */
  rows: string[];
}

const ART: Record<string, PixelArt> = {
  normal: {
    palette: {
      L: "#7cb845",
      G: "#6faa3c",
      d: "#5d9431",
      D: "#8a6244",
      K: "#7c573c",
    },
    rows: [
      "LLLLLLLLLLLLLLLL",
      "LLLLLLLLLLLLLLLL",
      "GGGGGGGGGGGGGGGG",
      "GGGGGGGGGGGGGGGG",
      "GGGGGGGGGGGGGGGG",
      "dddddddddddddddd",
      "DDDDDDDDDDDDDDDD",
      "DDDDDDDDDDDDDDDD",
      "DDDDDDDDDDDDDDDD",
      "DDDDDDDDDDDDDDDD",
      "DDDDDDDDDDDDDDDD",
      "DDDDDDDDDDDDDDDD",
      "DDDDDDDDDDDDDDDD",
      "DDDDDDDDDDDDDDDD",
      "KKKKKKKKKKKKKKKK",
      "KKKKKKKKKKKKKKKK",
    ],
  },
  forge: {
    palette: {
      l: "#b3bdc9",
      m: "#7a8697",
      s: "#5c6779",
      d: "#454e5e",
    },
    rows: [
      "........................",
      "........................",
      "........................",
      "........lllllllllll.....",
      "......llllllllllllll....",
      "....llmmmmmmmmmmmmmm....",
      "...mmmmmmmmmmmmmmmmm....",
      "..mmmmmmmmmmmmmmmmmm....",
      ".mms...ssssssssssss.....",
      "........ssssssss........",
      "..........lmmd..........",
      "..........lmmd..........",
      "..........lmmd..........",
      ".........mmmmmm.........",
      ".......llllllllll.......",
      ".....mmmmmmmmmmmmmm.....",
      "...mmmmmmmmmmmmmmmmmm...",
      "...ssssssssssssssssss...",
      "....ddddd......ddddd....",
      "........................",
      "........................",
      "........................",
      "........................",
      "........................",
    ],
  },
  fabric: {
    palette: {
      A: "#6b5537",
      B: "#dccda3",
      C: "#c4b384",
      D: "#b0a074",
      F: "#f2e9d2",
    },
    rows: [
      ".........A......",
      ".......AADA.....",
      ".......ADFDA....",
      ".......ADDBDA...",
      "......ABDDDBDA..",
      ".....ABBBBADBDA.",
      "....ABBBBBBADDAA",
      "..AAFBBBBBBBAADA",
      ".ADBBBBBBBCBAAA.",
      "ADBFBBBBBBCA....",
      "ADDBBBBBBDA.....",
      ".ADDBBBBDA......",
      "..ADDBBDA.......",
      "...ADDDAA.......",
      "....ADAAA.......",
      ".....AA.........",
    ],
  },
  quilt: {
    palette: {
      p: "#9722ff",
      m: "#dc29dd",
      c: "#27a2fd",
    },
    rows: [
      "........................",
      "........................",
      "..pppppp.mmmmmm.cccccc..",
      "..pppppp.mmmmmm.cccccc..",
      "..pppppp.mmmmmm.cccccc..",
      "..pppppp.mmmmmm.cccccc..",
      "..pppppp.mmmmmm.cccccc..",
      "..pppppp.mmmmmm.cccccc..",
      "........................",
      "..mmmmmm.cccccc.pppppp..",
      "..mmmmmm.cccccc.pppppp..",
      "..mmmmmm.cccccc.pppppp..",
      "..mmmmmm.cccccc.pppppp..",
      "..mmmmmm.cccccc.pppppp..",
      "..mmmmmm.cccccc.pppppp..",
      "........................",
      "..cccccc.pppppp.mmmmmm..",
      "..cccccc.pppppp.mmmmmm..",
      "..cccccc.pppppp.mmmmmm..",
      "..cccccc.pppppp.mmmmmm..",
      "..cccccc.pppppp.mmmmmm..",
      "..cccccc.pppppp.mmmmmm..",
      "........................",
      "........................",
    ],
  },
  neoforge: {
    palette: {
      H: "#66534d",
      I: "#8d7168",
      J: "#e7d9d3",
      K: "#a44e37",
      L: "#c7a3b9",
      M: "#bf6134",
      N: "#d7742f",
      O: "#e68c37",
      P: "#f9f4f4",
      R: "#262a33",
      S: "#13151a",
    },
    rows: [
      "ABBBBBBBBBBBBBBBBA",
      "BCCDDDDDCCCCCCCCCB",
      "BDEFGGHHGFHHFFFEFB",
      "BFGFFHIHGGHIHFFFFB",
      "BCFGHIJHKKHJIHFFEB",
      "BFFFKHLKMMKLHKFFFB",
      "BFFFKMMNOONMMKFFEB",
      "BFFFMNNNOONNNMFFEB",
      "BFFMNPJNONNJPNMFEB",
      "QFFFMPRNNNNRPMGGEQ",
      "QFEKNJSNMMNSJNKEEQ",
      "QFEEKNNNNNNNNKEEEQ",
      "QFEEKMMPSSPMMKEEEQ",
      "QFEEEKKPJJPKKEEEEQ",
      "QEQEEEEKKKKEEEEQEQ",
      "QETEEEEEEEEEEEETEQ",
    ],
  },
  cleanroom: {
    palette: {
      D: "#1f5c9e",
      w: "#f4f7fa",
    },
    rows: [
      "........................",
      "........................",
      "......DDDDDDDDDDDD......",
      "......DDDDDDDDDDDD......",
      ".....DDDDDDDDDDDDDD.....",
      "....DDDDDDwwwwwwDDDD....",
      "...DDDDwwwwwwwwwDDDDD...",
      "..DDDDDwwwwwwwwwDDDDDD..",
      ".DDDDDDwwwDDDDwwDDDDDDD.",
      ".DDDDDDwwwwwwwwwDDDDDDD.",
      ".DDDDDDwwwwwwwwwDDDDDDD.",
      ".DDDDDDwwwwwwwwDDDDDDDD.",
      ".DDDDDDwwwwwwDDDDDDDDDD.",
      ".DDDDDDwwwDwwwDDDDDDDDD.",
      ".DDDDDDwwwDDwwwDDDDDDDD.",
      ".DDDDDDwwwDDDwwwDDDDDDD.",
      ".DDDDDDwwwDDDDwwwDDDDDD.",
      "..DDDDDwwwDDDDDwwwDDDD..",
      "...DDDDDDDDDDDDDwwwDD...",
      "....DDDDDDDDDDDDDDDD....",
      ".....DDDDDDDDDDDDDD.....",
      "......DDDDDDDDDDDD......",
      "........................",
      "........................",
    ],
  },
  optifine: {
    palette: {
      r: "#6f1a1a",
      R: "#4a1010",
      y: "#f2c230",
    },
    rows: [
      "RRRRRRRRRRRRRRRR",
      "RrrrrrrrrrrrrrrR",
      "RrrrrrrrrrrrrrrR",
      "RrrryyyrryyyyyrR",
      "RrryrrryryrrrrrR",
      "RrryrrryryrrrrrR",
      "RrryrrryryyyyrrR",
      "RrryrrryryrrrrrR",
      "RrryrrryryrrrrrR",
      "RrryrrryryrrrrrR",
      "RrryrrryryrrrrrR",
      "RrrryyyrryrrrrrR",
      "RrrrrrrrrrrrrrrR",
      "RrrrrrrrrrrrrrrR",
      "RrrrrrrrrrrrrrrR",
      "RRRRRRRRRRRRRRRR",
    ],
  },
  liteloader: {
    palette: {
      w: "#f8f5ee",
      W: "#e4dfd1",
      g: "#cfc9ba",
      b: "#eda23c",
      r: "#c9503f",
      e: "#242424",
    },
    rows: [
      ".............rr.........",
      "...........rrrr.........",
      "...........rrrrrr.......",
      "...........rr..rr.......",
      "..........wwwwww........",
      ".........wwwwwwww.......",
      ".........wwwwwwww.......",
      ".........wwwwweww.......",
      ".........wwwwwewbb......",
      ".........wwwwwwwwbb.....",
      ".ggg.....wwwwwwwbb......",
      "..ggg....wwwwwwrr.......",
      "...gggwwwwwwwwwrr.......",
      ".....wwwwwwwwwwww.......",
      "....wwwwggwwwwwwww......",
      "....wwgWWWWgwwwwww......",
      "....wwgWWWWgwwwwww......",
      "....wwgWWWWgwwwwww......",
      "....wwggggggwwwwww......",
      ".....wwwggwwwwwww.......",
      "......wwwwwwwwww........",
      ".........bbwwbb.........",
      "........bbb..bbb........",
      "........................",
    ],
  },
  custom: {
    palette: {
      l: "#c8d0db",
      m: "#a7b0bf",
      s: "#6f7889",
    },
    rows: [
      "...s....................",
      "....s...................",
      ".....sssss..............",
      "l.....sssss.............",
      ".l.....sssss............",
      "..l.....sssss...........",
      "..ll.....ssss...........",
      "..lll.....sss...........",
      "..llll.....ss...........",
      "..lllll....ss...........",
      "...lllll..mmss..........",
      "....llllllmmmss.........",
      ".....llllllmmmss........",
      "..........llmmmss.......",
      "...........llmmmss......",
      "............llmmmss.....",
      ".............llmmmss....",
      "..............llmmmss...",
      "...............llmmmss..",
      "................llmmmss.",
      ".................llmmmss",
      "..................llmmms",
      "...................llmmm",
      "....................llmm",
    ],
  },
};

const props = withDefaults(
  defineProps<{
    /** 加载器 id：normal / forge / fabric / quilt / neoforge / optifine / cleanroom / liteloader / custom */
    loader?: string;
    /** 边长，单位 px */
    size?: number;
  }>(),
  { loader: "", size: 48 },
);

/** 当前图标；认不出的加载器统一画扳手 */
const art = computed(() => ART[props.loader] ?? ART.custom);

/** 网格尺寸：viewBox 用它，非正方形时按比例居中 */
const box = computed(() => art.value.rows[0].length + " " + art.value.rows.length);

/** 按行合并同色格子，少画一堆 rect */
const blocks = computed(() => {
  const { palette, rows } = art.value;
  const out: { x: number; y: number; w: number; fill: string }[] = [];
  rows.forEach((row, y) => {
    let x = 0;
    while (x < row.length) {
      const ch = row[x];
      let end = x + 1;
      while (end < row.length && row[end] === ch) end++;
      const fill = palette[ch];
      if (fill) out.push({ x, y, w: end - x, fill });
      x = end;
    }
  });
  return out;
});
</script>

<template>
  <svg class="inst-ph" :viewBox="'0 0 ' + box" shape-rendering="crispEdges" stroke="none" aria-hidden="true">
    <!-- 每块向外扩 0.02：相邻色块互相压住一点，缩放时缝里不会透出底色 -->
    <rect v-for="(b, i) in blocks" :key="i" :x="b.x - 0.02" :y="b.y - 0.02" :width="b.w + 0.04"
      height="1.04" :fill="b.fill" />
  </svg>
</template>

<style scoped>
/* 铺满父容器（实例图标那块），圆角与裁切由父级负责 */
.inst-ph {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}
</style>