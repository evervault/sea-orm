use super::*;

impl EntityWriter {
    #[allow(clippy::too_many_arguments)]
    pub fn gen_oxide_code_blocks(
        entity: &Entity,
        with_serde: &WithSerde,
        column_option: &ColumnOption,
        _schema_name: &Option<String>,
        serde_skip_deserializing_primary_key: bool,
        serde_skip_hidden_column: bool,
        _model_extra_derives: &TokenStream,
        model_extra_attributes: &TokenStream,
        _column_extra_derives: &TokenStream,
        _seaography: bool,
        _impl_active_model_behavior: bool,
    ) -> Vec<TokenStream> {
        let imports = Self::gen_imports(entity, with_serde);

        let code_blocks = vec![
            imports,
            Self::gen_oxide_model_struct(
                entity,
                with_serde,
                column_option,
                serde_skip_deserializing_primary_key,
                serde_skip_hidden_column,
                model_extra_attributes,
            ),
            Self::gen_oxide_entity_enum(entity),
            Self::gen_oxide_as_ref_impl(entity),
            Self::gen_oxide_iden_impl(entity),
            Self::gen_oxide_display_impl(entity),
        ];
        code_blocks
    }

    pub fn gen_imports(entity: &Entity, with_serde: &WithSerde) -> TokenStream {
        let mut imports = TokenStream::new();

        imports.extend(Self::gen_import_serde(with_serde));
        imports.extend(Self::gen_import_active_enum(entity));

        imports
    }

    #[allow(clippy::too_many_arguments)]
    pub fn gen_oxide_model_struct(
        entity: &Entity,
        with_serde: &WithSerde,
        column_option: &ColumnOption,
        serde_skip_deserializing_primary_key: bool,
        serde_skip_hidden_column: bool,
        model_extra_attributes: &TokenStream,
    ) -> TokenStream {
        let table_ident: TokenStream = entity
            .table_name
            .to_owned()
            .to_upper_camel_case()
            .parse()
            .unwrap();
        let column_names_snake_case = entity.get_column_names_snake_case();
        let column_rs_types = entity.get_column_rs_types(column_option);
        let if_eq_needed = entity.get_eq_needed();

        let primary_keys: Vec<String> = entity
            .primary_keys
            .iter()
            .map(|pk| pk.name.clone())
            .collect();

        let attrs: Vec<TokenStream> = entity
            .columns
            .iter()
            .map(|col| {
                let mut attrs: Punctuated<_, Comma> = Punctuated::new();
                let is_primary_key = primary_keys.contains(&col.name);
                if let Some(ts) = col.get_oxide_col_type_attrs() {
                    attrs.extend([ts]);
                };

                let mut ts = quote! {};
                if !attrs.is_empty() {
                    for (i, attr) in attrs.into_iter().enumerate() {
                        if i > 0 {
                            ts = quote! { #ts, };
                        }
                        ts = quote! { #ts #attr };
                    }
                    ts = quote! { #ts };
                }
                let serde_attribute = col.get_serde_attribute(
                    is_primary_key,
                    serde_skip_deserializing_primary_key,
                    serde_skip_hidden_column,
                );
                ts = quote! {
                    #ts
                    #serde_attribute
                };
                ts
            })
            .collect();
        let extra_derive = with_serde.extra_derive();

        quote! {
            #[derive(Clone, Debug, PartialEq, sqlx::FromRow #if_eq_needed #extra_derive)]
            #model_extra_attributes
            pub struct #table_ident {
                #(
                    #attrs
                    pub #column_names_snake_case: #column_rs_types,
                )*
            }
        }
    }

    pub fn gen_oxide_entity_enum(entity: &Entity) -> TokenStream {
        let table_name = entity.table_name.to_owned();
        let entity_name = format!("{table_name}Entity");
        let entity_ident: TokenStream = entity_name.to_upper_camel_case().parse().unwrap();

        let mut column_names_camel_case: Vec<syn::Ident> = Vec::new();
        column_names_camel_case.push(syn::parse_str("Table").unwrap());
        column_names_camel_case.extend(entity.get_column_names_camel_case());

        quote! {
            pub enum #entity_ident {
                #(
                    #column_names_camel_case,
                )*
            }
        }
    }

    pub fn gen_oxide_as_ref_impl(entity: &Entity) -> TokenStream {
        let table_name = entity.table_name.to_owned();
        let entity_name = format!("{table_name}Entity");
        let entity_ident: TokenStream = entity_name.to_upper_camel_case().parse().unwrap();

        let mut column_names_camel_case: Vec<syn::Ident> = Vec::new();
        column_names_camel_case.push(syn::parse_str("Table").unwrap());
        column_names_camel_case.extend(entity.get_column_names_camel_case());

        let mut column_names_snake_case: Vec<syn::Ident> = Vec::new();
        column_names_snake_case.push(syn::parse_str(&table_name).unwrap());
        column_names_snake_case.extend(entity.get_column_names_snake_case());

        let columns_mappings = (0..entity.columns.len() + 1).map(|idx| {
            let column_name = &column_names_camel_case[idx];
            let column_value = &column_names_snake_case[idx].to_string();
            let column_value = column_value.strip_prefix("r#").unwrap_or(column_value);

            let line = quote! {
                #entity_ident::#column_name => #column_value
            };

            line
        });

        quote! {
            impl AsRef<str> for #entity_ident {
                fn as_ref(&self) -> &str {
                    match self {
                        #(
                            #columns_mappings,
                        )*
                    }
                }
            }
        }
    }

    pub fn gen_oxide_iden_impl(entity: &Entity) -> TokenStream {
        let table_name = entity.table_name.to_owned();
        let entity_name = format!("{table_name}Entity");
        let entity_ident: TokenStream = entity_name.to_upper_camel_case().parse().unwrap();

        quote! {
            impl sea_query::Iden for #entity_ident {
                fn unquoted(&self, s: &mut dyn std::fmt::Write) {
                    write!(s, "{}", self.as_ref()).unwrap();
                }
            }
        }
    }

    pub fn gen_oxide_display_impl(entity: &Entity) -> TokenStream {
        let table_name = entity.table_name.to_owned();
        let entity_name = format!("{table_name}Entity");
        let entity_ident: TokenStream = entity_name.to_upper_camel_case().parse().unwrap();

        quote! {
            impl std::fmt::Display for #entity_ident {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{}", self.as_ref())
                }
            }
        }
    }
}
