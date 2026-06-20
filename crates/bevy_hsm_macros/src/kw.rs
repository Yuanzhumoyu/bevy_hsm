//! Custom keywords and punctuation used by the proc-macro parsers.
//!
//! Each entry defines a keyword recognized in `hsm!`, `fsm!`, `hsm_tree!`,
//! `fsm_graph!`, and `combination_condition!` macro input.

// --- Guard condition operators ---
syn::custom_keyword!(and);
syn::custom_keyword!(not);
syn::custom_keyword!(or);

// --- FSM / FSM-graph structure keywords ---
syn::custom_keyword!(states);
syn::custom_keyword!(components);
syn::custom_keyword!(transitions);

// --- Transition condition keywords ---
syn::custom_keyword!(guard);
syn::custom_keyword!(event);

// --- State attribute keys ---
syn::custom_keyword!(fsm_blueprint);
syn::custom_keyword!(guard_enter);
syn::custom_keyword!(guard_exit);
syn::custom_keyword!(before_enter);
syn::custom_keyword!(after_enter);
syn::custom_keyword!(on_update);
syn::custom_keyword!(before_exit);
syn::custom_keyword!(after_exit);
syn::custom_keyword!(minimal);
syn::custom_keyword!(strategy);
syn::custom_keyword!(behavior);
syn::custom_keyword!(state_scene);

// --- Transition direction punctuation ---
syn::custom_punctuation!(Both, <=>);

// --- Machine config keys ---
syn::custom_keyword!(history_capacity);
syn::custom_keyword!(init_state);
syn::custom_keyword!(curr_state);

// --- init(…) config wrapper ---
syn::custom_keyword!(init);
