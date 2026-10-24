import { test } from 'node:test';
import assert from 'node:assert/strict';
import { listen, once, emit, emitTo } from '../preview/events.mjs';

test('预览事件支持一次订阅、退订和目标标签隔离', async () => {
  const received = [];
  const remove = await listen('theme', (event) => received.push(event.payload), { target: 'main' });
  await once('theme', (event) => received.push('once:' + event.payload));
  await emitTo('settings', 'theme', 'Light');
  await emitTo({ label: 'main' }, 'theme', 'Dark');
  await emit('theme', 'Light');
  remove();
  await emit('theme', 'Dark');
  assert.deepEqual(received, ['once:Light', 'Dark', 'Light']);
});
