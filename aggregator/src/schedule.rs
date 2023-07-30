use type_safe_id::{StaticType, TypeSafeId};

#[derive(Default)]
pub struct FollowUpTypeMarker;

impl StaticType for FollowUpTypeMarker {
    const TYPE: &'static str = "fou";
}

pub type FollowUpId = TypeSafeId<FollowUpTypeMarker>;
