extern crate proc_macro;

mod guard_condition;

use proc_macro::TokenStream;

#[proc_macro]
pub fn combination_condition(item: TokenStream)-> TokenStream { 
    guard_condition::guard_condition_impl(item)
}