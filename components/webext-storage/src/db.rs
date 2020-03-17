/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::error::*;
use crate::schema;
use lazy_static::lazy_static;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use sql_support::{ConnExt, SqlInterruptHandle, SqlInterruptScope};
use std::collections::HashMap;
use std::fs;
use std::mem;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex, Weak,
};
use url::Url;

//pub const MAX_VARIABLE_NUMBER: usize = 999;

// We only allow a single StorageDb per filename.
lazy_static! {
    static ref APIS: Mutex<HashMap<PathBuf, Weak<StorageDb>>> = Mutex::new(HashMap::new());
}

static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The entry-point to the database API. This object ensures a singleton per
/// filename, gives access to database connections and other helpers, etc.
/// It also enforces that only 1 write connection can exist to the database at
/// once.
pub struct StorageDb {
    db_name: PathBuf,
    write_connection: Mutex<Option<StorageConn>>,
    coop_tx_lock: Arc<Mutex<()>>,
    sync_conn_active: AtomicBool,
    id: usize,
}
impl StorageDb {
    /// Create a new, or fetch an already open, StorageDb backed by a file on disk.
    pub fn new(db_name: impl AsRef<Path>) -> Result<Arc<Self>> {
        let db_name = normalize_path(db_name)?;
        Self::new_or_existing(db_name)
    }

    /// Create a new, or fetch an already open, memory-based StorageDb. You must
    /// provide a name, but you are still able to have a single writer and many
    ///  reader connections to the same memory DB open.
    pub fn new_memory(db_name: &str) -> Result<Arc<Self>> {
        let name = PathBuf::from(format!("file:{}?mode=memory&cache=shared", db_name));
        Self::new_or_existing(name)
    }
    fn new_or_existing_into(
        target: &mut HashMap<PathBuf, Weak<StorageDb>>,
        db_name: PathBuf,
        delete_on_fail: bool,
    ) -> Result<Arc<Self>> {
        let id = ID_COUNTER.fetch_add(1, Ordering::SeqCst);
        match target.get(&db_name).and_then(Weak::upgrade) {
            Some(existing) => Ok(existing),
            None => {
                // We always create a new read-write connection for an initial open so
                // we can create the schema and/or do version upgrades.
                let coop_tx_lock = Arc::new(Mutex::new(()));
                match StorageConn::open(
                    &db_name,
                    ConnectionType::ReadWrite,
                    id,
                    coop_tx_lock.clone(),
                ) {
                    Ok(connection) => {
                        let new = StorageDb {
                            db_name: db_name.clone(),
                            write_connection: Mutex::new(Some(connection)),
                            sync_conn_active: AtomicBool::new(false),
                            id,
                            coop_tx_lock,
                        };
                        let arc = Arc::new(new);
                        target.insert(db_name, Arc::downgrade(&arc));
                        Ok(arc)
                    }
                    Err(e) => {
                        if !delete_on_fail {
                            return Err(e);
                        }
                        if let ErrorKind::DatabaseUpgradeError = e.kind() {
                            fs::remove_file(&db_name)?;
                            Self::new_or_existing_into(target, db_name, false)
                        } else {
                            Err(e)
                        }
                    }
                }
            }
        }
    }

    fn new_or_existing(db_name: PathBuf) -> Result<Arc<Self>> {
        let mut guard = APIS.lock().unwrap();
        Self::new_or_existing_into(&mut guard, db_name, true)
    }

    /// Open a connection to the database.
    pub fn open_connection(&self, conn_type: ConnectionType) -> Result<StorageConn> {
        match conn_type {
            ConnectionType::ReadOnly => {
                // make a new one - we can have as many of these as we want.
                StorageConn::open(
                    self.db_name.clone(),
                    ConnectionType::ReadOnly,
                    self.id,
                    self.coop_tx_lock.clone(),
                )
            }
            ConnectionType::ReadWrite => {
                // We only allow one of these.
                let mut guard = self.write_connection.lock().unwrap();
                match mem::replace(&mut *guard, None) {
                    None => Err(ErrorKind::ConnectionAlreadyOpen.into()),
                    Some(db) => Ok(db),
                }
            }
            ConnectionType::Sync => {
                panic!("Use `open_sync_connection` to open a sync connection");
            }
        }
    }

    pub fn open_sync_connection(&self) -> Result<SyncConn<'_>> {
        let prev_value = self
            .sync_conn_active
            .compare_and_swap(false, true, Ordering::SeqCst);
        if prev_value {
            Err(ErrorKind::ConnectionAlreadyOpen.into())
        } else {
            let db = StorageConn::open(
                self.db_name.clone(),
                ConnectionType::Sync,
                self.id,
                self.coop_tx_lock.clone(),
            )?;
            Ok(SyncConn {
                db,
                flag: &self.sync_conn_active,
            })
        }
    }

    /// Close a connection to the database. If the connection is the write
    /// connection, you can re-fetch it using open_connection.
    pub fn close_connection(&self, connection: StorageConn) -> Result<()> {
        if connection.api_id() != self.id {
            return Err(ErrorKind::WrongApiForClose.into());
        }
        if connection.conn_type() == ConnectionType::ReadWrite {
            // We only allow one of these.
            let mut guard = self.write_connection.lock().unwrap();
            assert!((*guard).is_none());
            *guard = Some(connection);
        }
        Ok(())
    }

    /// Get a new interrupt handle for the sync connection.
    pub fn new_sync_conn_interrupt_handle(&self) -> Result<SqlInterruptHandle> {
        // XXX - places takes a lock here, but comments that it's not necessary.
        // We shouldn't need a "SyncState" for any other reason, so we've dropped
        // it!
        let conn = self.open_sync_connection()?;
        Ok(conn.new_interrupt_handle())
    }
}

