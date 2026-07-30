//! Implementation of the `#[chronon_coordinator_macros::script]` proc macro.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use syn::spanned::Spanned;
use syn::{
    parenthesized, parse::Parse, parse::ParseStream, AngleBracketedGenericArguments, FnArg,
    GenericArgument, ItemFn, LitStr, Pat, PatType, PathArguments, ReturnType, Signature, Token,
    Type, TypePath, TypeTuple,
};

#[cfg(feature = "default-job")]
use crate::script_default_job::{expand_default_job, DefaultJobSpec};

/// `#[chronon_coordinator_macros::script]` attributes: `name` plus optional `default_job(...)`.
struct ScriptAttrs {
    name: String,
    #[cfg(feature = "default-job")]
    default_job: Option<DefaultJobSpec>,
}

impl Parse for ScriptAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name: Option<String> = None;
        #[cfg(feature = "default-job")]
        let mut default_job: Option<DefaultJobSpec> = None;
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            if key == "name" {
                input.parse::<Token![=]>()?;
                let lit: LitStr = input.parse()?;
                if name.is_some() {
                    return Err(syn::Error::new(
                        key.span(),
                        "duplicate `name` in #[chronon_coordinator_macros::script]",
                    ));
                }
                name = Some(lit.value());
            } else if key == "default_job" {
                #[cfg(feature = "default-job")]
                {
                    let content;
                    parenthesized!(content in input);
                    if default_job.is_some() {
                        return Err(syn::Error::new(
                            key.span(),
                            "duplicate `default_job(...)` in #[chronon_coordinator_macros::script]",
                        ));
                    }
                    default_job = Some(content.parse()?);
                }
                #[cfg(not(feature = "default-job"))]
                {
                    return Err(syn::Error::new(
                        key.span(),
                        "`default_job(...)` requires the `default-job` feature on chronon-coordinator-macros",
                    ));
                }
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    "expected `name` or `default_job` in #[chronon_coordinator_macros::script]",
                ));
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        let name = name.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "missing `name = \"...\"` for #[chronon_coordinator_macros::script]",
            )
        })?;
        Ok(Self {
            name,
            #[cfg(feature = "default-job")]
            default_job,
        })
    }
}

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

fn build_signature_metadata(params: &[&PatType]) -> syn::Result<(String, u64)> {
    let mut signature = BTreeMap::new();
    for pat_type in params {
        let Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
            return Err(syn::Error::new(
                pat_type.pat.span(),
                "#[chronon_coordinator_macros::script] parameters after ScriptContext must be simple identifiers",
            ));
        };
        let name = pat_ident.ident.to_string();
        let ty_tokens = &pat_type.ty;
        let ty = quote! { #ty_tokens }.to_string();
        signature.insert(name, ty);
    }
    let signature_json = serde_json::to_string(&signature).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("failed to build script signature metadata: {e}"),
        )
    })?;
    let mut hasher = DefaultHasher::new();
    signature_json.hash(&mut hasher);
    Ok((signature_json, hasher.finish()))
}

fn validate_signature(sig: &Signature) -> syn::Result<()> {
    if sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            sig,
            "#[chronon_coordinator_macros::script] function must be async",
        ));
    }
    validate_first_parameter(sig)?;
    validate_return_type(sig)?;
    Ok(())
}

fn validate_first_parameter(sig: &Signature) -> syn::Result<()> {
    let first_param = sig.inputs.first().ok_or_else(|| {
        syn::Error::new_spanned(
            sig,
            "#[chronon_coordinator_macros::script] function must accept Box<dyn ScriptContext> as the first parameter",
        )
    })?;

    let FnArg::Typed(pat_type) = first_param else {
        return Err(syn::Error::new_spanned(
            first_param,
            "#[chronon_coordinator_macros::script] methods are not supported; use a free function",
        ));
    };

    if !matches!(pat_type.pat.as_ref(), Pat::Ident(_)) {
        return Err(syn::Error::new_spanned(
            &pat_type.pat,
            "#[chronon_coordinator_macros::script] first parameter must be a named ScriptContext binding",
        ));
    }

    if !is_script_context_param(pat_type.ty.as_ref()) {
        return Err(syn::Error::new_spanned(
            &pat_type.ty,
            "#[chronon_coordinator_macros::script] first parameter must be Box<dyn ScriptContext>",
        ));
    }

    Ok(())
}

