import fs from 'node:fs';
const transform = (value, factor = 2) => {
  return value * factor;
};
export function execute() {
  return transform(4);
}
const lazy = import('./lazy.js');
