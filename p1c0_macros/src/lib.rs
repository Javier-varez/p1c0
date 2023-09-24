use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
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
    offset: syn::LitInt,
    name: syn::Ident,
    ty: syn::Type,
}

impl Parse for Register {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _: syn::Token![<] = input.parse()?;
        let offset: syn::LitInt = input.parse()?;
        let _: syn::Token![>] = input.parse()?;

        let _: syn::Token![=>] = input.parse()?;

        let name: syn::Ident = input.parse()?;
        let _: syn::Token![:] = input.parse()?;
        let ty = input.parse()?;

        Ok(Register { offset, name, ty })
    }
}

struct RegisterBank {
    name: Ident,
    reg_size: syn::LitInt,
    registers: syn::punctuated::Punctuated<Register, syn::Token![,]>,
}

impl Parse for RegisterBank {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        let _: syn::Token![<] = input.parse()?;
        let reg_size: syn::LitInt = input.parse()?;
        let _: syn::Token![>] = input.parse()?;
        let content;
        let _brace = braced!(content in input);

        let registers = content.parse_terminated(Register::parse)?;

        Ok(RegisterBank {
            name,
            reg_size,
            registers,
        })
    }
}

impl RegisterBank {
    fn validate(&self) -> Result<(), syn::Error> {
        let reg_size: usize = self.reg_size.base10_parse()?;
        let mut regs: Vec<_> = self.registers.iter().collect();

        // Sort by offset
        regs.sort_by(|a, b| {
            let a: usize = a.offset.base10_parse().unwrap();
            let b: usize = b.offset.base10_parse().unwrap();
            a.cmp(&b)
        });

        let mut current_offset = 0;
        for register in regs {
            let offset: usize = register.offset.base10_parse().unwrap();
            if (offset % reg_size) != 0 {
                let error_message = format!("Register `{}` is unaligned", register.name);
                return Err(syn::Error::new(register.offset.span(), error_message));
            }

            if offset < current_offset {
                let error_message = format!("Register `{}` overlaps with another", register.name);
                return Err(syn::Error::new(register.offset.span(), error_message));
            }

            current_offset = offset + reg_size;
        }

        Ok(())
    }
}

impl TryInto<proc_macro::TokenStream> for RegisterBank {
    type Error = syn::Error;

    fn try_into(self) -> Result<TokenStream, Self::Error> {
        let bank_name = self.name;
        let reg_size: usize = self.reg_size.base10_parse()?;

        let mut regs: Vec<_> = self.registers.iter().collect();
        // Sort by offset
        regs.sort_by(|a, b| {
            let a: usize = a.offset.base10_parse().unwrap();
            let b: usize = b.offset.base10_parse().unwrap();
            a.cmp(&b)
        });

        let mut unused_fields = 0;
        let mut current_offset = 0;
        let mut fields = vec![];
        for register in regs {
            let offset: usize = register.offset.base10_parse().unwrap();

            if offset > current_offset {
                // Must insert unused field here
                let name = format!("_unused{}", unused_fields);
                let name = syn::Ident::new(&name, register.name.span());
                let size = offset - current_offset;
                fields.push(quote! {
                    #name: [u8; #size],
                });
                unused_fields += 1;
            }

            let name = &register.name;
            let ty = &register.ty;
            fields.push(quote! {
                pub #name: #ty,
            });

            current_offset = offset + reg_size;
        }

        let code = quote! {
            #[allow(non_snake_case)]
            mod #bank_name {
                use super::*;
                #[repr(C)]
                pub struct Bank {
                    #(#fields)*
                }
            }
        };

        Ok(code.into())
    }
}

#[proc_macro]
pub fn define_register_bank(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as RegisterBank);

    match ast.validate() {
        Ok(()) => {}
        Err(error) => {
            return error.to_compile_error().into();
        }
    }

    match ast.try_into() {
        Ok(stream) => stream,
        Err(error) => error.to_compile_error().into(),
    }
}
