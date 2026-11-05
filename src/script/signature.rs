//! Validation of the annotated function signature and script signature metadata.

use quote::quote;
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use syn::spanned::Spanned;
use syn::{
    AngleBracketedGenericArguments, FnArg, GenericArgument, Pat, PatType, PathArguments,
    ReturnType, Signature, Type, TypePath, TypeTuple,
};

pub(super) fn build_signature_metadata(params: &[&PatType]) -> syn::Result<(String, u64)> {
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

pub(super) fn validate_signature(sig: &Signature) -> syn::Result<()> {
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

pub(super) fn collect_script_params(sig: &Signature) -> syn::Result<Vec<&PatType>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use syn::{parse_quote, Signature};

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
}
