use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, AttributeArgs, Item, Lit, Meta, MetaNameValue, NestedMeta, PathSegment,
};

fn make_error(error_message: &str) -> TokenStream {
    TokenStream::from(quote! {
        compile_error!(#error_message);
    })
}

#[proc_macro_attribute]
pub fn initcall(input: TokenStream, annotated_item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(annotated_item as Item);
    let attr_ast = parse_macro_input!(input as AttributeArgs);

    const MAX_PRIORITY: u32 = 4;
    const DEFAULT_PRIORITY: u32 = 0;

    let mut priority: Option<u32> = None;
    for args in attr_ast {
        match args {
            NestedMeta::Meta(Meta::NameValue(MetaNameValue {
                path,
                lit: Lit::Int(lit),
                ..
            })) => {
                if path.segments.len() != 1 {
                    return make_error("Attribute metadata must have the form `priority = 1`");
                }

                let PathSegment { ident, .. } = path.segments.first().unwrap();
                if ident == "priority" {
                    let parsed_priority: u32 = lit
                        .base10_parse()
                        .expect("Expected an integer literal for `priority`");
                    if parsed_priority > MAX_PRIORITY {
                        let error_code = format!("Priority must be between 0 and {}", MAX_PRIORITY);
                        return make_error(&error_code);
                    }
                    priority = Some(parsed_priority);
                } else {
                    return make_error("Only the `priority` attribute is currently supported");
                }
            }
            _ => {
                return make_error(
                    "The initcall attribute must be of the form `#[initcall(priority = 1)]`",
                );
            }
        }
    }

    if let Item::Fn(function) = ast {
        let name_ident = function.sig.ident;
        let name = name_ident.to_string();

        let func_block = function.block;

        let priority = priority.unwrap_or(DEFAULT_PRIORITY);

        TokenStream::from(quote! {
            #[cfg(all(target_arch = "aarch64", target_os = "none"))]
            core::arch::global_asm!(
                core::concat!(".section .initcall.prio", #priority, ".", #name, ", \"a\""),
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
