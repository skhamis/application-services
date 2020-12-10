/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this
* file, You can obtain one at http://mozilla.org/MPL/2.0/.
*/

pub mod incoming;

use sync_guid::Guid as SyncGuid;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum RecordData {
    Data {
        pub given_name: String,

        #[serde(default)]
        pub additional_name: String,

        pub family_name: String,

        #[serde(default)]
        pub organization: String,

        pub street_address: String,

        #[serde(default)]
        pub address_level3: String,

        #[serde(default)]
        pub address_level2: String,

        #[serde(default)]
        pub address_level1: String,

        #[serde(default)]
        pub postal_code: String,

        #[serde(default)]
        pub country: String,

        #[serde(default)]
        pub tel: String,

        #[serde(default)]
        pub email: String,

        // #[serde(default)]
        // pub time_created: Timestamp,

        // #[serde(default)]
        // pub time_last_used: Option<Timestamp>,

        // #[serde(default)]
        // pub time_last_modified: Timestamp,

        // #[serde(default)]
        // pub times_used: i64,
    },
    #[serde(skip_deserializing)]
    Tombstone,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    #[serde(rename = "id")]
    guid: SyncGuid,
    #[serde(flatten, deserialize_with = "deserialize_record_data")]
    data: AddressRecordData,
}