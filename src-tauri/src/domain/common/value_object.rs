use super::rule::DomainRule;
use super::error::DomainValidationException;

pub trait ValueObject: PartialEq {
    fn check_rule(&self, rule: &dyn DomainRule) -> Result<(), DomainValidationException> {
        if rule.is_broken() {
            Err(DomainValidationException::new(rule.message()))
        } else {
            Ok(())
        }
    }
}
