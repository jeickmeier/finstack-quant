// Fluent mutators update the receiver in place and return it, as the Python
// twins do (Rust `MarketContext::insert_*` and the schedule builder return the
// builder). wasm-bindgen cannot return the receiver of a `&mut self` method,
// so the Rust methods return nothing and `this` is returned here.
export const chain = (cls, names) => {
  for (const name of names) {
    const raw = cls.prototype[name];
    cls.prototype[name] = {
      [name](...args) {
        raw.apply(this, args);
        return this;
      },
    }[name];
  }
};
