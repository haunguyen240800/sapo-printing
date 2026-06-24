use rusqlite::Connection;
use sapo_printer::domain::print_job::aggregate::PrintJob;
use sapo_printer::domain::print_job::PrintJobRepository;
use sapo_printer::infrastructure::database::migrations::run_migrations;
use sapo_printer::infrastructure::database::SqlitePrintJobRepository;
use sapo_printer::infrastructure::queue::{QueueManager, SqliteQueueManager};
use std::sync::{Arc, Mutex};

#[test]
fn test_jobs_persist_across_simulated_restart() {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(Mutex::new(conn));

    // "Session 1": insert PENDING job, push to queue
    let job_repo = SqlitePrintJobRepository::new(arc_conn.clone());
    let mgr1 = SqliteQueueManager::new(arc_conn.clone());
    let job = PrintJob::new("https://s3.example.com/doc.pdf".into(), "HP".into());
    let job_id = job.id().clone();
    job_repo.save(&job).unwrap();
    mgr1.push(&job_id).unwrap();

    // "Session 2": new QueueManager instance, same DB — job must still be there
    let mgr2 = SqliteQueueManager::new(arc_conn.clone());
    let depth = mgr2.queue_depth().unwrap();
    assert_eq!(depth, 1);

    let popped = mgr2.pop().unwrap();
    assert!(popped.is_some());
    assert_eq!(popped.unwrap().id(), &job_id);
}
