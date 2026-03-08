extern crate proc_macro;

mod guard_condition;
mod hsm;
mod fsm;
mod hsm_tree;
mod fsm_graph;
mod state_config;
mod kw;

use proc_macro::TokenStream;

#[proc_macro]
pub fn combination_condition(item: TokenStream) -> TokenStream {
    guard_condition::guard_condition_impl(item)
}

#[proc_macro]
pub fn hsm(item: TokenStream) -> TokenStream {
    hsm::hsm_impl(item)
}

#[proc_macro]
pub fn hsm_tree(item: TokenStream) -> TokenStream {
    hsm_tree::hsm_tree_impl(item)
}

#[proc_macro]
pub fn fsm(item: TokenStream) -> TokenStream {
    fsm::fsm_impl(item)
}

#[proc_macro]
pub fn fsm_graph(item: TokenStream) -> TokenStream {
    fsm_graph::fsm_graph_impl(item)
}