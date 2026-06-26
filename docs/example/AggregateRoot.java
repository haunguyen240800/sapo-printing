package vn.sapo.omni.service.ddd;

import com.fasterxml.jackson.annotation.JsonIgnore;
import jakarta.persistence.*;
import lombok.extern.slf4j.Slf4j;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

@Slf4j
@MappedSuperclass
public abstract class AggregateRoot<R extends AggregateRoot<R>> extends DomainEntity<R> {

    @Transient
    protected List<DomainEvent> domainEvents = new ArrayList<>();

    @JsonIgnore
    public List<DomainEvent> getDomainEvents() {
        if (this.domainEvents == null) return Collections.emptyList();
        return Collections.unmodifiableList(this.domainEvents);
    }

    @PostPersist
    @PostUpdate
    @PostRemove
    protected void clearDomainEvents() {
        if (domainEvents != null) domainEvents.clear();
    }

    protected void addDomainEvent(DomainEvent domainEvent) {
        this.domainEvents.add(domainEvent);
    }
}
