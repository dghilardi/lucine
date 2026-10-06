use serde::{de::DeserializeOwned, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
};

pub fn read_local<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        _ => return Err("Non riesco a leggere la configurazione locale.".into()),
    };
    let mut raw = Vec::new();
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut raw)
        .map_err(|_| "Non riesco a leggere la configurazione locale.")?;
    if raw.len() > 1024 * 1024 {
        return Err("La configurazione locale è troppo grande.".into());
    }
    serde_json::from_slice(&raw).map_err(|_| "La configurazione locale non è valida.".into())
}

pub fn write_local<T: Serialize>(path: &Path, values: &[T]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("Percorso della configurazione non valido.")?;
    let mut directory = std::fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory
        .create(parent)
        .map_err(|_| "Non riesco a creare la cartella della configurazione.")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| "Non riesco a salvare la configurazione.")?;
    serde_json::to_writer(&mut file, values)
        .map_err(|_| "Non riesco a salvare la configurazione.")?;
    file.flush()
        .map_err(|_| "Non riesco a salvare la configurazione.")?;
    file.as_file()
        .sync_all()
        .map_err(|_| "Non riesco a salvare la configurazione.")?;
    file.persist(path)
        .map_err(|_| "Non riesco a salvare la configurazione.")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_and_malformed_configuration_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.json");
        std::fs::write(&path, vec![b' '; 1024 * 1024 + 1]).unwrap();
        assert!(read_local::<serde_json::Value>(&path)
            .unwrap_err()
            .contains("grande"));
        std::fs::write(&path, "{}").unwrap();
        assert!(read_local::<serde_json::Value>(&path).is_err());
    }
}
