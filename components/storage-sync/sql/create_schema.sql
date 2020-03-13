-- This Source Code Form is subject to the terms of the Mozilla Public
-- License, v. 2.0. If a copy of the MPL was not distributed with this
-- file, You can obtain one at http://mozilla.org/MPL/2.0/.

-- This is a very simple schema.
CREATE TABLE IF NOT EXISTS moz_extension_data (
    /* The GUID is a hash of the extension ID - we explicitly do not store
       the raw ID in the DB
    */
    guid TEXT PRIMARY KEY,
    /* The JSON payload. not null because we prefer to delete the row than null it */
    data LONGVARCHAR NOT NULL,

    /* Same "sync status" strategy used by other components. */
    syncStatus INTEGER NOT NULL DEFAULT 0,
    syncChangeCounter INTEGER NOT NULL DEFAULT 1
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS moz_extension_data_tombstones (
    guid TEXT PRIMARY KEY
) WITHOUT ROWID;

-- This table holds key-value metadata - primarily for sync.
CREATE TABLE IF NOT EXISTS moz_meta (
    key TEXT PRIMARY KEY,
    value NOT NULL
) WITHOUT ROWID;
