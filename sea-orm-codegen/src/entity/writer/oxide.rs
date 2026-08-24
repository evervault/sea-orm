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
        imports.extend(Self::gen_import_uuid(entity));

        imports
    }

    pub fn gen_import_uuid(entity: &Entity) -> TokenStream {
        fn has_uuid(col_type: &sea_query::ColumnType) -> bool {
            match col_type {
                sea_query::ColumnType::Uuid => true,
                sea_query::ColumnType::Array(inner) => has_uuid(inner),
                _ => false,
            }
        }

        if entity
            .columns
            .iter()
            .any(|col| has_uuid(col.get_inner_col_type()))
        {
            quote! {
                use uuid::Uuid;
            }
        } else {
            TokenStream::new()
        }
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
        let column_rs_types = entity.get_oxide_column_rs_types(column_option, with_serde);
        let if_eq_needed = entity.get_oxide_eq_needed();

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
                if let Some(ts) = col.get_oxide_col_type_attrs(with_serde) {
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
                let serde_attribute = if crate::entity::column::oxide_range(&col.col_type).is_some()
                {
                    // The range-specific attribute already skips unsupported
                    // serde directions, so do not emit a second serde attribute.
                    quote! {}
                } else {
                    col.get_serde_attribute(
                        is_primary_key,
                        serde_skip_deserializing_primary_key,
                        serde_skip_hidden_column,
                    )
                };
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

#[cfg(test)]
mod tests {
    use crate::{Column, ColumnOption, DateTimeCrate, Entity, EntityWriter, WithSerde};
    use sea_query::{Alias, ColumnType, IntoIden, RcOrArc};

    fn range_column(name: &str, range: &str) -> Column {
        column(name, ColumnType::Custom(Alias::new(range).into_iden()))
    }

    fn column(name: &str, col_type: ColumnType) -> Column {
        Column {
            name: name.to_owned(),
            col_type,
            auto_increment: false,
            not_null: true,
            unique: false,
            unique_key: None,
        }
    }

    fn entity(columns: Vec<Column>) -> Entity {
        Entity {
            table_name: "test".to_owned(),
            columns,
            relations: vec![],
            conjunct_relations: vec![],
            primary_keys: vec![],
        }
    }

    #[test]
    fn gen_import_uuid_emits_import_for_uuid_column() {
        let entity = entity(vec![
            column("id", ColumnType::BigInteger),
            column("uuid", ColumnType::Uuid),
        ]);
        assert_eq!(
            EntityWriter::gen_import_uuid(&entity).to_string(),
            "use uuid :: Uuid ;"
        );
    }

    #[test]
    fn gen_import_uuid_emits_import_for_uuid_array_column() {
        let entity = entity(vec![column(
            "uuids",
            ColumnType::Array(RcOrArc::new(ColumnType::Uuid)),
        )]);
        assert_eq!(
            EntityWriter::gen_import_uuid(&entity).to_string(),
            "use uuid :: Uuid ;"
        );
    }

    #[test]
    fn gen_import_uuid_emits_nothing_without_uuid_column() {
        let entity = entity(vec![
            column("id", ColumnType::BigInteger),
            column("name", ColumnType::Text),
        ]);
        assert!(EntityWriter::gen_import_uuid(&entity).is_empty());
    }

    #[test]
    fn range_columns_are_rendered_as_pg_range() {
        let opt = ColumnOption::default();
        for (range, element) in [
            ("int4range", "i32"),
            ("int8range", "i64"),
            ("numrange", "sqlx :: types :: BigDecimal"),
            ("daterange", "chrono :: NaiveDate"),
            ("tsrange", "chrono :: NaiveDateTime"),
            ("tstzrange", "chrono :: DateTime < chrono :: Utc >"),
        ] {
            assert_eq!(
                range_column("r", range)
                    .get_oxide_rs_type(&opt, &WithSerde::Both)
                    .to_string(),
                format!("Option < sqlx :: postgres :: types :: PgRange < {element} > >"),
                "unexpected type for {range}"
            );
        }
    }

    #[test]
    fn temporal_range_columns_follow_the_date_time_crate() {
        let opt = ColumnOption {
            date_time_crate: DateTimeCrate::Time,
            ..Default::default()
        };
        assert_eq!(
            range_column("r", "tstzrange")
                .get_oxide_rs_type(&opt, &WithSerde::Both)
                .to_string(),
            "Option < sqlx :: postgres :: types :: PgRange < time :: OffsetDateTime > >"
        );
    }

    #[test]
    fn range_columns_are_optional_when_deserializing() {
        let col = range_column("r", "numrange");
        assert!(
            col.get_oxide_rs_type(&ColumnOption::default(), &WithSerde::Deserialize)
                .to_string()
                .starts_with("Option <"),
            "a skipped field must implement Default"
        );
    }

    #[test]
    fn non_null_range_columns_preserve_nullability_without_deserialization() {
        let col = range_column("r", "numrange");
        assert_eq!(
            col.get_oxide_rs_type(&ColumnOption::default(), &WithSerde::None)
                .to_string(),
            "sqlx :: postgres :: types :: PgRange < sqlx :: types :: BigDecimal >"
        );
    }

    #[test]
    fn other_custom_columns_are_untouched() {
        let opt = ColumnOption::default();
        assert_eq!(
            range_column("t", "tsvector")
                .get_oxide_rs_type(&opt, &WithSerde::None)
                .to_string(),
            "String"
        );
    }

    #[test]
    fn range_columns_are_skipped_only_when_serde_is_derived() {
        let col = range_column("r", "numrange");
        assert!(col.get_oxide_col_type_attrs(&WithSerde::None).is_none());
        assert_eq!(
            col.get_oxide_col_type_attrs(&WithSerde::Serialize)
                .expect("expected a serde attribute")
                .to_string(),
            "# [serde (skip_serializing)]"
        );
        assert_eq!(
            col.get_oxide_col_type_attrs(&WithSerde::Deserialize)
                .expect("expected a serde attribute")
                .to_string(),
            "# [serde (skip)]"
        );
    }

    #[test]
    fn numrange_suppresses_the_eq_derive() {
        let entity = entity(vec![
            column("id", ColumnType::BigInteger),
            range_column("r", "numrange"),
        ]);
        assert!(entity.get_oxide_eq_needed().is_empty());
    }

    #[test]
    fn ranges_with_eq_elements_keep_the_eq_derive() {
        let entity = entity(vec![
            column("id", ColumnType::BigInteger),
            range_column("r", "int8range"),
        ]);
        assert_eq!(entity.get_oxide_eq_needed().to_string(), ", Eq");
    }
}
