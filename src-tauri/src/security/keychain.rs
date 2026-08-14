use keyring::Entry;

use crate::{AppError, AppResult};

const KEYRING_SERVICE: &str = "com.yukkuri.agent";

pub fn set(username: &str, password: &str) -> AppResult<()> {
    let entry = Entry::new(KEYRING_SERVICE, username)?;
    entry.set_password(password)?;

    Ok(())
}

pub fn get(username: &str) -> AppResult<Option<String>> {
    let entry = Entry::new(KEYRING_SERVICE, username)?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

pub fn remove(username: &str) -> AppResult<()> {
    let entry = Entry::new(KEYRING_SERVICE, username)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::from(e)),
    }
}
