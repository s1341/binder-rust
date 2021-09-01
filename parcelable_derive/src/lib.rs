use proc_macro2::{Ident, TokenStream};
use quote::{quote, format_ident};
use syn::{self, Attribute, DeriveInput, parse_macro_input, punctuated::Punctuated};
use syn::Meta::{List, NameValue};
use syn::NestedMeta::Meta;
use syn::Token;

//#[derive(FromDeriveInput, Default)]
//#[darling(defautl, attributes(Parcelable))]
//struct Opts {
    //discriminator = Option<i32>,
//}

/// A source data structure annotated with `#[derive(Serialize)]` and/or `#[derive(Deserialize)]`,
/// parsed into an internal representation.
struct Container<'a> {
    /// The struct or enum name (without generics).
    pub ident: syn::Ident,
    /// Attributes on the structure, parsed for Serde.
    //pub attrs: ContainerAttribute,
    /// The contents of the struct or enum.
    pub data: Data<'a>,
    /// Any generics on the struct or enum.
    pub _generics: &'a syn::Generics,
    /// Original input.
    pub _original: &'a syn::DeriveInput,
}

/// The fields of a struct or enum.
///
/// Analogous to `syn::Data`.
enum Data<'a> {
    Enum(Vec<Variant<'a>>),
    Struct(Style, Vec<Field<'a>>),
}

/// A variant of an enum.
struct Variant<'a> {
    pub ident: syn::Ident,
    pub attrs: VariantAttribute,
    pub style: Style,
    pub fields: Vec<Field<'a>>,
    pub _original: &'a syn::Variant,
}

/// A field of a struct.
#[derive(Debug)]
struct Field<'a> {
    pub member: syn::Member,
    //pub attrs: FieldAttribute,
    pub ty: &'a syn::Type,
    pub original: &'a syn::Field,
}

#[derive(Copy, Clone)]
enum Style {
    /// Named fields.
    Struct,
    /// Many unnamed fields.
    Tuple,
    /// One unnamed field.
    Newtype,
    /// No fields.
    Unit,
}

impl<'a> Container<'a> {
    /// Convert the raw Syn ast into a parsed container object, collecting errors in `cx`.
    pub fn from_ast(
        item: &'a syn::DeriveInput,
    ) -> Option<Container<'a>> {
        let data = match &item.data {
            syn::Data::Enum(data) => Data::Enum(enum_from_ast(&data.variants)),
            syn::Data::Struct(data) => {
                let (style, fields) = struct_from_ast(&data.fields, None);
                Data::Struct(style, fields)
            }
            syn::Data::Union(_) => {
                panic!("Parcelable does not support derive for unions");
            }
        };

        let item = Container {
            ident: item.ident.clone(),
            //attrs,
            data,
            _generics: &item.generics,
            _original: item,
        };
        Some(item)
    }
}

impl<'a> Data<'a> {
    //pub fn all_fields(&'a self) -> Box<dyn Iterator<Item = &'a Field<'a>> + 'a> {
        //match self {
            //Data::Enum(variants) => {
                //Box::new(variants.iter().flat_map(|variant| variant.fields.iter()))
            //}
            //Data::Struct(_, fields) => Box::new(fields.iter()),
        //}
    //}

    //pub fn has_getter(&self) -> bool {
        //self.all_fields().any(|f| f.attrs.getter().is_some())
    //}
}

#[derive(Default)]
struct VariantAttribute {
    discriminator: Option<i32>,
}

fn get_meta_items(attr: &syn::Attribute) -> Result<Vec<syn::NestedMeta>, ()> {
    if attr.path.get_ident().unwrap() != "parcelable" {
        return Ok(Vec::new());
    }

    match attr.parse_meta() {
        Ok(List(meta)) => Ok(meta.nested.into_iter().collect()),
        Ok(_other) => {
            panic!("expected #[parcelable(...)]");
        }
        Err(err) => {
            panic!("error gathering attributes: {}", err);
        }
    }
}
fn variant_attributes(attrs: &[Attribute]) -> VariantAttribute {
    let mut variant_attribute = VariantAttribute::default();
    for meta_item in attrs.iter().flat_map(|attr| get_meta_items(attr)).flatten() {
        match &meta_item {
            Meta(NameValue(m)) if m.path.get_ident().unwrap() == "discriminator" => {
                if let syn::Lit::Int(int) = &m.lit {
                    variant_attribute.discriminator = Some(int.base10_parse::<i32>().unwrap());
                };
            }
            _ => {
                panic!("unexpected parcelable attribute");
            }
        }
    }

    variant_attribute
}


