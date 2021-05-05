/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */
use crate::db::LoginDb;
use crate::error::*;
use crate::login::Login;
use crate::LoginsSyncEngine;
use std::cell::RefCell;
use std::path::Path;
use std::sync::{Arc, Mutex, Weak};
use sync15::{sync_multiple, telemetry, EngineSyncAssociation, KeyBundle, Sync15StorageClientInit};

// Our "sync manager" will use whatever is stashed here.
lazy_static::lazy_static! {
    // Mutex: just taken long enough to update the inner stuff - needed
    //        to wrap the RefCell as they aren't `Sync`
    // RefCell: So we can replace what it holds. Normally you'd use `get_ref()`
    //          on the mutex and avoid the RefCell entirely, but that requires
    //          the mutex to be declared as `mut` which is apparently
    //          impossible in a `lazy_static`
    // <[Arc/Weak]<LoginStoreImpl>>: What the sync manager actually needs.
    pub static ref STORE_FOR_MANAGER: Mutex<RefCell<Weak<LoginStoreImpl>>> = Mutex::new(RefCell::new(Weak::new()));
}

// This is the type that uniffi exposes. It holds an `Arc<>` around the
// actual implementation, because we need to hand a clone of this `Arc<>` to
// the sync manager and to sync engines. One day
// https://github.com/mozilla/uniffi-rs/issues/417 will give us access to the
// `Arc<>` uniffi owns, which means we can drop this entirely (ie, `Store` and
// `StoreImpl` could be re-unified)
pub struct LoginStore {
    pub store_impl: Arc<LoginStoreImpl>,
}

impl LoginStore {
    // First up we have the (few) things that want access to our `Arc<>`

    /// A convenience wrapper around sync_multiple.
    // This can almost die later - consumers should never call it (they should
    // use the sync manager) and any of our examples probably can too!
    pub fn sync(
        &self,
        storage_init: &Sync15StorageClientInit,
        root_sync_key: &KeyBundle,
    ) -> Result<telemetry::SyncTelemetryPing> {
        let engine = LoginsSyncEngine::new(Arc::clone(&self.store_impl));

        let mut disk_cached_state = engine.get_global_state()?;
        // Because `sync` should not be used in practice, and because
        // `mem_cached_state` can be considered an optimization (in that it
        // allows subsequent syncs to do less work) we just discard this state,
        // so every sync acts like it's the first one the process has done.
        // (disk_cached_state is far more important - but even in this context
        // it probably doesn't matter, but it's not getting in our way yet)
        let mut mem_cached_state = Default::default();

        let mut result = sync_multiple(
            &[&engine],
            &mut disk_cached_state,
            &mut mem_cached_state,
            storage_init,
            root_sync_key,
            &engine.scope,
            None,
        );
        // We always update the state - sync_multiple does the right thing
        // if it needs to be dropped (ie, they will be None or contain Nones etc)
        engine.set_global_state(&disk_cached_state)?;

        // for b/w compat reasons, we do some dances with the result.
        // XXX - note that this means telemetry isn't going to be reported back
        // to the app - we need to check with lockwise about whether they really
        // need these failures to be reported or whether we can loosen this.
        if let Err(e) = result.result {
            return Err(e.into());
        }
        match result.engine_results.remove("passwords") {
            None | Some(Ok(())) => Ok(result.telemetry),
            Some(Err(e)) => Err(e.into()),
        }
    }

    // This needs our Arc<>
    pub fn register_with_sync_manager(&self) {
        STORE_FOR_MANAGER
            .lock()
            .unwrap()
            .replace(Arc::downgrade(&self.store_impl.clone()));
    }

    // This needs our Arc<>
    pub fn reset(&self) -> Result<()> {
        // Reset should not exist here - all resets should be done via the
        // sync manager. It seems that actual consumers don't use this, but
        // some tests do, so it remains for now.
        let engine = LoginsSyncEngine::new(Arc::clone(&self.store_impl));
        engine.do_reset(&EngineSyncAssociation::Disconnected)?;
        Ok(())
    }

