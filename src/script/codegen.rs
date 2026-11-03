//! Token generation for the params struct, typed handle, script type and invoker.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Pat, PatType, Signature};

pub(super) fn generate_params_struct(
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

pub(super) fn generate_handle_function(
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

pub(super) fn generate_script_type_api(
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

pub(super) fn generate_internal_signature(fn_sig: &Signature, fn_name: &syn::Ident) -> Signature {
    let mut internal_sig = fn_sig.clone();
    internal_sig.ident = syn::Ident::new(&format!("__{fn_name}_impl"), fn_name.span());
    internal_sig
}

pub(super) fn generate_deserialization_code(
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

pub(super) fn generate_invoke_script(
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
pub(super) fn to_pascal_case(s: &str) -> String {
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

    #[test]
    fn to_pascal_case_happy() {
        assert_eq!(to_pascal_case("daily_highscores"), "DailyHighscores");
        assert_eq!(to_pascal_case("simple"), "Simple");
        assert_eq!(to_pascal_case("a_b_c"), "ABC");
    }
}