fn validate_return_type(sig: &Signature) -> syn::Result<()> {
    match &sig.output {
        ReturnType::Type(_, ty) if is_result_unit(ty.as_ref()) => Ok(()),
        _ => Err(syn::Error::new_spanned(
            sig,
            "#[chronon_coordinator_macros::script] return type must be Result<()> (for example anyhow::Result<()>)",
        )),
    }
}

fn is_script_context_param(ty: &Type) -> bool {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return false;
    };
    let mut segments = path.segments.iter();
    let Some(box_seg) = segments.next() else {
        return false;
    };
    if box_seg.ident != "Box" {
        return false;
    }
    let PathArguments::AngleBracketed(args) = &box_seg.arguments else {
        return false;
    };
    let Some(GenericArgument::Type(Type::TraitObject(trait_obj))) = args.args.first() else {
        return false;
    };
    trait_obj.bounds.iter().any(|bound| match bound {
        syn::TypeParamBound::Trait(t) => t
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident == "ScriptContext"),
        _ => false,
    })
}

fn is_result_unit(ty: &Type) -> bool {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return false;
    };

    let Some(last_segment) = path.segments.last() else {
        return false;
    };

    if last_segment.ident != "Result" {
        return false;
    }

    let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
        &last_segment.arguments
    else {
        return false;
    };

    if args.len() != 1 {
        return false;
    }

    matches!(
        args.first(),
        Some(GenericArgument::Type(Type::Tuple(TypeTuple { elems, .. }))) if elems.is_empty()
    )
}

fn collect_script_params(sig: &Signature) -> syn::Result<Vec<&PatType>> {
    sig.inputs
        .iter()
        .skip(1)
        .map(|arg| match arg {
            FnArg::Typed(pat_type) => Ok(pat_type),
            FnArg::Receiver(receiver) => Err(syn::Error::new_spanned(
                receiver,
                "#[chronon_coordinator_macros::script] methods are not supported; use a free function",
            )),
        })
        .collect()
}

fn generate_params_struct(
    fn_vis: &syn::Visibility,
    params_struct_name: &syn::Ident,
    params: &[&PatType],
    is_unit_struct: bool,
) -> TokenStream2 {
    if is_unit_struct {
        quote! {
            /// Parameters struct for this script (no parameters).
            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            #fn_vis struct #params_struct_name;
        }
    } else {
        quote! {
            /// Parameters struct for this script.
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            #fn_vis struct #params_struct_name {
                #(#fn_vis #params),*
            }
        }
    }
}

fn generate_handle_function(
    fn_vis: &syn::Visibility,
    fn_name: &syn::Ident,
    params_struct_name: &syn::Ident,
    script_name: &str,
) -> TokenStream2 {
    quote! {
        /// Returns a typed handle for scheduling this script.
        #fn_vis fn #fn_name() -> ::chronon_core::ScriptHandle<#params_struct_name> {
            ::chronon_core::ScriptHandle::new(#script_name)
        }
    }
}

fn generate_script_type_api(
    fn_vis: &syn::Visibility,
    script_type_name: &syn::Ident,
    params_struct_name: &syn::Ident,
    script_name: &str,
) -> TokenStream2 {
    quote! {
        /// Macro-generated typed API surface for this script.
        #fn_vis struct #script_type_name;

        impl #script_type_name {
            /// Stable script registry name.
            pub const NAME: &'static str = #script_name;

            /// Return a typed script handle.
            pub fn handle() -> ::chronon_core::ScriptHandle<#params_struct_name> {
                ::chronon_core::ScriptHandle::new(Self::NAME)
            }

            /// Create a backend-bound typed scheduler API for this script.
            pub fn scheduler<'a>(
                backend: &'a dyn ::chronon_coordinator::ChrononCoordinatorBackend,
                valence: ::valence::Valence,
            ) -> ::chronon_coordinator::ScriptScheduler<'a, #params_struct_name> {
                ::chronon_coordinator::ScriptScheduler::new(backend, &Self::handle(), valence)
            }

            /// Resolve an existing job by name as a typed wrapper.
            pub async fn get_job_by_name<'a>(
                backend: &'a dyn ::chronon_coordinator::ChrononCoordinatorBackend,
                job_name: &str,
            ) -> ::chronon_coordinator::Result<::chronon_coordinator::TypedJobRef<'a, #params_struct_name>> {
                ::chronon_coordinator::typed_job_ref_for_script(backend, job_name, Self::NAME).await
            }
        }
    }
}

