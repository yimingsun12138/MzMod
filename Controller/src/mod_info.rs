//! The native package identity used to create and recognize an MzMod installation.
use serde::Deserialize;

pub(crate) const MOD_NAME: &str = "MzMod";
pub(crate) const MOD_INFO: &str = "modinfo.json";

#[derive(Deserialize)]
pub(crate) struct ModIdentity {
    pub(crate) name: String,
}

pub(crate) fn read<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> serde_json::Result<T> {
    serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_the_native_name_in_older_manifests() {
        for bytes in [
            &br#"{"name":"MzMod"}"#[..],
            b"\xef\xbb\xbf{\"name\":\"MzMod\",\"clientMetadata\":42}",
        ] {
            assert_eq!(read::<ModIdentity>(bytes).unwrap().name, MOD_NAME);
        }
        assert!(read::<ModIdentity>(br#"{"author":"MzMod"}"#).is_err());
    }
}
