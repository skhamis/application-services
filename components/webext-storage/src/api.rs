/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::db::StorageConn;
use crate::error::*;
use serde_json::Map;
use serde_json::Value as JsonValue;
use sql_support::{self, ConnExt};

type JsonMap = Map<String, JsonValue>; // no idea why Map<> isn't already this?

fn get_from_db(conn: &StorageConn, ext_guid: &str) -> Result<Option<JsonMap>> {
    Ok(
        match conn.try_query_one::<String>(
            "SELECT data FROM moz_extension_data
             WHERE guid = :guid",
            &[(":guid", &ext_guid)],
            true,
        )? {
            Some(s) => match serde_json::from_str(&s)? {
                JsonValue::Object(m) => Some(m),
                // we could panic here as it's theoretically impossible, but we
                // might as well treat it as not existing...
                _ => None,
            },
            None => None,
        },
    )
}

fn save_to_db(conn: &StorageConn, ext_guid: &str, val: &JsonValue) -> Result<()> {
    // XXX - sync support will need to do the syncStatus thing here.
    conn.execute_named(
        "INSERT OR REPLACE INTO moz_extension_data(guid, data)
            VALUES (:guid, :data)",
        &[(":guid", &ext_guid), (":data", &val.to_string())],
    )?;
    Ok(())
}

fn remove_from_db(conn: &StorageConn, ext_guid: &str) -> Result<()> {
    // XXX - sync support will need to do the tombstone thing here.
    conn.execute_named(
        "DELETE FROM moz_extension_data
        WHERE guid = :guid",
        &[(":guid", &ext_guid)],
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageChange {
    #[serde(skip_serializing_if = "Option::is_none")]
    old_value: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_value: Option<JsonValue>,
}

// XXX - TODO - enforce quotas!
// XXX - the shape of StorageChange is wrong! Instead of, say:
//  {oldValue: {foo: "old"}, newValue: {foo: "new"}}
// it should be:
//  {foo: {oldValue: "old", newValue: "new"}}
pub fn set(conn: &StorageConn, ext_guid: &str, val: JsonValue) -> Result<StorageChange> {
    // XXX - Should we consider making this function  take a &str, and parse
    // it ourselves? That way we could avoid parsing entirely if no existing
    // value (but presumably that's going to be the uncommon case, so it probably
    // doesn't matter)
    let existing = get_from_db(conn, ext_guid)?;
    let old_value = existing.clone();

    let new_map = match existing {
        Some(mut e) => {
            let new = match val {
                JsonValue::Object(m) => m,
                // Not clear what the error semantics should be yet. For now, pretend an empty map.
                _ => Map::new(),
            };
            // iterate over "new", updating without recursing.
            for (k, v) in new.into_iter() {
                e.insert(k, v);
            }
            e
        }
        None => match val {
            JsonValue::Object(m) => m,
            // Not clear what the error semantics should be yet. For now, pretend an empty map.
            _ => Map::new(),
        },
    };

    let new_value = JsonValue::Object(new_map);
    save_to_db(conn, ext_guid, &new_value)?;
    Ok(StorageChange {
        old_value: old_value.map(JsonValue::Object),
        new_value: Some(new_value),
    })
}

// XXX - is this signature OK? We never return None, only Null
pub fn get(conn: &StorageConn, ext_guid: &str, key: JsonValue) -> Result<JsonValue> {
    // key is optional, or string or array of string or object keys
    let maybe_existing = get_from_db(conn, ext_guid)?;
    let mut existing = match maybe_existing {
        None => return Ok(JsonValue::Null),
        Some(v) => v,
    };
    Ok(match key {
        JsonValue::Null => JsonValue::Object(existing),
        JsonValue::String(s) => {
            let mut result = Map::with_capacity(1);
            if let Some(v) = existing.remove(&s) {
                result.insert(s, v);
            }
            JsonValue::Object(result)
        }
        JsonValue::Array(keys) => {
            // because nothing with json is ever simple, each key may not be
            // a string. We ignore any which aren't.
            let max_num_keys = keys.len();
            let skeys = keys
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()));
            let mut result = Map::with_capacity(max_num_keys);
            // XXX - assume that if key doesn't exist, it doesn't exist in the result.
            for key in skeys {
                if let Some(v) = existing.remove(&key) {
                    result.insert(key.to_string(), v);
                }
            }
            JsonValue::Object(result)
        }
        // XXX - I guess we never fail even when invalid JSON values?
        _ => JsonValue::Null,
    })
}

pub fn clear(conn: &StorageConn, ext_guid: &str) -> Result<()> {
    remove_from_db(conn, ext_guid)
}

// XXX - get_bytes_in_use()

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test::new_mem_connection;
    use serde_json::json;

    #[test]
    fn test_simple() -> Result<()> {
        let ext_id = "x";
        let conn = new_mem_connection();

        assert_eq!(get(&conn, &ext_id, JsonValue::Null)?, JsonValue::Null);
        // XXX - all other JsonValue variants should return null.

        assert_eq!(
            set(&conn, &ext_id, json!({"foo": "bar" }))?,
            StorageChange {
                old_value: None,
                new_value: Some(json!({"foo": "bar" }))
            }
        );
        assert_eq!(
            get(&conn, &ext_id, JsonValue::Null)?,
            json!({"foo": "bar" })
        );
        // XXX - other variants.

        assert_eq!(
            set(
                &conn,
                &ext_id,
                json!({"foo": "new",
                                       "other": "also new" })
            )?,
            StorageChange {
                old_value: Some(json!({"foo": "bar" })),
                new_value: Some(json!({"foo": "new",
                                       "other": "also new"}))
            }
        );
        assert_eq!(
            get(&conn, &ext_id, JsonValue::Null)?,
            json!({"foo": "new",
                                       "other": "also new"})
        );
        // XXX - other variants.

        clear(&conn, &ext_id)?;
        assert_eq!(get(&conn, &ext_id, JsonValue::Null)?, JsonValue::Null);

        Ok(())
    }
}
