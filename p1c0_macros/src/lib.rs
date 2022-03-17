use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Item};

#[proc_macro_attribute]
pub fn initcall(_input: TokenStream, annotated_item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(annotated_item as Item);

    if let Item::Fn(function) = ast {
        let name_ident = function.sig.ident;
        let name = name_ident.to_string();

        let func_block = function.block;

        TokenStream::from(quote! {
            #[cfg(all(target_arch = "aarch64", target_os = "none"))]
            core::arch::global_asm!(
                core::concat!(".section .initcall.", #name, ", \"a\""),
                core::concat!(".quad ", #name),
                ".previous"
            );

            #[cfg_attr(all(target_arch = "aarch64", target_os = "none"), link_section = core::concat!(".init.", #name))]
            #[no_mangle]
            extern "C" fn #name_ident() {
                #func_block
            }
        })
    } else {
        TokenStream::from(quote! {
            compile_error!("initcall must be applied to a function")
        })
    }
}
