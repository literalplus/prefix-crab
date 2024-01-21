pub use cidr::*;

mod cidr {
    use diesel::internal::derives::as_expression::Bound;

    use diesel::expression::AsExpression;
    use diesel::pg::Pg;
    use diesel::sql_types::{Bool, Cidr};
    use diesel::Expression;
    use diesel::{self, ExpressionMethods};
    use ipnet::{IpNet, Ipv6Net};

    diesel::infix_operator!(SubnetOrEq, " <<= ", Bool, backend: Pg);
    diesel::infix_operator!(SupernetOrEq, " >>= ", Bool, backend: Pg);
    diesel::sql_function! { fn masklen(x: Cidr) -> Int8; }

    pub trait CidrMethods
    where
        Self: AsExpression<Cidr> + Sized,
    {
        // Note: If there are issues with operator precedence (because of missing parentheses),
        // we can copy over Grouped from upstream (which is not public atm sadly)
        // https://github.com/diesel-rs/diesel/blob/13c237473627ea2500c2274e3b0cf8a54187079a/diesel/src/expression/grouped.rs#L8

        fn subnet_or_eq<OtherType>(
            self,
            other: OtherType,
        ) -> SubnetOrEq<Self::Expression, OtherType::Expression>
        where
            OtherType: AsExpression<Cidr>,
        {
            SubnetOrEq::new(self.as_expression(), other.as_expression())
        }

        fn subnet_or_eq6(
            self,
            other: &Ipv6Net,
        ) -> SubnetOrEq<Self::Expression, Bound<Cidr, IpNet>> {
            self.subnet_or_eq(IpNet::V6(*other))
        }

        fn supernet_or_eq<OtherType>(
            self,
            other: OtherType,
        ) -> SupernetOrEq<Self::Expression, OtherType::Expression>
        where
            OtherType: AsExpression<Cidr>,
        {
            SupernetOrEq::new(self.as_expression(), other.as_expression())
        }

        fn supernet_or_eq6(
            self,
            other: &Ipv6Net,
        ) -> SupernetOrEq<Self::Expression, Bound<Cidr, IpNet>> {
            self.supernet_or_eq(IpNet::V6(*other))
        }

        fn eq6(self, other: &Ipv6Net) -> diesel::dsl::Eq<Self::Expression, IpNet> {
            ExpressionMethods::eq(self.as_expression(), IpNet::V6(*other))
        }
    }

    impl<T> CidrMethods for T where T: Expression<SqlType = Cidr> {}
}
