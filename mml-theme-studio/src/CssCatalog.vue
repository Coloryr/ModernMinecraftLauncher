<script setup lang="ts">
import { computed, ref } from 'vue';
import type { Catalog } from './api';
import { t } from './i18n';

const props = defineProps<{ catalog: Catalog; enabled: boolean }>();
const emit = defineEmits<{ insert: [value: string] }>();
const tab = ref<'controls' | 'variables'>('controls');
const search = ref('');
const items = computed(() => (tab.value === 'controls'
  ? props.catalog.controls.map((entry) => ({ name: '.' + entry.className, detail: entry.category, code: `\n.${entry.className} {\n  \n}\n` }))
  : props.catalog.variables.map((entry) => ({ name: entry.name, detail: entry.description, code: `\n:root {\n  ${entry.name}: ;\n}\n` })))
  .filter((entry) => (entry.name + entry.detail).toLowerCase().includes(search.value.toLowerCase())).slice(0, 150));
</script>

<template>
  <section class="catalog">
    <div class="panel-head"><button v-for="item in (['controls', 'variables'] as const)" :key="item" class="ui-btn v-ghost s-sm" :class="{ selected: tab === item }" @click="tab = item">{{ t(item) }}</button></div>
    <input v-model="search" class="field-input" :placeholder="t('search')" :aria-label="t('search')" />
    <div class="catalog-list">
      <button v-for="entry in items" :key="entry.name" class="catalog-entry ui-btn v-ghost s-sm" :disabled="!enabled" :title="entry.detail + '\n' + t('insert')" @click="emit('insert', entry.code)"><code>{{ entry.name }}</code><small>{{ entry.detail }}</small></button>
    </div>
  </section>
</template>