fn enum_from_ast(
    variants: &Punctuated<syn::Variant,  Token![,]>,
) -> Vec<Variant> {
    variants
        .iter()
        .map(|variant| {
            let attrs = variant_attributes(&variant.attrs);
            let (style, fields) =
                struct_from_ast(&variant.fields, Some(&attrs));
            Variant {
                ident: variant.ident.clone(),
                attrs,
                style,
                fields,
                _original: variant,
            }
        })
        .collect()
}

fn struct_from_ast<'a>(
    fields: &'a syn::Fields,
    attrs: Option<&VariantAttribute>,
) -> (Style, Vec<Field<'a>>) {
    match fields {
        syn::Fields::Named(fields) => (
            Style::Struct,
            fields_from_ast(&fields.named, attrs),
        ),
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => (
            Style::Newtype,
            fields_from_ast(&fields.unnamed, attrs),
        ),
        syn::Fields::Unnamed(fields) => (
            Style::Tuple,
            fields_from_ast(&fields.unnamed, attrs),
        ),
        syn::Fields::Unit => (Style::Unit, Vec::new()),
    }
}

fn fields_from_ast<'a>(
    fields: &'a Punctuated<syn::Field, Token![,]>,
    _attrs: Option<&VariantAttribute>,
) -> Vec<Field<'a>> {
    fields
        .iter()
        .enumerate()
        .map(|(i, field)| Field {
            member: match &field.ident {
                Some(ident) => syn::Member::Named(ident.clone()),
                None => syn::Member::Unnamed(i.into()),
            },
            //attrs: field_attributes(field.attrs),
            ty: &field.ty,
            original: field,
        })
        .collect()
}

fn build_newtype_variant(typename: &Ident, variant_name: &Ident, field: &Field) -> TokenStream {
    let field_ty = field.ty;
    quote! {{
        #typename::#variant_name(<#field_ty as Parcelable>::deserialize(parcel)?)
    }}
}
fn build_tuple_variant(typename: &Ident, variant_name: &Ident, fields: &[Field]) -> TokenStream {
    if fields.len() == 1 {
        return build_newtype_variant(typename, variant_name, &fields[0]);
    }

    let field_expressions = fields.iter().map(|field| {
        let field_ty = field.ty;
        quote! {
            <#field_ty as Parcelable>::deserialize(parcel)?
        }
    });

    quote! {{
        #typename::#variant_name(#(#field_expressions),*)
    }}
}
fn build_struct_variant(typename: &Ident, variant_name: &Ident, fields: &[Field]) -> TokenStream {
    let field_expressions = fields.iter().map(|field| {
        let field_ty = field.ty;
        let field_name = &field.member;
        quote! {
            #field_name: <#field_ty as Parcelable>::deserialize(parcel)?
        }
    });

    quote! {{
        #typename::#variant_name{#(#field_expressions),*}
    }}
}

