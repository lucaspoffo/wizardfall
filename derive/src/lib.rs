mod network_state;

use network_state::network_state_impl;

#[proc_macro_derive(NetworkState)]
pub fn network_state_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    network_state_impl(input)
}