/// Wrapper around StorageConn that automatically sets a flag (`sync_conn_active`)
/// to false when finished
pub struct SyncConn<'api> {
    db: StorageConn,
    flag: &'api AtomicBool,
}

impl<'a> Drop for SyncConn<'a> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst)
    }
}

impl<'a> std::ops::Deref for SyncConn<'a> {
    type Target = StorageConn;
    fn deref(&self) -> &StorageConn {
        &self.db
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ConnectionType {
    ReadOnly = 1,
    ReadWrite = 2,
    Sync = 3,
}

impl ConnectionType {
    pub fn from_primitive(p: u8) -> Option<Self> {
        match p {
            1 => Some(ConnectionType::ReadOnly),
            2 => Some(ConnectionType::ReadWrite),
            3 => Some(ConnectionType::Sync),
            _ => None,
        }
    }
}

impl ConnectionType {
    pub fn rusqlite_flags(self) -> OpenFlags {
        let common_flags = OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI;
        match self {
            ConnectionType::ReadOnly => common_flags | OpenFlags::SQLITE_OPEN_READ_ONLY,
            ConnectionType::ReadWrite => {
                common_flags | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE
            }
            ConnectionType::Sync => common_flags | OpenFlags::SQLITE_OPEN_READ_WRITE,
        }
    }
}

#[derive(Debug)]
pub struct StorageConn {
    pub sqldb: Connection,
    conn_type: ConnectionType,
    interrupt_counter: Arc<AtomicUsize>,
    api_id: usize,
    pub(super) coop_tx_lock: Arc<Mutex<()>>,
}

impl StorageConn {
    fn with_sql_connection(
        db: Connection,
        conn_type: ConnectionType,
        api_id: usize,
        coop_tx_lock: Arc<Mutex<()>>,
    ) -> Result<Self> {
        let initial_pragmas = "
            -- Cargo-culted from places.
            PRAGMA page_size = 32768;

            -- Disable calling mlock/munlock for every malloc/free.
            -- In practice this results in a massive speedup, especially
            -- for insert-heavy workloads.
            -- XXX - is this relevant?
            PRAGMA cipher_memory_security = false;

            -- `temp_store = 2` - also cargo-culted - See both places and
            -- https://github.com/mozilla/mentat/issues/505.
            PRAGMA temp_store = 2;

            -- Moar cargo-cult.
            PRAGMA cache_size = -6144;

            -- We probably do NOT want foreign-key support?
            -- PRAGMA foreign_keys = ON;

            -- we unconditionally want write-ahead-logging mode
            PRAGMA journal_mode=WAL;

            -- How often to autocheckpoint (in units of pages).
            -- 2048000 (our max desired WAL size) / 32760 (page size).
            PRAGMA wal_autocheckpoint=62
        ";

        db.execute_batch(initial_pragmas)?;
        define_functions(&db)?;
        db.set_prepared_statement_cache_capacity(128);
        let res = Self {
            sqldb: db,
            conn_type,
            // The API sets this explicitly.
            api_id,
            interrupt_counter: Arc::new(AtomicUsize::new(0)),
            coop_tx_lock,
        };
        match res.conn_type() {
            // For read-only connections, we can avoid opening a transaction,
            // since we know we won't be migrating or initializing anything.
            ConnectionType::ReadOnly => {}
            _ => {
                // Even though we're the owner of the db, we need it to be an unchecked tx
                // since we want to pass &StorageConn and not &Connection to schema::init.
                let tx = res.unchecked_transaction()?;
                schema::init(&res)?;
                tx.commit()?;
            }
        }

        Ok(res)
    }

    pub fn open(
        path: impl AsRef<Path>,
        conn_type: ConnectionType,
        api_id: usize,
        coop_tx_lock: Arc<Mutex<()>>,
    ) -> Result<Self> {
        Ok(Self::with_sql_connection(
            Connection::open_with_flags(path, conn_type.rusqlite_flags())?,
            conn_type,
            api_id,
            coop_tx_lock,
        )?)
    }

    pub fn new_interrupt_handle(&self) -> SqlInterruptHandle {
        SqlInterruptHandle::new(
            self.sqldb.get_interrupt_handle(),
            self.interrupt_counter.clone(),
        )
    }

    #[inline]
    pub fn begin_interrupt_scope(&self) -> SqlInterruptScope {
        SqlInterruptScope::new(self.interrupt_counter.clone())
    }

    #[inline]
    pub fn conn_type(&self) -> ConnectionType {
        self.conn_type
    }

    #[inline]
    pub fn api_id(&self) -> usize {
        self.api_id
    }
}

impl Drop for StorageConn {
    fn drop(&mut self) {
        // In line with both the recommendations from SQLite and the behavior of places in
        // Database.cpp, we run `PRAGMA optimize` before closing the connection.
        let res = self.sqldb.execute_batch("PRAGMA optimize(0x02);");
        if let Err(e) = res {
            log::warn!("Failed to execute pragma optimize (DB locked?): {}", e);
        }
    }
}

impl ConnExt for StorageConn {
    #[inline]
    fn conn(&self) -> &Connection {
        &self.sqldb
    }
}

impl Deref for StorageConn {
    type Target = Connection;
    #[inline]
    fn deref(&self) -> &Connection {
        &self.sqldb
    }
}

fn define_functions(_c: &Connection) -> Result<()> {
    Ok(())
}

// Utilities for working with paths.
// (From places_utils - ideally these would be shared, but the use of
// ErrorKind values makes that non-trivial.

/// `Path` is basically just a `str` with no validation, and so in practice it
/// could contain a file URL. Rusqlite takes advantage of this a bit, and says
/// `AsRef<Path>` but really means "anything sqlite can take as an argument".
///
/// Swift loves using file urls (the only support it has for file manipulation
/// is through file urls), so it's handy to support them if possible.
fn unurl_path(p: impl AsRef<Path>) -> PathBuf {
    p.as_ref()
        .to_str()
        .and_then(|s| Url::parse(s).ok())
        .and_then(|u| {
            if u.scheme() == "file" {
                u.to_file_path().ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| p.as_ref().to_owned())
}

/// If `p` is a file URL, return it, otherwise try and make it one.
///
/// Errors if `p` is a relative non-url path, or if it's a URL path
/// that's isn't a `file:` URL.
pub fn ensure_url_path(p: impl AsRef<Path>) -> Result<Url> {
    if let Some(u) = p.as_ref().to_str().and_then(|s| Url::parse(s).ok()) {
        if u.scheme() == "file" {
            Ok(u)
        } else {
            Err(ErrorKind::IllegalDatabasePath(p.as_ref().to_owned()).into())
        }
    } else {
        let p = p.as_ref();
        let u = Url::from_file_path(p).map_err(|_| ErrorKind::IllegalDatabasePath(p.to_owned()))?;
        Ok(u)
    }
}

/// As best as possible, convert `p` into an absolute path, resolving
/// all symlinks along the way.
///
/// If `p` is a file url, it's converted to a path before this.
fn normalize_path(p: impl AsRef<Path>) -> Result<PathBuf> {
    let path = unurl_path(p);
    if let Ok(canonical) = path.canonicalize() {
        return Ok(canonical);
    }
    // It probably doesn't exist yet. This is an error, although it seems to
    // work on some systems.
    //
    // We resolve this by trying to canonicalize the parent directory, and
    // appending the requested file name onto that. If we can't canonicalize
    // the parent, we return an error.
    //
    // Also, we return errors if the path ends in "..", if there is no
    // parent directory, etc.
    let file_name = path
        .file_name()
        .ok_or_else(|| ErrorKind::IllegalDatabasePath(path.clone()))?;

    let parent = path
        .parent()
        .ok_or_else(|| ErrorKind::IllegalDatabasePath(path.clone()))?;

    let mut canonical = parent.canonicalize()?;
    canonical.push(file_name);
    Ok(canonical)
}

// Helpers for tests
#[cfg(test)]
pub mod test {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // A helper for our tests to get their own memory Api.
    static ATOMIC_COUNTER: AtomicUsize = AtomicUsize::new(0);

    pub fn new_mem_api() -> Arc<StorageDb> {
        let counter = ATOMIC_COUNTER.fetch_add(1, Ordering::Relaxed);
        StorageDb::new_memory(&format!("test-api-{}", counter)).expect("should get an API")
    }

    pub fn new_mem_connection() -> StorageConn {
        new_mem_api()
            .open_connection(ConnectionType::ReadWrite)
            .expect("should get a connection")
    }

    pub struct MemConnections {
        pub read: StorageConn,
        pub write: StorageConn,
        pub api: Arc<StorageDb>,
    }

    pub fn new_mem_connections() -> MemConnections {
        let api = new_mem_api();
        let read = api
            .open_connection(ConnectionType::ReadOnly)
            .expect("should get a read connection");
        let write = api
            .open_connection(ConnectionType::ReadWrite)
            .expect("should get a write connection");
        MemConnections { api, read, write }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    // Sanity check that we can create a database.
    #[test]
    fn test_open() {
        StorageConn::with_sql_connection(
            Connection::open_in_memory().expect("no connection"),
            ConnectionType::ReadWrite,
            0,
            Arc::new(Mutex::new(())),
        )
        .expect("no memory db");
    }
}