    // Everything below here is a simple delegate to the impl.
    pub fn new(path: impl AsRef<Path>, encryption_key: &str) -> Result<Self> {
        Ok(Self {
            store_impl: Arc::new(LoginStoreImpl::new(path, encryption_key)?),
        })
    }

    pub fn new_with_salt(path: impl AsRef<Path>, encryption_key: &str, salt: &str) -> Result<Self> {
        Ok(Self {
            store_impl: Arc::new(LoginStoreImpl::new_with_salt(path, encryption_key, salt)?),
        })
    }

    pub fn new_in_memory(encryption_key: Option<&str>) -> Result<Self> {
        Ok(Self {
            store_impl: Arc::new(LoginStoreImpl::new_in_memory(encryption_key)?),
        })
    }

    pub fn list(&self) -> Result<Vec<Login>> {
        self.store_impl.list()
    }

    pub fn get(&self, id: &str) -> Result<Option<Login>> {
        self.store_impl.get(id)
    }

    pub fn get_by_base_domain(&self, base_domain: &str) -> Result<Vec<Login>> {
        self.store_impl.get_by_base_domain(base_domain)
    }

    pub fn potential_dupes_ignoring_username(&self, login: Login) -> Result<Vec<Login>> {
        self.store_impl.potential_dupes_ignoring_username(login)
    }

    pub fn touch(&self, id: &str) -> Result<()> {
        self.store_impl.touch(id)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        self.store_impl.delete(id)
    }

    pub fn wipe(&self) -> Result<()> {
        self.store_impl.wipe()
    }

    pub fn wipe_local(&self) -> Result<()> {
        self.store_impl.wipe_local()
    }

    pub fn update(&self, login: Login) -> Result<()> {
        self.store_impl.update(login)
    }

    pub fn add(&self, login: Login) -> Result<String> {
        self.store_impl.add(login)
    }

    pub fn import_multiple(&self, logins: Vec<Login>) -> Result<String> {
        self.store_impl.import_multiple(logins)
    }

    pub fn disable_mem_security(&self) -> Result<()> {
        self.store_impl.disable_mem_security()
    }

    pub fn new_interrupt_handle(&self) -> sql_support::SqlInterruptHandle {
        self.store_impl.new_interrupt_handle()
    }

    pub fn rekey_database(&self, new_encryption_key: &str) -> Result<()> {
        self.store_impl.rekey_database(new_encryption_key)
    }

    pub fn check_valid_with_no_dupes(&self, login: &Login) -> Result<()> {
        self.store_impl.check_valid_with_no_dupes(login)
    }
}

// The actual store implementation.
// This store is a bundle of state to manage the login DB and to help the
// SyncEngine.
// This will go away once uniffi gives us access to its `Arc<>`
pub struct LoginStoreImpl {
    pub db: Mutex<LoginDb>,
}

impl LoginStoreImpl {
    pub fn new(path: impl AsRef<Path>, encryption_key: &str) -> Result<Self> {
        let db = Mutex::new(LoginDb::open(path, Some(encryption_key))?);
        Ok(Self { db })
    }

    pub fn new_with_salt(path: impl AsRef<Path>, encryption_key: &str, salt: &str) -> Result<Self> {
        let db = Mutex::new(LoginDb::open_with_salt(path, encryption_key, salt)?);
        Ok(Self { db })
    }

    pub fn new_in_memory(encryption_key: Option<&str>) -> Result<Self> {
        let db = Mutex::new(LoginDb::open_in_memory(encryption_key)?);
        Ok(Self { db })
    }

    pub fn list(&self) -> Result<Vec<Login>> {
        self.db.lock().unwrap().get_all()
    }

    pub fn get(&self, id: &str) -> Result<Option<Login>> {
        self.db.lock().unwrap().get_by_id(id)
    }

    pub fn get_by_base_domain(&self, base_domain: &str) -> Result<Vec<Login>> {
        self.db.lock().unwrap().get_by_base_domain(base_domain)
    }

