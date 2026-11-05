//! Parsing of the `#[chronon_coordinator_macros::script(...)]` attribute arguments.

use syn::{parenthesized, parse::Parse, parse::ParseStream, LitStr, Token};

#[cfg(feature = "default-job")]
use crate::script_default_job::DefaultJobSpec;

/// `#[chronon_coordinator_macros::script]` attributes: `name` plus optional `default_job(...)`.
pub(super) struct ScriptAttrs {
    pub(super) name: String,
    #[cfg(feature = "default-job")]
    pub(super) default_job: Option<DefaultJobSpec>,
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
