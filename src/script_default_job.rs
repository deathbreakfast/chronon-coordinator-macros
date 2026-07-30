//! Optional `default_job(...)` attribute expansion (template product hosts).

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse::Parse, parse::ParseStream, LitStr, Token};

/// Schedule for a co-located default Chronon job row.
pub enum DefaultJobSchedule {
    Cron(String),
    RunOnce,
    Manual,
}

/// `default_job(job = "...", ...)` inside `#[chronon_coordinator_macros::script(...)]`.
pub struct DefaultJobSpec {
    pub job_name: String,
    pub schedule: DefaultJobSchedule,
}

impl Parse for DefaultJobSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut job_name: Option<String> = None;
        let mut schedule: Option<DefaultJobSchedule> = None;
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            if key == "job" {
                input.parse::<Token![=]>()?;
                let lit: LitStr = input.parse()?;
                if job_name.is_some() {
                    return Err(syn::Error::new(
                        key.span(),
                        "duplicate `job` in default_job(...)",
                    ));
                }
                job_name = Some(lit.value());
            } else if key == "cron" {
                input.parse::<Token![=]>()?;
                let lit: LitStr = input.parse()?;
                if schedule.is_some() {
                    return Err(syn::Error::new(
                        key.span(),
                        "only one schedule is allowed: `cron = \"...\"`, `run_once`, or `manual`",
                    ));
                }
                schedule = Some(DefaultJobSchedule::Cron(lit.value()));
            } else if key == "run_once" {
                if schedule.is_some() {
                    return Err(syn::Error::new(
                        key.span(),
                        "only one schedule is allowed: `cron = \"...\"`, `run_once`, or `manual`",
                    ));
                }
                schedule = Some(DefaultJobSchedule::RunOnce);
            } else if key == "manual" {
                if schedule.is_some() {
                    return Err(syn::Error::new(
                        key.span(),
                        "only one schedule is allowed: `cron = \"...\"`, `run_once`, or `manual`",
                    ));
                }
                schedule = Some(DefaultJobSchedule::Manual);
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    "expected `job`, `cron`, `run_once`, or `manual` inside default_job(...)",
                ));
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(Self {
            job_name: job_name.ok_or_else(|| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "default_job(...) requires `job = \"...\"`",
                )
            })?,
            schedule: schedule.ok_or_else(|| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "default_job(...) requires one of `cron = \"...\"`, `run_once`, or `manual`",
                )
            })?,
        })
    }
}

pub fn expand_default_job(
    dj: &DefaultJobSpec,
    fn_name: &syn::Ident,
    script_type_name: &syn::Ident,
) -> TokenStream2 {
    let ensure_ident = syn::Ident::new(
        &format!("__chronon_default_job_ensure_{fn_name}"),
        fn_name.span(),
    );
    let job_lit = LitStr::new(&dj.job_name, fn_name.span());
    let schedule_tokens = match &dj.schedule {
        DefaultJobSchedule::Cron(expr) => {
            let cron_lit = LitStr::new(expr, fn_name.span());
            quote! { .cron(#cron_lit)? }
        }
        DefaultJobSchedule::RunOnce => quote! { .run_once_at(::chrono::Utc::now()) },
        DefaultJobSchedule::Manual => quote! { .manual() },
    };
    quote! {
        fn #ensure_ident(
            backend: ::std::sync::Arc<dyn ::chronon_coordinator::ChrononCoordinatorBackend>,
            valence: ::valence::Valence,
        ) -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = ::anyhow::Result<()>> + Send>> {
            ::std::boxed::Box::pin(async move {
                let backend_ref: &(dyn ::chronon_coordinator::ChrononCoordinatorBackend) = backend.as_ref();
                if #script_type_name::get_job_by_name(backend_ref, #job_lit).await.is_err() {
                    #script_type_name::scheduler(backend_ref, valence)
                        .name(#job_lit)
                        #schedule_tokens
                        .add()
                        .await
                        .map_err(|e| ::anyhow::anyhow!("{}", e))?;
                    ::log::info!(
                        target: "chronon_default_jobs",
                        "Registered default job {}",
                        #job_lit,
                    );
                }
                Ok(())
            })
        }

        ::chronon_coordinator::inventory::submit! {
            ::chronon_coordinator::DefaultJobDescriptor {
                job_name: #job_lit,
                ensure: #ensure_ident,
            }
        }
    }
}
