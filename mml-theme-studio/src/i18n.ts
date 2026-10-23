import { ref } from 'vue';
import { messages, text } from '../lib/messages.mjs';
import type { Locale } from './api';

export const locale = ref<Locale>('zh-CN');
export type MessageKey = keyof typeof messages['zh-CN'];
export const t = (key: MessageKey, values: Record<string, string> = {}) => text(locale.value, key, values);
