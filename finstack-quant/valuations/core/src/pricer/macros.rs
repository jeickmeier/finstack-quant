//! Registration macros for instrument pricer families.

/// Register a [`GenericInstrumentPricer`](crate::instruments::pricing::GenericInstrumentPricer)
/// for an instrument type, collapsing the repetitive registration boilerplate
/// shared by every family table.
///
/// Two forms are supported:
///
/// - `register_generic!(registry, InstrumentType::Foo, crate::instruments::FooType);`
///   registers the discounting pricer (`GenericInstrumentPricer::<T>::discounting`)
///   under `ModelKey::Discounting`.
/// - `register_generic!(registry, InstrumentType::Foo, crate::instruments::FooType, ModelKey::Bar);`
///   registers `GenericInstrumentPricer::<T>::new(InstrumentType::Foo, ModelKey::Bar)`
///   under the explicit model key.
///
/// Both forms expand to a single `registry.register(...)` call.
#[macro_export]
macro_rules! register_generic {
    ($registry:expr_2021, $inst:expr_2021, $ty:ty $(,)?) => {
        $registry.register(
            $crate::instruments::pricing::GenericInstrumentPricer::<$ty>::discounting($inst),
        )?
    };
    ($registry:expr_2021, $inst:expr_2021, $ty:ty, $model:expr_2021 $(,)?) => {
        $registry.register(
            $crate::instruments::pricing::GenericInstrumentPricer::<$ty>::new($inst, $model),
        )?
    };
}
