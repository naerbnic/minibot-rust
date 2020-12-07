//! Fake implementations of services. These generally do not talk to external services, and do not
//! persist any data. They're useful for testing.

pub mod account;
pub mod mq;
pub mod token_store;
