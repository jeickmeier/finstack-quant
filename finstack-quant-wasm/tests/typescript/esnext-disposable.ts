import { core } from '../../index.js';

declare module '../../index.js' {
  interface WasmOwned extends Disposable {
    [Symbol.dispose](): void;
  }
}

const currency = new core.Currency('USD');
currency[Symbol.dispose]();
