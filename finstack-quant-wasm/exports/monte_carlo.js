import * as wasm from '../pkg/finstack_quant_wasm.js';

export const monte_carlo = {
  // European pricing — MC
  priceEuropeanCall: wasm.priceEuropeanCall,
  priceEuropeanPut: wasm.priceEuropeanPut,
  priceHestonCall: wasm.priceHestonCall,
  priceHestonPut: wasm.priceHestonPut,
  // Path-dependent pricing — MC
  priceAsianCall: wasm.priceAsianCall,
  priceAsianPut: wasm.priceAsianPut,
  // LSMC pricing
  priceAmericanPut: wasm.priceAmericanPut,
  priceAmericanCall: wasm.priceAmericanCall,
  priceAmericanPutUnbiased: wasm.priceAmericanPutUnbiased,
  priceAmericanCallUnbiased: wasm.priceAmericanCallUnbiased,
  // Analytical
  blackScholesCall: wasm.blackScholesCall,
  blackScholesPut: wasm.blackScholesPut,
};
