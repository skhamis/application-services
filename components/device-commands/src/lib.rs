/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use tabs::RemoteTabRecord;
use tabs::TabsStore;

uniffi::include_scaffolding!("device_commands");

// Test 2: Having a dedicated crate call into FxA/Tabs?
pub fn close_remote_tabs(tabs_store: &TabsStore, tabs_to_close: Vec<RemoteTabRecord>) {

    // Investigate being able to call both FxA and tabs via this command
    // tell FxA to send the Push notification
    // tell tabs engine that we're closing the tab
}