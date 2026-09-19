//! 四个派生宏的公共实现。

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{Attribute, Data, DeriveInput, Expr, Fields, Ident, Meta, Token};

/// 四层之一。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Controller,
    System,
    Model,
    Utility,
}

impl Layer {
    /// 层接口的名字，例如 `IModel`。
    pub fn layer_trait(self) -> Ident {
        Ident::new(self.layer_trait_name(), Span::call_site())
    }

    fn layer_trait_name(self) -> &'static str {
        match self {
            Layer::Controller => "IController",
            Layer::System => "ISystem",
            Layer::Model => "IModel",
            Layer::Utility => "IUtility",
        }
    }

    /// 承载生命周期钩子的辅助属性名，例如 `#[model(init = ...)]`。
    fn helper_attribute(self) -> &'static str {
        match self {
            Layer::Controller => "controller",
            Layer::System => "system",
            Layer::Model => "model",
            Layer::Utility => "utility",
        }
    }

    /// 该层拥有的能力接口。
    fn capabilities(self) -> &'static [&'static str] {
        match self {
            Layer::Controller => &[
                "ICanGetModel",
                "ICanGetSystem",
                "ICanSendCommand",
                "ICanSendQuery",
                "ICanRegisterEvent",
            ],
            Layer::System => &[
                "ICanGetModel",
                "ICanGetSystem",
                "ICanGetUtility",
                "ICanSendEvent",
                "ICanRegisterEvent",
            ],
            Layer::Model => &["ICanGetUtility", "ICanSendEvent"],
            Layer::Utility => &[],
        }
    }

    /// 是否需要 `arch: ArchRef` 字段。
    fn needs_arch(self) -> bool {
        !matches!(self, Layer::Utility)
    }
}

/// 展开派生宏，出错时输出 `compile_error!`。
pub fn expand(input: DeriveInput, layer: Layer) -> TokenStream {
    match try_expand(input, layer) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error(),
    }
}

fn try_expand(input: DeriveInput, layer: Layer) -> syn::Result<TokenStream> {
    let struct_name = input.ident.clone();

    let arch_field = if layer.needs_arch() {
        Some(find_arch_field(&input, layer)?)
    } else {
        None
    };

    let lifecycle = parse_lifecycle(&input.attrs, layer)?;

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // --- 架构引用相关实现 -------------------------------------------------
    let arch_impls = arch_field.as_ref().map(|arch| {
        quote! {
            impl #impl_generics ::qframework_core::HasArchRef for #struct_name #ty_generics #where_clause {
                fn arch_ref(&self) -> &::qframework_core::ArchRef {
                    &self.#arch
                }
            }

            impl #impl_generics ::qframework_core::ICanGetArchitecture for #struct_name #ty_generics #where_clause {
                fn architecture(&self) -> ::std::sync::Arc<::qframework_core::Architecture> {
                    self.#arch.get()
                }
            }
        }
    });

    // --- 能力接口实现 -----------------------------------------------------
    let capability_impls = layer.capabilities().iter().map(|capability| {
        let capability = Ident::new(capability, Span::call_site());
        quote! {
            impl #impl_generics ::qframework_core::#capability for #struct_name #ty_generics #where_clause {}
        }
    });

    // --- 层接口实现 -------------------------------------------------------
    // 泛型结构体需要额外保证 `Send + Sync + 'static`（层接口的 supertrait）。
    let mut layer_generics = input.generics.clone();
    if !layer_generics.params.is_empty() {
        let predicate: syn::WherePredicate = syn::parse_quote!(
            #struct_name #ty_generics: ::core::marker::Send + ::core::marker::Sync + 'static
        );
        layer_generics
            .make_where_clause()
            .predicates
            .push(predicate);
    }
    let (layer_impl_generics, layer_ty_generics, layer_where_clause) =
        layer_generics.split_for_impl();

    let layer_trait = layer.layer_trait();

    let init = lifecycle.init.map(|expr| {
        quote! {
            fn init(&self) {
                let hook = #expr;
                hook(self);
            }
        }
    });

    let deinit = lifecycle.deinit.map(|expr| {
        quote! {
            fn deinit(&self) {
                let hook = #expr;
                hook(self);
            }
        }
    });

    Ok(quote! {
        #arch_impls

        #(#capability_impls)*

        impl #layer_impl_generics ::qframework_core::#layer_trait for #struct_name #layer_ty_generics #layer_where_clause {
            #init
            #deinit
        }
    })
}

/// 找到用于注入架构引用的字段：优先 `#[arch]` 标记，其次名为 `arch` 的字段。
fn find_arch_field(input: &DeriveInput, layer: Layer) -> syn::Result<Ident> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            format!("#[derive({})] 只能用于结构体", layer.layer_trait_name()),
        ));
    };

    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            format!(
                "#[derive({})] 需要具名字段（包含 `arch: ArchRef`）",
                layer.layer_trait_name()
            ),
        ));
    };

    if let Some(field) = fields
        .named
        .iter()
        .find(|field| field.attrs.iter().any(|attr| attr.path().is_ident("arch")))
    {
        return Ok(field.ident.clone().expect("具名字段一定有标识符"));
    }

    if let Some(field) = fields
        .named
        .iter()
        .find(|field| field.ident.as_ref().is_some_and(|ident| ident == "arch"))
    {
        return Ok(field.ident.clone().expect("具名字段一定有标识符"));
    }

    Err(syn::Error::new_spanned(
        &input.ident,
        format!(
            "#[derive({})] 找不到架构引用字段：请添加 `arch: ArchRef`，或用 `#[arch]` 标记已有字段",
            layer.layer_trait_name()
        ),
    ))
}

/// 解析 `#[model(init = ..., deinit = ...)]` 这样的辅助属性。
fn parse_lifecycle(attrs: &[Attribute], layer: Layer) -> syn::Result<Lifecycle> {
    let helper = layer.helper_attribute();
    let mut lifecycle = Lifecycle::default();

    for attr in attrs {
        if !attr.path().is_ident(helper) {
            continue;
        }

        let entries = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;

        for entry in entries {
            let Meta::NameValue(name_value) = &entry else {
                return Err(syn::Error::new_spanned(
                    &entry,
                    format!("`#[{helper}(...)]` 只支持 `init = <表达式>` 与 `deinit = <表达式>`"),
                ));
            };

            if name_value.path.is_ident("init") {
                lifecycle.init = Some(name_value.value.clone());
            } else if name_value.path.is_ident("deinit") {
                lifecycle.deinit = Some(name_value.value.clone());
            } else {
                return Err(syn::Error::new_spanned(
                    &name_value.path,
                    format!("`#[{helper}(...)]` 只支持 `init` 与 `deinit`"),
                ));
            }
        }
    }

    Ok(lifecycle)
}

#[derive(Default)]
struct Lifecycle {
    init: Option<Expr>,
    deinit: Option<Expr>,
}