#[proc_macro_derive(Parcelable, attributes(parcelable))]
pub fn parcelable_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let cont = Container::from_ast(&input).unwrap();
    let ident = &cont.ident;
    let ident_path: syn::Path = ident.clone().into();
    let typename = &ident_path.segments.last().unwrap().ident;

    let body_deserialize = match &cont.data {
        Data::Enum(variants) => {
            let variant_arms = variants.iter().enumerate().map(|(i, variant)| {
                let discriminator = if let Some(discriminator) = variant.attrs.discriminator {
                    discriminator
                } else {
                    i as i32
                };

                let variant_name = &variant.ident;

                let block = match variant.style {
                    Style::Unit => {
                        quote! {
                            #typename::#variant_name,
                        }
                    },
                    Style::Newtype => {
                        build_newtype_variant(typename, variant_name, &variant.fields[0])
                    },
                    Style::Tuple => {
                        build_tuple_variant(typename, variant_name, &variant.fields)
                    },
                    Style::Struct => {
                        build_struct_variant(typename, variant_name, &variant.fields)
                    },
                };
                quote! {
                    #discriminator => #block
                }
            });

            quote! {
                Ok(match parcel.read_i32()? {
                    #(#variant_arms)*
                    _ => { return Err(Error::BadEnumValue); }
                })
            }
        },
        Data::Struct(Style::Struct, fields) => {
            let field_expressions = fields.iter().map(|field| {
                let field_name = &field.member;
                let field_ty = field.ty;
                quote! {
                    #field_name: <#field_ty as Parcelable>::deserialize(parcel)?
                }
            });

            quote! {
                Ok(#typename{#(#field_expressions),*})
            }
        },
        Data::Struct(Style::Tuple, fields) => {
            let field_expressions = fields.iter().map(|field| {
                let field_ty = field.ty;
                quote! {
                    <#field_ty as Parcelable>::deserialize(parcel)?
                }
            });

            quote! {
                Ok(#typename(#(#field_expressions),*))
            }
        },
        Data::Struct(Style::Unit, _fields) => {
            quote! {
                Ok(())
            }
        },
        Data::Struct(Style::Newtype, fields) => {
            let field_type = fields[0].ty;
            quote! {
                Ok(#typename(<#field_type as Parcelable>::deserialize(parcel)?))
            }
        },
    };

    let body_serialize = match &cont.data {
        Data::Enum(variants) => {
            let variant_arms = variants.iter().enumerate().map(|(i, variant)| {
                let discriminator = if let Some(discriminator) = variant.attrs.discriminator {
                    discriminator
                } else {
                    i as i32
                };

                let variant_name = &variant.ident;

                let block = match variant.style {
                    Style::Unit => {
                        quote! {
                            #typename::#variant_name => { parcel.write_i32(#discriminator)?; },
                        }
                    },
                    Style::Newtype => {
                        //build_newtype_variant(typename, variant_name, &variant.fields[0])
                        quote! {
                            #typename::#variant_name(_nt) => {
                                parcel.write_i32(#discriminator)?;
                                _nt.serialize(parcel)?
                            }
                        }
                    },
                    Style::Tuple => {
                        let field_expressions = variant.fields.iter().enumerate().map(|(i, _field)| {
                            let name = format_ident!("_t_{}", i);
                            quote! {
                                #name.serialize(parcel)?
                            }
                        });


                        let mut field_names = Vec::new();
                        for i in 0..variant.fields.len() {
                            field_names.push(format_ident!("_t_{}", i));
                        }

                        quote! {
                            #typename::#variant_name(#(#field_names),*) => {
                                parcel.write_i32(#discriminator)?;
                                #(#field_expressions);*
                            }
                        }
                    },
                    Style::Struct => {
                        let field_expressions = variant.fields.iter().map(|field| {
                            let field_name = &field.member;
                            quote! {
                                #field_name.serialize(parcel)?
                            }

                        });
                        let field_names = variant.fields.iter().map(|field| {
                            &field.member

                        });

                        quote! {
                            #typename::#variant_name{#(#field_names),*} => {
                                parcel.write_i32(#discriminator)?;

                                #(#field_expressions);*
                            }
                        }
                    },
                };
                block
            });

            quote! {
                match self {
                    #(#variant_arms)*
                };
            }
        },
        Data::Struct(Style::Struct, fields) => {
            let field_expressions = fields.iter().map(|field| {
                let field_name = &field.member;
                quote! {
                    self.#field_name.serialize(parcel)?;
                }
            });

            quote! {
                #(#field_expressions)*
            }
        },
        Data::Struct(Style::Tuple, fields) => {
            let field_expressions = fields.iter().enumerate().map(|(i, _field)| {
                let name = format_ident!("_t_{}", i);
                quote! {
                    #name.serialize(parcel)?;
                }
            });


            let mut field_names = Vec::new();
            for i in 0..fields.len() {
                field_names.push(format_ident!("_t_{}", i));
            }

            quote! {
                if let #typename(#(#field_names),*) = self {
                    #(#field_expressions)*
                }

            }
        },
        Data::Struct(Style::Unit, _fields) => {
            quote! {
            }
        },
        Data::Struct(Style::Newtype, _fields) => {
            quote! {
                self.0.serialize(parcel)?;
            }
        },
    };

    let output = quote! {
        impl Parcelable for #ident {
            fn deserialize(parcel: &mut Parcel) -> Result<Self, Error> where Self: Sized {
                #body_deserialize
            }
            fn serialize(&self, parcel: &mut Parcel) -> Result<(), Error> {
                #body_serialize
                Ok(())
            }
        }
    };

    output.into()
}