fn generate_internal_signature(fn_sig: &Signature, fn_name: &syn::Ident) -> Signature {
    let mut internal_sig = fn_sig.clone();
    internal_sig.ident = syn::Ident::new(&format!("__{fn_name}_impl"), fn_name.span());
    internal_sig
}

fn generate_deserialization_code(
    params_struct_name: &syn::Ident,
    is_unit_struct: bool,
) -> TokenStream2 {
    if is_unit_struct {
        quote! {
            let params: #params_struct_name =
                if params_json.is_object() && params_json.as_object().map(|o| o.is_empty()).unwrap_or(false) {
                    serde_json::from_value(serde_json::Value::Null)?
                } else {
                    serde_json::from_value(params_json)?
                };
        }
    } else {
        quote! {
            let params: #params_struct_name = serde_json::from_value(params_json)?;
        }
    }
}

fn generate_invoke_script(
    internal_fn_name: &syn::Ident,
    params: &[&PatType],
    is_unit_struct: bool,
) -> syn::Result<TokenStream2> {
    if is_unit_struct {
        return Ok(quote! {
            #internal_fn_name(ctx).await.map_err(|e| ::chronon_core::ChrononError::Internal(e.to_string()))
        });
    }

    let param_accessors = param_accessors(params)?;
    Ok(quote! {
        #internal_fn_name(ctx, #(#param_accessors),*).await.map_err(|e| ::chronon_core::ChrononError::Internal(e.to_string()))
    })
}

fn param_accessors(params: &[&PatType]) -> syn::Result<Vec<TokenStream2>> {
    params
        .iter()
        .map(|pat_type| {
            if let Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
                let ident = &pat_ident.ident;
                Ok(quote! { params.#ident })
            } else {
                Err(syn::Error::new(
                    pat_type.pat.span(),
                    "#[chronon_coordinator_macros::script] parameters after ScriptContext must be simple identifiers",
                ))
            }
        })
        .collect()
}

/// Converts `snake_case` to `PascalCase`.
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::{parse_quote, Signature};

    #[test]
    fn to_pascal_case_happy() {
        assert_eq!(to_pascal_case("daily_highscores"), "DailyHighscores");
        assert_eq!(to_pascal_case("simple"), "Simple");
        assert_eq!(to_pascal_case("a_b_c"), "ABC");
    }

    #[test]
    fn validate_signature_rejects_non_async_function_sad() {
        let sig: Signature = parse_quote! {
            fn not_async(ctx: Box<dyn chronon_core::ScriptContext>) -> anyhow::Result<()>
        };
        let error = validate_signature(&sig).expect_err("non-async function must fail");
        assert!(error.to_string().contains("must be async"));
    }

    #[test]
    fn validate_signature_rejects_first_param_not_context_sad() {
        let sig: Signature = parse_quote! {
            async fn wrong_first_param(user_id: String) -> anyhow::Result<()>
        };
        let error = validate_signature(&sig).expect_err("first param mismatch must fail");
        assert!(error
            .to_string()
            .contains("first parameter must be Box<dyn ScriptContext>"));
    }

    #[test]
    fn validate_signature_rejects_wrong_return_type_sad() {
        let sig: Signature = parse_quote! {
            async fn wrong_return_type(ctx: Box<dyn chronon_core::ScriptContext>) -> anyhow::Result<String>
        };
        let error = validate_signature(&sig).expect_err("wrong return type must fail");
        assert!(error.to_string().contains("return type must be Result<()>"));
    }

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
