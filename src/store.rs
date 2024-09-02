use std::{collections::HashMap, path::PathBuf};

use aes_gcm::{
    aead::{Aead, OsRng},
    AeadCore, Aes256Gcm, Key, KeyInit,
};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// a store of all of the encrypted variables in cryptenv
pub struct Store {
    vars: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct EncryptedVariable<'a> {
    value: &'a str,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

#[derive(Debug, ZeroizeOnDrop)]
pub struct DecryptedVariable<'a> {
    value: String,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

impl Store {
    /// read the store from disk
    /// reads from dirs::data_dir()/cryptenv/store.json
    pub fn read() -> Self {
        let path = Store::get_path();

        if !path.exists() {
            return Store {
                vars: HashMap::new(),
            };
        }

        let store = std::fs::read_to_string(&path).expect("Could not read store file");

        serde_json::from_str(&store).expect("Could not parse store file")
    }

    pub fn save_to_disk(self) {
        let path = Store::get_path();

        let store = serde_json::to_string(&self).expect("Could not serialize store");

        std::fs::create_dir_all(path.parent().expect("Could not get parent directory"))
            .expect("Could not create store directory");

        std::fs::write(&path, store).expect("Could not write store file");
    }

    pub fn get<'a>(&'a self, name: &'a str) -> Option<EncryptedVariable<'a>> {
        self.vars.get(name).map(|value| EncryptedVariable {
            value,
            _lifetime: std::marker::PhantomData,
        })
    }

    pub fn add<'a>(&mut self, key: String, value: &str) {
        self.vars.insert(key, encrypt(value));
    }

    fn get_path() -> PathBuf {
        let mut path = dirs::data_dir().expect("Could not find data directory");
        path.push("cryptenv");
        path.push("store.json");

        path
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.vars.keys().map(|s| s.as_str())
    }
}

impl<'a> EncryptedVariable<'a> {
    pub fn decrypt(self) -> DecryptedVariable<'a> {
        DecryptedVariable {
            value: decrypt(self.value),
            _lifetime: std::marker::PhantomData,
        }
    }
}

impl<'a> DecryptedVariable<'a> {
    /// get the decrypted value
    pub fn value(&'a self) -> &'a str {
        &self.value
    }
}

fn decrypt(value: &str) -> String {
    let entry = Entry::new("cryptenv", "key").expect("Could not get entry");

    let mut key = match entry.get_secret() {
        // TODO: un needed clone
        Ok(key) => Key::<Aes256Gcm>::clone_from_slice(&key),
        Err(_) => {
            let key = Aes256Gcm::generate_key(&mut OsRng);

            entry
                .set_secret(key.as_ref())
                .expect("Could not set secret");

            key
        }
    };

    let cipher = Aes256Gcm::new(&key);
    use base64::prelude::*;
    let data = BASE64_STANDARD
        .decode(value)
        .expect("value is valid base64");
    let nonce = data[0..12].try_into().expect("nonce is a 96 bit value");

    key.zeroize();

    let decrypted = cipher.decrypt(nonce, &data[12..]).unwrap();

    String::from_utf8(decrypted).expect("decrypted text is valid utf8")
}

fn encrypt(value: &str) -> String {
    let entry = Entry::new("cryptenv", "key").expect("Could not get entry");

    let mut key = match entry.get_secret() {
        // TODO: un needed clone
        Ok(key) => Key::<Aes256Gcm>::clone_from_slice(&key),
        Err(_) => {
            let key = Aes256Gcm::generate_key(&mut OsRng);

            entry
                .set_secret(key.as_ref())
                .expect("Could not set secret");

            key
        }
    };

    let cipher = Aes256Gcm::new(&key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let encrypted = cipher.encrypt(&nonce, value.as_bytes()).unwrap();

    key.zeroize();

    // TODO: lots of copying here
    let data: Vec<u8> = [nonce.as_slice(), &encrypted].concat();
    use base64::prelude::*;
    BASE64_STANDARD.encode(&data)
}
