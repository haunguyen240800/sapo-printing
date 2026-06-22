// Domain Layer — Core Business Logic (PURE — NO EXTERNAL DEPENDENCIES)
//
// Organized by Bounded Context per DDD strategic design:
//   - print_job/  : Print Job Management context (aggregate, entities, VOs, events,
//                   repository, services). Owns `PrinterId` as a cross-aggregate
//                   reference VO — the OS-owned printer resource is *not* modelled
//                   as a domain aggregate in this application (see ADR notes).

pub mod print_job;
