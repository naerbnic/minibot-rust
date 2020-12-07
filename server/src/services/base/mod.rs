//! These are the base interfaces for the services minibot depends on.

// A common macro to create handles for services

macro_rules! define_handle {
    ($name:ident, $base:ident) => {
        #[derive(::gotham_derive::StateData)]
        pub struct $name (::std::sync::Arc<dyn $base + Send + Sync>);

        impl ::std::clone::Clone for $name {
            fn clone(&self) -> Self {
                $name (self.0.clone())
            }
        }
    };
}

macro_rules! define_deref_handle {
    ($name:ident, $base:ident) => {
        define_handle!($name, $base);

        impl ::std::ops::Deref for $name {
            type Target = dyn $base;

            fn deref(&self) -> &Self::Target {
                &*self.0
            }
        }
    }
}

pub mod account;
pub mod mq;
pub mod token_store;
