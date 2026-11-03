//! Implementation of the `#[chronon_coordinator_macros::script]` proc macro.

mod attrs;
mod codegen;
mod signature;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{ItemFn, LitStr};

use attrs::ScriptAttrs;
use codegen::{
    generate_deserialization_code, generate_handle_function, generate_internal_signature,
    generate_invoke_script, generate_params_struct, generate_script_type_api, to_pascal_case,
};
use signature::{build_signature_metadata, collect_script_params, validate_signature};

#[cfg(feature = "default-job")]
use crate::script_default_job::expand_default_job;

pub fn script_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    match script_impl_impl(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn script_impl_impl(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    let attrs: ScriptAttrs = syn::parse2(attr)?;
    let input: ItemFn = syn::parse2(item)?;
    validate_signature(&input.sig)?;
    expand_script(attrs, &input)
}

fn expand_script(attrs: ScriptAttrs, input: &ItemFn) -> syn::Result<TokenStream2> {
    let script_name = attrs.name;
    let fn_name = &input.sig.ident;
    let fn_vis = &input.vis;
    let fn_block = &input.block;
    let fn_attrs = &input.attrs;
    let fn_sig = &input.sig;
    let params = collect_script_params(fn_sig)?;
    let (signature_json, signature_hash) = build_signature_metadata(&params)?;

    let fn_name_pascal = to_pascal_case(&fn_name.to_string());
    let struct_name_str = format!("{fn_name_pascal}Params");
    let params_struct_name = syn::Ident::new(&struct_name_str, fn_name.span());
    let script_type_name_str = if fn_name_pascal.ends_with("Script") {
        fn_name_pascal
    } else {
        format!("{fn_name_pascal}Script")
    };
    let script_type_name = syn::Ident::new(&script_type_name_str, fn_name.span());
    let is_unit_struct = params.is_empty();
    let params_struct =
        generate_params_struct(fn_vis, &params_struct_name, &params, is_unit_struct);
    let handle_fn = generate_handle_function(fn_vis, fn_name, &params_struct_name, &script_name);
    let script_type_api =
        generate_script_type_api(fn_vis, &script_type_name, &params_struct_name, &script_name);
    let internal_sig = generate_internal_signature(fn_sig, fn_name);
    let internal_fn_name = &internal_sig.ident;
    let deserialize_code = generate_deserialization_code(&params_struct_name, is_unit_struct);
    let invoke_script = generate_invoke_script(internal_fn_name, &params, is_unit_struct)?;
    let signature_json_lit = LitStr::new(&signature_json, fn_name.span());

    let default_job_tokens = {
        #[cfg(feature = "default-job")]
        {
            attrs.default_job.as_ref().map_or_else(
                || quote! {},
                |dj| expand_default_job(dj, fn_name, &script_type_name),
            )
        }
        #[cfg(not(feature = "default-job"))]
        {
            quote! {}
        }
    };

    Ok(quote! {
        #params_struct

        #handle_fn

        #script_type_api

        #(#fn_attrs)*
        #fn_vis #internal_sig #fn_block

        ::chronon_coordinator::inventory::submit! {
            ::chronon_executor::ScriptDescriptor::with_signature(
                #script_name,
                |ctx, params_json| {
                    ::std::boxed::Box::pin(async move {
                        #deserialize_code
                        #invoke_script
                    })
                },
                #signature_json_lit,
                #signature_hash,
            )
        }

        #default_job_tokens
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_impl_impl_rejects_non_identifier_params_sad() {
        let error = script_impl_impl(
            quote!(name = "bad_params"),
            quote! {
                pub async fn bad_params(
                    ctx: Box<dyn chronon_core::ScriptContext>,
                    (a, b): (String, String),
                ) -> anyhow::Result<()> {
                    let _ = (ctx, a, b);
                    Ok(())
                }
            },
        )
        .expect_err("destructured parameters must fail");
        assert!(error
            .to_string()
            .contains("parameters after ScriptContext must be simple identifiers"));
    }

    #[test]
    fn script_impl_impl_expands_valid_function_happy() {
        let tokens = script_impl_impl(
            quote!(name = "daily_cleanup"),
            quote! {
                pub async fn daily_cleanup(
                    ctx: Box<dyn chronon_core::ScriptContext>,
                    dry_run: bool,
                ) -> anyhow::Result<()> {
                    let _ = (ctx, dry_run);
                    Ok(())
                }
            },
        )
        .expect("valid script should expand");

        let expanded = tokens.to_string();
        assert!(expanded.contains("struct DailyCleanupParams"));
        assert!(expanded.contains("ScriptHandle"));
        assert!(expanded.contains("struct DailyCleanupScript"));
        assert!(expanded.contains("fn scheduler"));
        assert!(expanded.contains("fn get_job_by_name"));
        assert!(expanded.contains("ScriptDescriptor"));
        assert!(expanded.contains("dry_run"));
    }

    #[test]
    fn script_impl_impl_expands_default_job_happy() {
        let tokens = script_impl_impl(
            quote! {
                name = "tick",
                default_job(job = "tick-job", cron = "*/5 * * * *")
            },
            quote! {
                pub async fn tick(ctx: Box<dyn chronon_core::ScriptContext>) -> anyhow::Result<()> {
                    let _ = ctx;
                    Ok(())
                }
            },
        )
        .expect("script with default_job should expand");

        let expanded = tokens.to_string();
        assert!(expanded.contains("DefaultJobDescriptor"));
        assert!(expanded.contains("__chronon_default_job_ensure_tick"));
        assert!(expanded.contains("tick-job"));
    }

    #[test]
    fn script_impl_impl_rejects_missing_name_attribute_sad() {
        let error = script_impl_impl(
            quote!(),
            quote! {
                pub async fn missing_name(ctx: Box<dyn chronon_core::ScriptContext>) -> anyhow::Result<()> {
                    let _ = ctx;
                    Ok(())
                }
            },
        )
        .expect_err("missing name attribute must fail");

        let message = error.to_string();
        assert!(
            message.contains("missing `name = \"...\"`")
                || message.contains("expected `name`")
                || message.contains("unexpected end of input")
                || message.contains("unexpected token"),
            "unexpected parse error: {message}"
        );
    }
}
