package vn.sapo.omni.service.ddd;

import java.time.Instant;
import java.util.UUID;

public interface DomainEvent {

    UUID getEventId();

    String getEventName();

    /**
     * Alias for {@link #getHappenedAt() happenedAt}
     */
    Instant getOccurredAt();

    /**
     * Alias for {@link #getOccurredAt() occurredAt}
     */
    Instant getHappenedAt();
}
