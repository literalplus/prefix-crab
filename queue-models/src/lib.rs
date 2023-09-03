pub mod echo_response;
pub mod probe_request;

/// Types that have a (constant) routing key to be used to indicate messages of this type on an exchange where multiple
/// types of message are sent.
///
/// This takes a self parameter for use in enums (specifically, enums of message type on an exchange).
pub trait RoutedMessage {
    fn routing_key(&self) -> &'static str;
}

/// Specialisation of [RoutedMessage] where the routing key resolution doesn't require a self parameter.
/// This is so that routing keys can be obtained statically without requiring an instance.
pub trait TypeRoutedMessage {
    fn routing_key() -> &'static str;
}

impl<T: TypeRoutedMessage> RoutedMessage for T {
    fn routing_key(&self) -> &'static str {
        Self::routing_key()
    }
}
