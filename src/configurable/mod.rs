
/// The trait defining the necessary types for a configurable protocol
pub trait ConfigurableProtocol {

    type Config: Send + Clone;

}