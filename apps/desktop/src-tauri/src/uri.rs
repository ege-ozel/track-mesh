//! Per-user `trackmesh://` association used by the native invite handler.
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use std::ptr;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
    REG_OPTION_NON_VOLATILE, REG_SZ,
};
use zeroize::Zeroizing;

pub fn register_protocol() -> Result<()> {
    let exe = std::env::current_exe().context("could not locate TrackMesh executable")?;
    let command = format!("\"{}\" \"%1\"", exe.display());
    set_default(r"Software\Classes\trackmesh", "URL:TrackMesh Protocol")?;
    set_value(r"Software\Classes\trackmesh", "URL Protocol", "")?;
    set_default(r"Software\Classes\trackmesh\shell\open\command", &command)
}

pub fn is_invite_link(value: &str) -> bool {
    value.starts_with("trackmesh://invite/")
}

pub fn parse_invite_link(value: &str) -> Result<(String, Zeroizing<[u8; 32]>)> {
    ensure!(value.len() <= 256, "invalid invitation link");
    let body = value
        .strip_prefix("trackmesh://invite/")
        .context("invalid invitation link")?;
    let (id, key) = body.split_once('#').context("invalid invitation link")?;
    ensure!(
        id.len() == 43
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            && key.len() == 43,
        "invalid invitation link"
    );
    let decoded_id = URL_SAFE_NO_PAD
        .decode(id)
        .context("invalid invitation link")?;
    ensure!(
        decoded_id.len() == 32 && URL_SAFE_NO_PAD.encode(decoded_id) == id,
        "invalid invitation link"
    );
    let decoded = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(key)
            .context("invalid invitation link")?,
    );
    ensure!(decoded.len() == 32, "invalid invitation link");
    let canonical_key = Zeroizing::new(URL_SAFE_NO_PAD.encode(&*decoded));
    ensure!(canonical_key.as_str() == key, "invalid invitation link");
    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    let key = Zeroizing::new(key);
    Ok((id.to_owned(), key))
}

fn set_default(path: &str, value: &str) -> Result<()> {
    set_registry_value(path, None, value)
}

fn set_value(path: &str, name: &str, value: &str) -> Result<()> {
    set_registry_value(path, Some(name), value)
}

fn set_registry_value(path: &str, name: Option<&str>, value: &str) -> Result<()> {
    let path = path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let name = name.map(|value| {
        value
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>()
    });
    let mut key: HKEY = ptr::null_mut();
    let result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            ptr::null(),
            &mut key,
            ptr::null_mut(),
        )
    };
    ensure!(result == 0, "could not register TrackMesh link handler");
    let mut data = value
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let data_bytes = u32::try_from(data.len() * 2)?;
    let result = unsafe {
        RegSetValueExW(
            key,
            name.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
            0,
            REG_SZ,
            data.as_mut_ptr().cast::<u8>() as *const u8,
            data_bytes,
        )
    };
    unsafe {
        RegCloseKey(key);
    }
    ensure!(result == 0, "could not register TrackMesh link handler");
    Ok(())
}
