---
name: rusqlite-transaction-patterns
description: Common rusqlite pitfalls and correct patterns for transactions, error handling, and ownership in this project
source: auto-skill
extracted_at: '2026-06-23T15:05:18.830Z'
---

# rusqlite Transaction & Error Patterns

Learned from fixing story 3-7 (SqlitePrintJobRepository + SqliteEventStore). These are non-obvious rusqlite behaviors that cause compile errors or silent bugs.

## 1. Transaction::rollback() takes ownership — cannot use in map_err closure

`Transaction::rollback(self)` consumes `self`. If you call it inside a closure passed to `map_err`, the closure moves `tx` on the first iteration, making `tx.commit()` fail with E0382.

**WRONG** — fails to compile (E0382: use of moved value):
```rust
for event in events.iter() {
    tx.execute(sql, params).map_err(|e| {
        let _ = tx.rollback();  // moves tx into closure!
        MyError::from(e)
    })?;
}
tx.commit()?;  // ERROR: tx was moved
```

**RIGHT** — use `drop(tx)` which also triggers automatic rollback:
```rust
for event in events.iter() {
    if let Err(e) = tx.execute(sql, params) {
        // Drop tx to trigger automatic rollback (rusqlite rolls back on uncommitted drop).
        // We do NOT call tx.rollback() because it takes ownership of self,
        // preventing further use of tx. Dropping achieves the same effect.
        drop(tx);
        return Err(MyError::from(e));
    }
}
tx.commit()?;
```

Alternatively, restructure to use `match` without closures:
```rust
match tx.execute(sql, params) {
    Ok(_) => {}
    Err(e) => {
        // tx is still owned here, can be dropped or rolled back
        let _ = tx.rollback();
        return Err(MyError::from(e));
    }
}
```

## 2. ErrorCode for constraint violations is `ConstraintViolation`, not `Constraint`

`rusqlite::ErrorCode::Constraint` does NOT exist. The correct variant is `ConstraintViolation`.

**WRONG** — compile error (E0599):
```rust
rusqlite::ErrorCode::Constraint  // does not exist
```

**RIGHT**:
```rust
if matches!(
    &e,
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error {
            code: rusqlite::ErrorCode::ConstraintViolation,
            ..
        },
        _
    )
) {
    // Handle duplicate key / constraint violation
}
```

Pattern matching requires `&e` (borrow) because `Error::SqliteFailure` contains non-`Copy` fields. Use `matches!` macro, not `==` comparison.

## 3. Always check `rows == 0` after `execute()` for UPDATE/DELETE

`conn.execute()` returns the number of affected rows. If no row matches the WHERE clause, it returns `Ok(0)` — NOT an error. This means UPDATE/DELETE silently succeed when the target doesn't exist.

**WRONG** — silent failure:
```rust
conn.execute("UPDATE print_jobs SET status = ?1 WHERE id = ?2", params)?;
Ok(())  // Returns Ok even if job doesn't exist!
```

**RIGHT**:
```rust
let rows = conn.execute("UPDATE print_jobs SET status = ?1 WHERE id = ?2", params)?;
if rows == 0 {
    return Err(DomainError::RepositoryError {
        reason: format!("Job '{}' not found", id),
    });
}
Ok(())
```

Same pattern applies to INSERT for detecting duplicate keys (though constraint violation error is more reliable for this).

## 4. Single timestamp for all temporal columns

When setting `created_at`, `updated_at`, and `completed_at` in the same operation, compute `SystemTime::now()` once and reuse it. Calling `now()` multiple times produces different microsecond values, which is confusing during audit.

**RIGHT**:
```rust
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
let completed_at = completed_at_for_status(status, now);
// Use `now` for created_at, updated_at, and `completed_at` for completed_at
```

## 5. WDAC blocks stale proc-macro DLLs

If `cargo build` or `cargo check` fails with `os error 4551` on `webview2_com_macros-*.dll` (or any proc-macro DLL), delete the cached DLL:

```powershell
del "target\debug\deps\webview2_com_macros-*.dll"
cargo check
```

This is a Windows Defender Application Control issue — the DLL was cached from a previous build and is now blocked. Deleting it forces a fresh build which may succeed.
