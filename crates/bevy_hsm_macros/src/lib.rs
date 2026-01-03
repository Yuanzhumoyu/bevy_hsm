extern crate proc_macro;

mod combination_condition;

use proc_macro::TokenStream;

#[proc_macro]
pub fn combination_condition(item: TokenStream)-> TokenStream { 
    combination_condition::combination_condition_impl(item)
}