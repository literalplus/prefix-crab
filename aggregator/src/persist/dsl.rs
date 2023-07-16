pub use path::*;

mod path {
    use std::str::FromStr;

    use anyhow::{anyhow, Context};
    use diesel;
    use diesel::backend::Backend;
    use diesel::deserialize::FromSql;
    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::sql_types::Bool;
    use diesel::Expression;

    use crate::model::PrefixPath;

    use super::super::schema::sql_types::Ltree;

    diesel::infix_operator!(AncestorOrSame, " @> ", Bool, backend: Pg);
    diesel::infix_operator!(DescendantOrSame, " <@ ", Bool, backend: Pg);

    pub trait PathExpressionMethods
    where
        Self: AsExpression<Ltree> + Sized,
    {
        // Note: If there are issues with operator precedence (because of missing parentheses),
        // we can copy over Grouped from upstream (which is not public atm sadly)
        // https://github.com/diesel-rs/diesel/blob/13c237473627ea2500c2274e3b0cf8a54187079a/diesel/src/expression/grouped.rs#L8

        fn ancestor_or_same_as<OtherType>(
            self,
            other: OtherType,
        ) -> AncestorOrSame<Self::Expression, OtherType::Expression>
        where
            OtherType: AsExpression<Ltree>,
        {
            AncestorOrSame::new(self.as_expression(), other.as_expression())
        }

        fn descendant_or_same_as<OtherType>(
            self,
            other: OtherType,
        ) -> DescendantOrSame<Self::Expression, OtherType::Expression>
        where
            OtherType: AsExpression<Ltree>,
        {
            DescendantOrSame::new(self.as_expression(), other.as_expression())
        }
    }

    impl<T> PathExpressionMethods for T where T: Expression<SqlType = Ltree> {}

    impl FromSql<Ltree, Pg> for PrefixPath {
        fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            let buf = bytes.as_bytes();
            if buf[0] != 1u8 {
                return Err(
                    anyhow!("Unexpected ltree version {}, only 1 is supported.", buf[0]).into(),
                );
            }
            // Sadly, once we "open up" the RawValue, we cannot re-assemble it since all necessary
            // functions are private / only-for-new-backends in Diesel. So we cannot reuse the
            // existing (zero-copy) String impl but instead need to parse manually.
            let as_str = String::from_utf8(buf[1..].to_vec())
                .with_context(|| "while reading UTF-8 string")?;
            Ok(PrefixPath::from_str(&as_str)?)
        }
    }
}
