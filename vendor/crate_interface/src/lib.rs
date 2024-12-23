#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::parse::{Error, Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{parenthesized, parse_macro_input, Token};
use syn::{
    Expr, FnArg, ImplItem, ImplItemFn, ItemImpl, ItemTrait, Path, PathArguments, PathSegment,
    TraitItem, Type,
};

fn compiler_error(err: Error) -> TokenStream {
    err.to_compile_error().into()
}

/// Define an interface.
///
/// This attribute should be added above the definition of a trait. All traits
/// that use the attribute cannot have the same name.
///
/// It is not necessary to define it in the same crate as the implementation,
/// but it is required that these crates are linked together.
///
/// See the [crate-level documentation](crate) for more details.
#[proc_macro_attribute]
pub fn def_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return compiler_error(Error::new(
            Span::call_site(),
            "expect an empty attribute: `#[crate_interface_def]`",
        ));
    }

    let ast = syn::parse_macro_input!(item as ItemTrait);
    let trait_name = &ast.ident;
    let vis = &ast.vis;

    let mut extern_fn_list = vec![];
    for item in &ast.items {
        if let TraitItem::Fn(method) = item {
            let mut sig = method.sig.clone();
            let fn_name = &sig.ident;
            sig.ident = format_ident!("__{}_{}", trait_name, fn_name);
            sig.inputs = syn::punctuated::Punctuated::new();

            for arg in &method.sig.inputs {
                if let FnArg::Typed(_) = arg {
                    sig.inputs.push(arg.clone());
                }
            }

            let extern_fn = quote! {
                pub #sig;
            };
            extern_fn_list.push(extern_fn);
        }
    }

    let mod_name = format_ident!("__{}_mod", trait_name);
    quote! {
        #ast

        #[doc(hidden)]
        #[allow(non_snake_case)]
        #vis mod #mod_name {
            use super::*;
            extern "Rust" {
                #(#extern_fn_list)*
            }
        }
    }
    .into()
}

/// Implement the interface for a struct.
///
/// This attribute should be added above the implementation of a trait for a
/// struct, and the trait must be defined with
/// [`#[def_interface]`](macro@crate::def_interface).
///
/// It is not necessary to implement it in the same crate as the definition, but
/// it is required that these crates are linked together.
///
/// See the [crate-level documentation](crate) for more details.
#[proc_macro_attribute]
pub fn impl_interface(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return compiler_error(Error::new(
            Span::call_site(),
            "expect an empty attribute: `#[crate_interface_impl]`",
        ));
    }

    let mut ast = syn::parse_macro_input!(item as ItemImpl);
    let trait_name = if let Some((_, path, _)) = &ast.trait_ {
        &path.segments.last().unwrap().ident
    } else {
        return compiler_error(Error::new_spanned(ast, "expect a trait implementation"));
    };
    let impl_name = if let Type::Path(path) = &ast.self_ty.as_ref() {
        path.path.get_ident().unwrap()
    } else {
        return compiler_error(Error::new_spanned(ast, "expect a trait implementation"));
    };

    for item in &mut ast.items {
        if let ImplItem::Fn(method) = item {
            let (attrs, vis, sig, stmts) =
                (&method.attrs, &method.vis, &method.sig, &method.block.stmts);
            let fn_name = &sig.ident;
            let extern_fn_name = format_ident!("__{}_{}", trait_name, fn_name).to_string();

            let mut new_sig = sig.clone();
            new_sig.ident = format_ident!("{}", extern_fn_name);
            new_sig.inputs = syn::punctuated::Punctuated::new();

            let mut args = vec![];
            let mut has_self = false;
            for arg in &sig.inputs {
                match arg {
                    FnArg::Receiver(_) => has_self = true,
                    FnArg::Typed(ty) => {
                        args.push(ty.pat.clone());
                        new_sig.inputs.push(arg.clone());
                    }
                }
            }

            let call_impl = if has_self {
                quote! {
                    let _impl: #impl_name = #impl_name;
                    _impl.#fn_name( #(#args),* )
                }
            } else {
                quote! { #impl_name::#fn_name( #(#args),* ) }
            };

            let item = quote! {
                #(#attrs)*
                #vis
                #sig
                {
                    {
                        #[export_name = #extern_fn_name]
                        extern "Rust" #new_sig {
                            #call_impl
                        }
                    }
                    #(#stmts)*
                }
            }
            .into();
            *method = syn::parse_macro_input!(item as ImplItemFn);
        }
    }

    quote! { #ast }.into()
}

struct CallInterface {
    path: Path,
    args: Punctuated<Expr, Token![,]>,
}

impl Parse for CallInterface {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        let path: Path = input.parse()?;
        let args = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            input.parse_terminated(Expr::parse, Token![,])?
        } else if !input.is_empty() {
            parenthesized!(content in input);
            content.parse_terminated(Expr::parse, Token![,])?
        } else {
            Punctuated::new()
        };
        Ok(CallInterface { path, args })
    }
}

/// Call a function in the interface.
///
/// It is not necessary to call it in the same crate as the implementation, but
/// it is required that these crates are linked together.
///
/// See the [crate-level documentation](crate) for more details.
#[proc_macro]
pub fn call_interface(item: TokenStream) -> TokenStream {
    let call = parse_macro_input!(item as CallInterface);
    let args = call.args;
    let mut path = call.path.segments;

    if path.len() < 2 {
        compiler_error(Error::new(Span::call_site(), "expect `Trait::func`"));
    }
    let fn_name = path.pop().unwrap();
    let trait_name = path.pop().unwrap();
    let extern_fn_name = format_ident!("__{}_{}", trait_name.value().ident, fn_name.value().ident);

    path.push_value(PathSegment {
        ident: format_ident!("__{}_mod", trait_name.value().ident),
        arguments: PathArguments::None,
    });
    quote! { unsafe { #path :: #extern_fn_name( #args ) } }.into()
}
