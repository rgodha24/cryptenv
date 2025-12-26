use std::{collections::HashMap, fmt, fs, path::PathBuf, process};

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

#[derive(Debug)]
pub enum DecryptError {
    Keyring(keyring::Error),
    InvalidBase64(base64::DecodeError),
    InvalidDataLength(usize),
    Crypto,
    Utf8(std::string::FromUtf8Error),
}

impl DecryptError {
    fn hint(&self) -> Option<&'static str> {
        match self {
            DecryptError::Keyring(_) => {
                Some("keyring entry is missing or inaccessible; restore it or re-add values")
            }
            DecryptError::InvalidBase64(_) | DecryptError::InvalidDataLength(_) => {
                Some("stored data appears corrupted; re-add the variable")
            }
            DecryptError::Crypto => {
                Some("the encryption key may not match the store; re-add values or restore the key")
            }
            DecryptError::Utf8(_) => Some("stored data is not valid utf8; re-add the variable"),
        }
    }
}

impl fmt::Display for DecryptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecryptError::Keyring(err) => write!(f, "failed to read encryption key: {}", err),
            DecryptError::InvalidBase64(err) => {
                write!(f, "stored value is not valid base64: {}", err)
            }
            DecryptError::InvalidDataLength(len) => {
                write!(f, "stored value is too short ({} bytes)", len)
            }
            DecryptError::Crypto => write!(f, "decryption failed (wrong key or corrupted data)"),
            DecryptError::Utf8(err) => write!(f, "decrypted value is not valid utf8: {}", err),
        }
    }
}

impl std::error::Error for DecryptError {}

/// Get the path to the fallback key file
fn get_key_file_path() -> PathBuf {
    let mut path = dirs::data_dir().expect("Could not find data directory");
    path.push("cryptenv");
    path.push("key");
    path
}

/// Try to get the encryption key, first from keyring, then from file fallback
fn get_key() -> Result<Key<Aes256Gcm>, String> {
    // Try keyring first
    if let Ok(entry) = Entry::new("cryptenv", "key") {
        if let Ok(secret) = entry.get_secret() {
            return Ok(Key::<Aes256Gcm>::clone_from_slice(&secret));
        }
    }

    // Fall back to file-based key storage
    let key_path = get_key_file_path();
    if key_path.exists() {
        let key_bytes =
            fs::read(&key_path).map_err(|e| format!("failed to read key file: {}", e))?;
        if key_bytes.len() == 32 {
            return Ok(Key::<Aes256Gcm>::clone_from_slice(&key_bytes));
        }
    }

    Err("no encryption key found".to_string())
}

/// Store the encryption key, trying keyring first, falling back to file
fn store_key(key: &Key<Aes256Gcm>) -> Result<(), String> {
    // Try keyring first
    let keyring_result =
        Entry::new("cryptenv", "key").and_then(|entry| entry.set_secret(key.as_ref()));

    if keyring_result.is_ok() {
        // Verify it was actually stored by trying to read it back
        let verify = Entry::new("cryptenv", "key").and_then(|entry| entry.get_secret());
        if verify.is_ok() {
            return Ok(());
        }
    }

    // Fall back to file-based storage
    let key_path = get_key_file_path();
    fs::create_dir_all(key_path.parent().unwrap())
        .map_err(|e| format!("failed to create key directory: {}", e))?;

    // Write key with restricted permissions
    fs::write(&key_path, key.as_slice()).map_err(|e| format!("failed to write key file: {}", e))?;

    // Set file permissions to owner-only (Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&key_path, perms)
            .map_err(|e| format!("failed to set key file permissions: {}", e))?;
    }

    Ok(())
}

/// Get or create the encryption key
fn get_or_create_key() -> Key<Aes256Gcm> {
    if let Ok(key) = get_key() {
        return key;
    }

    // Generate new key
    let key = Aes256Gcm::generate_key(&mut OsRng);
    store_key(&key).expect("failed to store encryption key");
    key
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

    pub fn get_decrypted_or_exit<'a>(&'a self, name: &'a str) -> DecryptedVariable<'a> {
        let encrypted = self.get(name).unwrap_or_else(|| {
            eprintln!("cryptenv: variable {} not found", name);
            process::exit(1);
        });

        match encrypted.decrypt() {
            Ok(variable) => variable,
            Err(err) => {
                eprintln!("cryptenv: failed to decrypt {}: {}", name, err);
                if let Some(hint) = err.hint() {
                    eprintln!("cryptenv: hint - {}", hint);
                }
                process::exit(1);
            }
        }
    }

    pub fn add(&mut self, key: String, value: &str) {
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

    pub fn iter(&'_ self) -> impl Iterator<Item = (&'_ str, EncryptedVariable<'_>)> {
        use itertools::Itertools;
        self.vars
            .keys()
            .sorted()
            .map(|k| (k.as_str(), self.get(k).expect("key exists")))
    }
}

impl<'a> EncryptedVariable<'a> {
    pub fn decrypt(self) -> Result<DecryptedVariable<'a>, DecryptError> {
        decrypt(self.value).map(|value| DecryptedVariable {
            value,
            _lifetime: std::marker::PhantomData,
        })
    }
}

impl<'a> DecryptedVariable<'a> {
    /// get the decrypted value
    pub fn value(&'a self) -> &'a str {
        &self.value
    }
}

fn decrypt(value: &str) -> Result<String, DecryptError> {
    let mut key = get_key().map_err(|_| DecryptError::Keyring(keyring::Error::NoEntry))?;

    let cipher = Aes256Gcm::new(&key);
    key.zeroize();

    use base64::prelude::*;
    let data = BASE64_STANDARD
        .decode(value)
        .map_err(DecryptError::InvalidBase64)?;
    if data.len() < 12 {
        return Err(DecryptError::InvalidDataLength(data.len()));
    }
    let nonce = data[0..12].into();

    let decrypted = cipher
        .decrypt(nonce, &data[12..])
        .map_err(|_| DecryptError::Crypto)?;

    String::from_utf8(decrypted).map_err(DecryptError::Utf8)
}

fn encrypt(value: &str) -> String {
    let mut key = get_or_create_key();

    let cipher = Aes256Gcm::new(&key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let encrypted = cipher.encrypt(&nonce, value.as_bytes()).unwrap();

    key.zeroize();

    // TODO: lots of copying here
    let data: Vec<u8> = [nonce.as_slice(), &encrypted].concat();
    use base64::prelude::*;
    BASE64_STANDARD.encode(data)
}