    pub fn potential_dupes_ignoring_username(&self, login: Login) -> Result<Vec<Login>> {
        self.db
            .lock()
            .unwrap()
            .potential_dupes_ignoring_username(&login)
    }

    pub fn touch(&self, id: &str) -> Result<()> {
        self.db.lock().unwrap().touch(id)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        self.db.lock().unwrap().delete(id)
    }

    pub fn wipe(&self) -> Result<()> {
        // This should not be exposed - it wipes the server too and there's
        // no good reason to expose that to consumers. wipe_local makes some
        // sense though.
        // TODO: this is exposed to android-components consumers - we should
        // check if anyone actually calls it.
        let db = self.db.lock().unwrap();
        let scope = db.begin_interrupt_scope();
        db.wipe(&scope)?;
        Ok(())
    }

    pub fn wipe_local(&self) -> Result<()> {
        self.db.lock().unwrap().wipe_local()?;
        Ok(())
    }

    pub fn reset(&self) -> Result<()> {
        // Reset should not exist here - all resets should be done via the
        // sync manager. It seems that actual consumers don't use this, but
        // some tests do, so it remains for now.
        let engine = LoginsSyncEngine::new(&self);
        engine.do_reset(&EngineSyncAssociation::Disconnected)?;
        Ok(())
    }

    pub fn update(&self, login: Login) -> Result<()> {
        self.db.lock().unwrap().update(login)
    }

    pub fn add(&self, login: Login) -> Result<String> {
        // Just return the record's ID (which we may have generated).
        self.db
            .lock()
            .unwrap()
            .add(login)
            .map(|record| record.guid().into_string())
    }

    pub fn import_multiple(&self, logins: Vec<Login>) -> Result<String> {
        let metrics = self.db.lock().unwrap().import_multiple(&logins)?;
        Ok(serde_json::to_string(&metrics)?)
    }

    pub fn disable_mem_security(&self) -> Result<()> {
        self.db.lock().unwrap().disable_mem_security()
    }

    pub fn new_interrupt_handle(&self) -> sql_support::SqlInterruptHandle {
        self.db.lock().unwrap().new_interrupt_handle()
    }

    pub fn rekey_database(&self, new_encryption_key: &str) -> Result<()> {
        self.db.lock().unwrap().rekey_database(new_encryption_key)
    }

    pub fn check_valid_with_no_dupes(&self, login: &Login) -> Result<()> {
        self.db.lock().unwrap().check_valid_with_no_dupes(&login)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util;
    use more_asserts::*;
    use std::cmp::Reverse;
    use std::time::SystemTime;
    // Doesn't check metadata fields
    fn assert_logins_equiv(a: &Login, b: &Login) {
        assert_eq!(b.guid(), a.guid());
        assert_eq!(b.hostname, a.hostname);
        assert_eq!(b.form_submit_url, a.form_submit_url);
        assert_eq!(b.http_realm, a.http_realm);
        assert_eq!(b.username, a.username);
        assert_eq!(b.password, a.password);
        assert_eq!(b.username_field, a.username_field);
        assert_eq!(b.password_field, a.password_field);
    }

    #[test]
    fn test_general() {
        let store = LoginStore::new_in_memory(Some("secret")).unwrap();
        let list = store.list().expect("Grabbing Empty list to work");
        assert_eq!(list.len(), 0);
        let start_us = util::system_time_ms_i64(SystemTime::now());

        let a = Login {
            id: "aaaaaaaaaaaa".into(),
            hostname: "https://www.example.com".into(),
            form_submit_url: Some("https://www.example.com".into()),
            username: "coolperson21".into(),
            password: "p4ssw0rd".into(),
            username_field: "user_input".into(),
            password_field: "pass_input".into(),
            ..Login::default()
        };

        let b = Login {
            // Note: no ID, should be autogenerated for us
            hostname: "https://www.example2.com".into(),
            http_realm: Some("Some String Here".into()),
            username: "asdf".into(),
            password: "fdsa".into(),
            ..Login::default()
        };
        let a_id = store.add(a.clone()).expect("added a");
        let b_id = store.add(b.clone()).expect("added b");

        assert_eq!(a_id, a.guid());

        assert_ne!(b_id, b.guid(), "Should generate guid when none provided");

        let a_from_db = store
            .get(&a_id)
            .expect("Not to error getting a")
            .expect("a to exist");

        assert_logins_equiv(&a, &a_from_db);
        assert_ge!(a_from_db.time_created, start_us);
        assert_ge!(a_from_db.time_password_changed, start_us);
        assert_ge!(a_from_db.time_last_used, start_us);
        assert_eq!(a_from_db.times_used, 1);

        let b_from_db = store
            .get(&b_id)
            .expect("Not to error getting b")
            .expect("b to exist");

        assert_logins_equiv(
            &b_from_db,
            &Login {
                id: b_id.to_string(),
                ..b.clone()
            },
        );
        assert_ge!(b_from_db.time_created, start_us);
        assert_ge!(b_from_db.time_password_changed, start_us);
        assert_ge!(b_from_db.time_last_used, start_us);
        assert_eq!(b_from_db.times_used, 1);

        let mut list = store.list().expect("Grabbing list to work");
        assert_eq!(list.len(), 2);

        let mut expect = vec![a_from_db, b_from_db.clone()];

        list.sort_by_key(|b| Reverse(b.guid()));
        expect.sort_by_key(|b| Reverse(b.guid()));
        assert_eq!(list, expect);

        store.delete(&a_id).expect("Successful delete");
        assert!(store
            .get(&a_id)
            .expect("get after delete should still work")
            .is_none());

        let list = store.list().expect("Grabbing list to work");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], b_from_db);

