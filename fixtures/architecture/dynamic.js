import { helper } from './present.js';

export async function load() {
  const lazy = import('./missing.js');
  return helper() + (await lazy).default;
}
