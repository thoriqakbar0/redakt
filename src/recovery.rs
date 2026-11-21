use redakt::{redact_with, Span};
use ring::{
    aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305, NONCE_LEN},
    rand::{SecureRandom, SystemRandom},
};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const FORMAT: &[u8] = b"redakt-recovery-v1\0";
const PREFIX: &str = "[REDAKT_";
const TOKEN_BYTES: usize = 16;
const TOKEN_LEN: usize = PREFIX.len() + TOKEN_BYTES * 2 + 1;
const MAP_SUFFIX: &str = ".recovery";

type Mapping = BTreeMap<String, String>;
type Result<T> = std::result::Result<T, String>;

pub struct Recovery {
    path: PathBuf,
    code: PathBuf,
}

impl Recovery {
    pub fn select(path: Option<PathBuf>, code: Option<PathBuf>, reverse: bool) -> Result<Self> {
        let path = path.map_or_else(|| default_mapping(reverse), Ok)?;
        let code = code.unwrap_or_else(|| path.with_added_extension("code"));
        if path == code {
            return Err("Recovery file and code file must be separate".into());
        }
        Ok(Self { path, code })
    }

    pub fn mask(&self, text: &str, mut spans: Vec<Span>) -> Result<String> {
        let _lock = self.lock()?;
        let existing = read_optional(&self.path)?;
        let create = existing.is_none();
        let key = self.key(create)?;
        let mut mapping = match existing {
            Some(bytes) => decrypt(&key, bytes)?,
            None => Mapping::new(),
        };
        let stored_count = mapping.len();
        add_known_spans(text, &mapping, &mut spans);

        let mut by_value: HashMap<_, _> = mapping
            .iter()
            .map(|(token, value)| (value.clone(), token.clone()))
            .collect();
        let masked = redact_with(text, &spans, |output, _, value| {
            if let Some(token) = by_value.get(value) {
                output.push_str(token);
                return Ok(());
            }
            let token = loop {
                let token = format!("{PREFIX}{}]", random_hex::<TOKEN_BYTES>()?);
                if !mapping.contains_key(&token) && !text.contains(&token) {
                    break token;
                }
            };
            output.push_str(&token);
            by_value.insert(value.to_owned(), token.clone());
            mapping.insert(token, value.to_owned());
            Ok(())
        })?;

        if create || mapping.len() != stored_count {
            save(&self.path, &encrypt(&key, &mapping)?, create)?;
        }
        Ok(masked)
    }

    pub fn reverse(&self, text: &str) -> Result<String> {
        let key = self.key(false)?;
        let bytes = fs::read(&self.path).map_err(|_| "Could not read recovery file")?;
        let mapping = decrypt(&key, bytes)?;
        let mut output = String::with_capacity(text.len());
        let mut cursor = 0;
        for (offset, token) in tokens(text) {
            if let Some(value) = mapping.get(token) {
                output.push_str(&text[cursor..offset]);
                output.push_str(value);
                cursor = offset + TOKEN_LEN;
            }
        }
        output.push_str(&text[cursor..]);
        Ok(output)
    }

    fn lock(&self) -> Result<File> {
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.path.with_added_extension("lock"))
            .map_err(|_| "Could not open recovery lock")?;
        lock.lock().map_err(|_| "Could not lock recovery file")?;
        Ok(lock)
    }

    fn key(&self, create: bool) -> Result<LessSafeKey> {
        let code = match read_optional(&self.code)? {
            Some(_) if create => return Err("New recovery mappings require a new code file".into()),
            Some(code) => code,
            None if create => {
                let code = random_hex::<32>()?;
                save(&self.code, code.as_bytes(), true)?;
                code.into_bytes()
            }
            None => return Err("Recovery code file is missing".into()),
        };
        let code = std::str::from_utf8(&code).map_err(|_| "Invalid recovery code")?;
        let code = code.trim();
        if code.len() != 64 || !code.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("Invalid recovery code".into());
        }
        let mut key = [0u8; 32];
        for (index, byte) in key.iter_mut().enumerate() {
            let start = index * 2;
            *byte = u8::from_str_radix(&code[start..start + 2], 16)
                .map_err(|_| "Invalid recovery code")?;
        }
        UnboundKey::new(&CHACHA20_POLY1305, &key)
            .map(LessSafeKey::new)
            .map_err(|_| "Could not initialize recovery encryption".into())
    }
}

