import { save as persist } from './store';
export class Service extends Base {
  private value: number = 0;
  run(input: number, enabled = true): number {
    if (enabled) {
      this.value = persist(input);
    }
    return this.value;
  }
}
