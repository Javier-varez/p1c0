use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    braced, parse_macro_input, AttributeArgs, Ident, Item, Lit, Meta, MetaNameValue, NestedMeta,
    PathSegment,
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

        let mut static_name = name_ident.to_string().to_ascii_uppercase();
        static_name.push_str("_STATIC");
        let static_name_ident = syn::Ident::new(&static_name, name_ident.span());

        let func_block = function.block;

        let priority = priority.unwrap_or(DEFAULT_PRIORITY);

        TokenStream::from(quote! {
            #[cfg_attr(all(target_arch = "aarch64", target_os = "none"), link_section = core::concat!(".initcall.prio", #priority, ".", #name))]
            #[used]
            static #static_name_ident: extern "C" fn() = {
                #[cfg_attr(all(target_arch = "aarch64", target_os = "none"), link_section = core::concat!(".init.", #name))]
                #[no_mangle]
                extern "C" fn #name_ident() {
                    #func_block
                }
                #name_ident
            };
        })
    } else {
        TokenStream::from(quote! {
            compile_error!("initcall must be applied to a function")
        })
    }
}

struct Register {
    name: Option<syn::Ident>,
    ty: syn::Type,
}

impl Parse for Register {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Option<syn::Ident> = input.parse().ok();
        if name.is_none() {
            let _: syn::Token![_] = input.parse()?;
        }

        let _: syn::Token![:] = input.parse()?;
        let ty = input.parse()?;

        Ok(Register { name, ty })
    }
}

struct RegisterBank {
    name: Ident,
    registers: syn::punctuated::Punctuated<Register, syn::Token![,]>,
}

impl Parse for RegisterBank {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        let content;
        let _brace = braced!(content in input);

        let registers = content.parse_terminated(Register::parse)?;

        Ok(RegisterBank { name, registers })
    }
}

impl TryInto<proc_macro::TokenStream> for RegisterBank {
    type Error = syn::Error;

    fn try_into(self) -> Result<TokenStream, Self::Error> {
        let mut unused_fields = 0;

        let regs = self.registers.iter().map(|register| {
            let ty = &register.ty;
            let name = match &register.name {
                Some(name) => name.clone(),
                None => {
                    let name = format!("_unused{}", unused_fields);
                    unused_fields += 1;
                    syn::Ident::new(&name, register.ty.span())
                }
            };
            quote! {
                #name: #ty,
            }
        });
        let name = self.name;
        let code = quote! {
            #[repr(C)]
            struct #name {
                #(#regs)*
            }
        };

        Ok(code.into())
    }
}

#[proc_macro]
pub fn define_register_bank(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as RegisterBank);

    match ast.try_into() {
        Ok(stream) => stream,
        Err(error) => error.to_compile_error().into(),
    }
}
