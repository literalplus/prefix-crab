pub use super::echo::result::*;

pub trait CanFollowUp {
    fn needs_follow_up(&self) -> bool;
}

impl CanFollowUp for EchoResult {
    fn needs_follow_up(&self) -> bool {
        return self.splits.iter().any(|it| it.needs_follow_up());
    }
}

impl CanFollowUp for EchoSplitResult {
    fn needs_follow_up(&self) -> bool {
        !self.follow_ups.is_empty()
    }
}
