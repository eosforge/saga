//! Application-owned catalog connection, serialized across blocking native commands.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::storage;

/// Keeps the initialized connection alive until the application releases its managed state.
pub(super) struct Database {
    /// Acquired before storage archive locks; operations must not recursively enter this owner.
    connection: Mutex<Connection>,
}

impl Database {
    /// Opens and initializes once; callers supply an application-owned or isolated test path.
    pub(super) fn open(path: &Path) -> Result<Self, String> {
        Ok(Self {
            connection: Mutex::new(storage::open(path)?),
        })
    }

    /// Runs a complete operation on a blocking worker; ordinary errors release the connection.
    pub(super) fn execute<T>(&self, operation: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
        let connection = self.connection.lock().map_err(|_| "catalog connection is poisoned; restart the application")?;
        operation(&connection)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn operations_reuse_connection_state_and_survive_sql_errors() {
        let database = Database::open(Path::new(":memory:")).unwrap();
        database
            .execute(|connection| {
                connection
                    .execute_batch("CREATE TEMP TABLE fixture(value INTEGER); INSERT INTO fixture VALUES(7)")
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        assert!(database
            .execute(|connection| connection.execute_batch("INSERT INTO absent VALUES(1)").map_err(|e| e.to_string()))
            .is_err());
        let value: i64 = database
            .execute(|connection| {
                connection
                    .query_row("SELECT value FROM fixture", [], |row| row.get(0))
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        assert_eq!(value, 7);
    }

    #[test]
    fn a_panicked_operation_poisons_the_owner_instead_of_reusing_unknown_connection_state() {
        let database = Database::open(Path::new(":memory:")).unwrap();
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            database.execute::<()>(|_| panic!("isolated operation failure"))
        }));
        assert!(interrupted.is_err());
        assert!(database.execute(|_| Ok(())).unwrap_err().contains("poisoned"));
    }

    #[test]
    fn concurrent_operations_serialize_complete_read_modify_write_sequences() {
        let database = Arc::new(Database::open(Path::new(":memory:")).unwrap());
        database
            .execute(|connection| {
                connection
                    .execute_batch("CREATE TEMP TABLE counter(value INTEGER); INSERT INTO counter VALUES(0)")
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let database = Arc::clone(&database);
                std::thread::spawn(move || {
                    for _ in 0..30 {
                        database
                            .execute(|connection| {
                                let value: i64 = connection
                                    .query_row("SELECT value FROM counter", [], |row| row.get(0))
                                    .map_err(|e| e.to_string())?;
                                connection.execute("UPDATE counter SET value=?1", [value + 1]).map_err(|e| e.to_string())?;
                                Ok(())
                            })
                            .unwrap();
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        let value: i64 = database
            .execute(|connection| {
                connection
                    .query_row("SELECT value FROM counter", [], |row| row.get(0))
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        assert_eq!(value, 240);
    }
}
