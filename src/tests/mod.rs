#[cfg(any(feature = "hsm", feature = "fsm"))]
mod common;

#[cfg(feature = "fsm")]
mod fsm_tests;
#[cfg(feature = "hsm")]
mod hsm_basic_tests;
#[cfg(feature = "hsm")]
mod hsm_chain_tests;
#[cfg(feature = "hsm")]
mod hsm_guard_tests;
#[cfg(feature = "hsm")]
mod hsm_interrupt_tests;
#[cfg(feature = "hsm")]
mod hsm_misc_tests;
#[cfg(all(feature = "hsm", feature = "history"))]
mod regression_tests;
