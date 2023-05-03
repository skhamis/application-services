/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! This module defines all the information needed to match a user with an experiment.
//! Soon it will also include a `match` function of some sort that does the matching.
//!
//! It contains the `AppContext`
//! provided by the consuming client.
//!
use serde_derive::*;
use serde_json::{Map, Value};

/// The `AppContext` object represents the parameters and characteristics of the
/// consuming application that we are interested in for targeting purposes. The
/// `app_name` and `channel` fields are not optional as they are expected
/// to be provided by all consuming applications as they are used in the top-level
/// targeting that help to ensure that an experiment is only processed
/// by the correct application.
///
/// Definitions of the fields are as follows:
/// - `app_name`: This is the name of the application (e.g. "Fenix" or "Firefox iOS")
/// - `app_id`: This is the application identifier, especially for mobile (e.g. "org.mozilla.fenix")
/// - `channel`: This is the delivery channel of the application (e.g "nightly")
/// - `app_version`: The user visible version string (e.g. "1.0.3")
/// - `app_build`: The build identifier generated by the CI system (e.g. "1234/A")
/// - `architecture`: The architecture of the device, (e.g. "arm", "x86")
/// - `device_manufacturer`: The manufacturer of the device the application is running on
/// - `device_model`: The model of the device the application is running on
/// - `locale`: The locale of the application during initialization (e.g. "es-ES")
/// - `os`: The name of the operating system (e.g. "Android", "iOS", "Darwin", "Windows")
/// - `os_version`: The user-visible version of the operating system (e.g. "1.2.3")
/// - `android_sdk_version`: Android specific for targeting specific sdk versions
/// - `debug_tag`: Used for debug purposes as a way to match only developer builds, etc.
/// - `installation_date`: The date the application installed the app
/// - `home_directory`: The application's home directory
/// - `custom_targeting_attributes`: Contains attributes specific to the application, derived by the application
#[cfg(feature = "stateful")]
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct AppContext {
    pub app_name: String,
    pub app_id: String,
    pub channel: String,
    pub app_version: Option<String>,
    pub app_build: Option<String>,
    pub architecture: Option<String>,
    pub device_manufacturer: Option<String>,
    pub device_model: Option<String>,
    pub locale: Option<String>,
    pub os: Option<String>,
    pub os_version: Option<String>,
    pub android_sdk_version: Option<String>,
    pub debug_tag: Option<String>,
    pub installation_date: Option<i64>,
    pub home_directory: Option<String>,
    #[serde(flatten)]
    pub custom_targeting_attributes: Option<Map<String, Value>>,
}

/// The `AppContext` object represents the parameters and characteristics of the
/// consuming application that we are interested in for targeting purposes. The
/// `app_name`, `app_id` and `channel` fields are not optional as they are expected
/// to be provided by all consuming applications as they are used in the top-level
/// targeting that help to ensure that an experiment is only processed
/// by the correct application.
///
/// Definitions of the fields are as follows:
/// - `app_name`: This is the name of the application (e.g. "Fenix" or "Firefox iOS")
/// - `app_id`: This is the application identifier, especially for mobile (e.g. "org.mozilla.fenix")
/// - `channel`: This is the delivery channel of the application (e.g "nightly")
/// - `app_version`: The user visible version string (e.g. "1.0.3")
/// - `app_build`: The build identifier generated by the CI system (e.g. "1234/A")
/// - `locale`: The locale of the application during initialization (e.g. "es-ES")
/// - `os`: The name of the operating system (e.g. "Android", "iOS", "Darwin", "Windows")
/// - `os_version`: The user-visible version of the operating system (e.g. "1.2.3")
/// - `user_agent`: The user agent as defined by the browser (e.g. "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:109.0) Gecko/20100101 Firefox/114.0" )
/// - `custom_targeting_attributes`: Contains attributes specific to the application, derived by the application
#[cfg(not(feature = "stateful"))]
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct AppContext {
    pub app_name: String,
    pub app_id: String,
    pub channel: String,
    pub app_version: Option<String>,
    pub app_build: Option<String>,
    #[serde(flatten)]
    pub custom_targeting_attributes: Option<Map<String, Value>>,
}