fn add_known_spans(text: &str, mapping: &Mapping, spans: &mut Vec<Span>) {
    if mapping.is_empty() {
        return;
    }
    let offsets: Vec<_> = text
        .char_indices()
        .map(|(offset, _)| offset)
        .chain([text.len()])
        .collect();

    for value in mapping.keys().chain(mapping.values()) {
        for (offset, matched) in text.match_indices(value) {
            let start = offsets
                .binary_search(&offset)
                .expect("match starts at a character");
            spans.push(Span {
                start,
                end: start + matched.chars().count(),
                label: String::new(),
            });
        }
    }
}

fn default_mapping(reverse: bool) -> Result<PathBuf> {
    let directory = state_directory()?;
    if reverse {
        return find_mapping(&directory);
    }
    fs::create_dir_all(&directory).map_err(|_| "Could not create recovery directory")?;
    let identifier = random_hex::<TOKEN_BYTES>()?;
    Ok(directory.join(format!("{identifier}{MAP_SUFFIX}")))
}

fn tokens(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.match_indices(PREFIX).filter_map(|(offset, _)| {
        let token = text.get(offset..offset + TOKEN_LEN)?;
        let identifier = token.strip_prefix(PREFIX)?.strip_suffix(']')?;
        identifier
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
            .then_some((offset, token))
    })
}

fn state_directory() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("REDAKT_STATE_DIR") {
        return Ok(path.into());
    }
    if let Some(path) =
        std::env::var_os("XDG_STATE_HOME").or_else(|| std::env::var_os("LOCALAPPDATA"))
    {
        return Ok(PathBuf::from(path).join("redakt"));
    }
    let home = std::env::var_os("HOME").ok_or("Set REDAKT_STATE_DIR for recovery files")?;
    Ok(PathBuf::from(home).join(".local/state/redakt"))
}

fn find_mapping(directory: &Path) -> Result<PathBuf> {
    let entries = fs::read_dir(directory).map_err(|_| "No recovery file found")?;
    let mut selected = None;
    for entry in entries {
        let entry = entry.map_err(|_| "Could not list recovery files")?;
        if entry.file_name().to_string_lossy().ends_with(MAP_SUFFIX) {
            if selected.is_some() {
                return Err("Multiple recovery files found; select one with --recovery".into());
            }
            selected = Some(entry.path());
        }
    }
    selected.ok_or_else(|| "No recovery file found".into())
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("Could not read recovery data".into()),
    }
}

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "Could not generate secure random bytes")?;
    Ok(bytes)
}

fn random_hex<const N: usize>() -> Result<String> {
    let mut output = String::with_capacity(N * 2);
    for byte in random::<N>()? {
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(output)
}

fn encrypt(key: &LessSafeKey, mapping: &Mapping) -> Result<Vec<u8>> {
    let nonce = random::<NONCE_LEN>()?;
    let mut payload = serde_json::to_vec(mapping).map_err(|_| "Could not encode recovery data")?;
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(FORMAT),
        &mut payload,
    )
    .map_err(|_| "Could not encrypt recovery data")?;
    let mut bytes = FORMAT.to_vec();
    bytes.extend(nonce);
    bytes.extend(payload);
    Ok(bytes)
}

fn decrypt(key: &LessSafeKey, mut bytes: Vec<u8>) -> Result<Mapping> {
    let payload = bytes.strip_prefix(FORMAT).ok_or("Invalid recovery file")?;
    let nonce = payload.get(..NONCE_LEN).ok_or("Invalid recovery file")?;
    let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| "Invalid recovery file")?;
    let payload = &mut bytes[FORMAT.len() + NONCE_LEN..];
    let cleartext = key
        .open_in_place(nonce, Aad::from(FORMAT), payload)
        .map_err(|_| "Could not authenticate recovery data")?;
    serde_json::from_slice(cleartext).map_err(|_| "Invalid recovery data".into())
}

fn save(path: &Path, bytes: &[u8], create_only: bool) -> Result<()> {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let mut file = NamedTempFile::new_in(directory.unwrap_or(Path::new(".")))
        .map_err(|_| "Could not create recovery file")?;
    file.write_all(bytes)
        .and_then(|()| file.as_file().sync_all())
        .map_err(|_| "Could not write recovery file")?;

    let saved = if create_only {
        file.persist_noclobber(path)
    } else {
        file.persist(path)
    };
    saved.map_err(|_| "Could not save recovery file")?;
    #[cfg(unix)]
    File::open(directory.unwrap_or(Path::new(".")))
        .and_then(|directory| directory.sync_all())
        .map_err(|_| "Could not synchronize recovery directory")?;
    Ok(())
}