        let list = store
            .get_by_base_domain("example2.com")
            .expect("Expect a list for this hostname");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], b_from_db);

        let list = store
            .get_by_base_domain("www.example.com")
            .expect("Expect an empty list");
        assert_eq!(list.len(), 0);

        let now_us = util::system_time_ms_i64(SystemTime::now());
        let b2 = Login {
            password: "newpass".into(),
            id: b_id.to_string(),
            ..b
        };

        store.update(b2.clone()).expect("update b should work");

        let b_after_update = store
            .get(&b_id)
            .expect("Not to error getting b")
            .expect("b to exist");

        assert_logins_equiv(&b_after_update, &b2);
        assert_ge!(b_after_update.time_created, start_us);
        assert_le!(b_after_update.time_created, now_us);
        assert_ge!(b_after_update.time_password_changed, now_us);
        assert_ge!(b_after_update.time_last_used, now_us);
        // Should be two even though we updated twice
        assert_eq!(b_after_update.times_used, 2);
    }

    #[test]
    fn test_rekey() {
        let store = LoginStore::new_in_memory(Some("secret")).unwrap();
        store.rekey_database("new_encryption_key").unwrap();
        let list = store.list().expect("Grabbing Empty list to work");
        assert_eq!(list.len(), 0);
    }
    #[test]
    fn test_sync_manager_registration() {
        let store = LoginStore::new_in_memory(Some("sync-manager")).unwrap();
        assert_eq!(Arc::strong_count(&store.store_impl), 1);
        assert_eq!(Arc::weak_count(&store.store_impl), 0);
        store.register_with_sync_manager();
        assert_eq!(Arc::strong_count(&store.store_impl), 1);
        assert_eq!(Arc::weak_count(&store.store_impl), 1);
        let registered = STORE_FOR_MANAGER
            .lock()
            .unwrap()
            .borrow()
            .upgrade()
            .expect("should upgrade");
        assert!(Arc::ptr_eq(&store.store_impl, &registered));
        drop(registered);
        // should be no new references
        assert_eq!(Arc::strong_count(&store.store_impl), 1);
        assert_eq!(Arc::weak_count(&store.store_impl), 1);
        // dropping the registered object should drop the registration.
        drop(store);
        assert!(STORE_FOR_MANAGER
            .lock()
            .unwrap()
            .borrow()
            .upgrade()
            .is_none());
    }
}

#[test]
fn test_send() {
    fn ensure_send<T: Send>() {}
    ensure_send::<LoginStore>();
}
