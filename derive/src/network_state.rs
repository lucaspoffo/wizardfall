use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn network_state_impl(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let type_name = input.ident;

    let gen = quote! {
        impl NetworkState for #type_name {
            type State = Self;

            fn from_state(state: Self::State) -> Self {
                state
            }
            fn update_from_state(&mut self, state: Self::State) {
                let _ = std::mem::replace(self, state);
            }

            fn state(&self) -> Self::State {
                self.clone()
            }
        }
    };

    proc_macro::TokenStream::from(gen)
}
