pub trait DomainRule {
    fn is_broken(&self) -> bool;
    fn message(&self) -> String;
}
