/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

 use serde_derive::*;
use super::{commands::send_tab::{
    self, EncryptedSendTabPayload, PrivateSendTabKeys, PublicSendTabKeys,
    SendTabKeysPayload, SendTabPayload,
}, scopes, FirefoxAccount, device::Device};
use crate::{Result, Error, ScopedKey};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rc_crypto::ece::{self, EcKeyComponents};
use sync15::{EncryptedPayload, KeyBundle};

pub const COMMAND_NAME: &str = "https://identity.mozilla.com/cmd/open-uri";

impl FirefoxAccount {
    pub fn close_remote_tab(
        &mut self,
        target_device_id: &str,
        url: &str,
    ) -> Result<()> {
        log::debug!("close_remote_tab called -- url:{} device: {}!", target_device_id, url);
        
        // Copied from send tab, 
        let devices = self.get_devices(false)?;
        let target = devices
            .iter()
            .find(|d| d.id == target_device_id)
            .ok_or_else(|| Error::UnknownTargetDevice(target_device_id.to_owned()))?;
        let payload = RemoteTabPayload::close_remote_tab(url);
        let oldsync_key = self.get_scoped_key(scopes::OLD_SYNC)?;
        let command_payload = build_send_command(oldsync_key, target, &payload)?;
        self.invoke_command(send_tab::COMMAND_NAME, target, &command_payload)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteTabPayload {
    pub actions: Vec<RemoteTabAction>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteTabAction {
    pub url: String,
}

impl RemoteTabPayload {
    pub fn close_remote_tab(url: &str) -> Self {
            RemoteTabPayload {
                actions: vec![RemoteTabAction {
                    url: url.to_string(),
                }],
            }
    }
    fn encrypt(&self, keys: PublicSendTabKeys) -> Result<EncryptedSendTabPayload> {
        rc_crypto::ensure_initialized();
        let bytes = serde_json::to_vec(&self)?;
        let public_key = URL_SAFE_NO_PAD.decode(&keys.public_key)?;
        let auth_secret = URL_SAFE_NO_PAD.decode(&keys.auth_secret)?;
        let encrypted = ece::encrypt(&public_key, &auth_secret, &bytes)?;
        let encrypted = URL_SAFE_NO_PAD.encode(encrypted);
        Ok(EncryptedSendTabPayload { encrypted })
    }
}

pub fn build_send_command(
    scoped_key: &ScopedKey,
    target: &Device,
    remote_tab_payload: &RemoteTabPayload,
) -> Result<serde_json::Value> {
    let command = target
        .available_commands
        .get(COMMAND_NAME)
        .ok_or(Error::UnsupportedCommand(COMMAND_NAME))?;
    let bundle: SendTabKeysPayload = serde_json::from_str(command)?;
    let public_keys = bundle.decrypt(scoped_key)?;
    let encrypted_payload = remote_tab_payload.encrypt(public_keys)?;
    Ok(serde_json::to_value(encrypted_payload)?)
}