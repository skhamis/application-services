/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::storage::{ClientRemoteTabs, RemoteTab, TabsStorage};
use std::path::Path;
use parking_lot::Mutex;

// static TABS_INSTANCE: OnceCell<Arc<TabsStore>> = OnceCell::new();


pub struct TabsStore {
    pub storage: Mutex<TabsStorage>,
}

impl TabsStore {
    pub fn new(db_path: impl AsRef<Path>) -> Self {
        Self {
            storage: Mutex::new(TabsStorage::new(db_path)),
        }
    }

    pub fn new_with_mem_path(db_path: &str) -> Self {
        Self {
            storage: Mutex::new(TabsStorage::new_with_mem_path(db_path)),
        }
    }

    // pub fn initialize_tabs_store(db_path: impl AsRef<Path>) -> Result<(), &'static str> {
    //     let store: TabsStore = TabsStore::new(db_path);
    //     TABS_INSTANCE.set(Arc::new(store))
    //         .map_err(|_| "TabsStore instance was already initialized")
    //}

    pub fn set_local_tabs(&self, local_state: Vec<RemoteTab>) {
        self.storage.lock().update_local_state(local_state);
    }

    // like remote_tabs, but serves the uniffi layer
    pub fn get_all(&self) -> Vec<ClientRemoteTabs> {
        match self.remote_tabs() {
            Some(list) => list,
            None => vec![],
        }
    }

    pub fn remote_tabs(&self) -> Option<Vec<ClientRemoteTabs>> {
        self.storage.lock().get_remote_tabs()
    }

    pub fn add_remote_tabs_to_pending_delete(&self, tabs_to_close: Vec<RemoteTab>) {
        self.storage.lock().do_some_db_work()
    }

}

// // Test, this gets the tab store, assumes this is already initalized in the running app
// pub fn add_remote_tabs_to_pending_delete() {
//     // Get the instance of tabs
//     let tabs_store = get_tabs_store();
//     tabs_store.do_some_db_work()
// }

// pub fn get_tabs_store() -> &'static Arc<TabsStore> {
//     TABS_INSTANCE.get().expect("TabsStore has not been initialized")
// }