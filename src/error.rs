pub trait IsPermanent {
    fn is_permanent(&self) -> bool;
}

#[macro_export]
macro_rules! drop_if_permanent {
    ($err: ident <- $concrete_ty: ty) => {
        match $err.downcast_ref::<$concrete_ty>() {
            Some(inner) => {
                if inner.is_permanent() {
                    error!("Permanent error handling a request, skipping it: {:?}", inner);
                    return Ok(());
                }
            },
            None => {}
        };
    }
}