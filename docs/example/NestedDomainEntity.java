package vn.sapo.omni.service.ddd;

public abstract class NestedDomainEntity<R extends AggregateRoot<R>> extends DomainEntity<R> {

    protected abstract R getAggRoot();

    protected void addDomainEvent(DomainEvent domainEvent) {
        if (getAggRoot() != null) getAggRoot().addDomainEvent(domainEvent);
    }
}
