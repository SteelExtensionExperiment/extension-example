use std::collections::BTreeMap;

use syn::{
    Error, Expr, ExprLit, Ident, Lit, LitInt, LitStr, Token, braced, parse::ParseStream, token,
};

pub(super) struct Element {
    pub(super) name: Ident,
    props: BTreeMap<String, Property>,
    children: Vec<Self>,
}

pub(super) struct Property(Expr);

impl Element {
    pub(super) fn parse_body(name: Ident, input: ParseStream<'_>) -> syn::Result<Self> {
        let body;
        braced!(body in input);
        Self::parse_fields(name, &body)
    }

    pub(super) fn parse_fields(name: Ident, body: ParseStream<'_>) -> syn::Result<Self> {
        let mut props = BTreeMap::new();
        let mut children = Vec::new();
        while !body.is_empty() {
            let key = body.parse::<Ident>()?;
            if body.peek(token::Brace) {
                children.push(Self::parse_body(key, body)?);
                if body.peek(Token![,]) {
                    body.parse::<Token![,]>()?;
                }
                continue;
            }
            if !children.is_empty() {
                return Err(Error::new_spanned(
                    key,
                    "properties must precede child elements",
                ));
            }
            body.parse::<Token![:]>()?;
            let value = Property(body.parse()?);
            if props.insert(key.to_string(), value).is_some() {
                return Err(Error::new_spanned(key, "duplicate property"));
            }
            if !body.is_empty() {
                body.parse::<Token![,]>()?;
            }
        }
        Ok(Self {
            name,
            props,
            children,
        })
    }

    pub(super) fn optional<T: TryFrom<Property, Error = Error>>(
        &mut self,
        key: &str,
    ) -> syn::Result<Option<T>> {
        self.props.remove(key).map(T::try_from).transpose()
    }

    pub(super) fn required<T: TryFrom<Property, Error = Error>>(
        &mut self,
        key: &str,
    ) -> syn::Result<T> {
        self.optional(key)?
            .ok_or_else(|| Error::new_spanned(&self.name, format!("missing property {key}")))
    }

    pub(super) fn finish(self) -> syn::Result<Vec<Self>> {
        if let Some((key, value)) = self.props.into_iter().next() {
            return Err(Error::new_spanned(
                value.0,
                format!("unknown {} property {key}", self.name),
            ));
        }
        Ok(self.children)
    }
}

impl TryFrom<Property> for Expr {
    type Error = Error;

    fn try_from(value: Property) -> syn::Result<Self> {
        Ok(value.0)
    }
}

impl TryFrom<Property> for Ident {
    type Error = Error;

    fn try_from(value: Property) -> syn::Result<Self> {
        if let Expr::Path(path) = &value.0
            && path.qself.is_none()
            && let Some(ident) = path.path.get_ident()
        {
            return Ok(ident.clone());
        }
        Err(Error::new_spanned(value.0, "expected an identifier"))
    }
}

macro_rules! property_type {
    ($type:ty, $variant:ident, $message:literal) => {
        impl TryFrom<Property> for $type {
            type Error = Error;

            fn try_from(value: Property) -> syn::Result<Self> {
                match value.0 {
                    Expr::Lit(ExprLit {
                        lit: Lit::$variant(value),
                        ..
                    }) => Ok(value),
                    value => Err(Error::new_spanned(value, $message)),
                }
            }
        }
    };
}

property_type!(LitStr, Str, "expected a string literal");
property_type!(LitInt, Int, "expected an integer literal");

pub(super) struct SlotSize {
    pub(super) width: LitInt,
    pub(super) height: LitInt,
}

impl TryFrom<Property> for SlotSize {
    type Error = Error;

    fn try_from(value: Property) -> syn::Result<Self> {
        let Expr::Tuple(tuple) = value.0 else {
            return Err(Error::new_spanned(
                value.0,
                "expected size: (width, height)",
            ));
        };
        let mut elements = tuple.elems.into_pairs();
        let (Some(width), Some(height), None) = (elements.next(), elements.next(), elements.next())
        else {
            return Err(Error::new(
                tuple.paren_token.span.join(),
                "size requires a width and height",
            ));
        };
        Ok(Self {
            width: Property(width.into_value()).try_into()?,
            height: Property(height.into_value()).try_into()?,
        })
    }
}
