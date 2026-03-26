use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
pub struct Vault {
    pub path: String,
    pub added_at: String,
    pub last_opened_at: String,
}

#[derive(Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
pub struct Profiles {
    pub current: Option<String>,
    #[serde(default)]
    pub vaults: Vec<Vault>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ProfilesResponse {
    pub home_dir: String,
    pub profiles: Profiles,
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default()
}

fn profiles_path(home_dir: &Path) -> PathBuf {
    home_dir.join(".rhizome").join("profiles.json")
}

pub fn load(home_dir: &Path) -> Profiles {
    let path = profiles_path(home_dir);
    let Ok(content) = fs::read_to_string(path) else {
        return Profiles::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save(home_dir: &Path, profiles: &Profiles) -> Result<(), String> {
    let path = profiles_path(home_dir);
    let dir = path
        .parent()
        .ok_or_else(|| "无法确定 profiles.json 所在目录".to_string())?;
    fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败: {e}"))?;

    let content = serde_json::to_string_pretty(profiles).map_err(|e| format!("序列化失败: {e}"))?;
    let tmp = dir.join("profiles.json.tmp");
    fs::write(&tmp, content).map_err(|e| format!("写入临时文件失败: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("替换配置文件失败: {e}"))?;
    Ok(())
}

/// 按 path 去重 upsert vault，设为 current，更新 last_opened_at，按 last_opened_at 降序
pub fn upsert_vault(profiles: &mut Profiles, path: &str) {
    let now = now_rfc3339();
    match profiles.vaults.iter_mut().find(|v| v.path == path) {
        Some(vault) => vault.last_opened_at = now,
        None => profiles.vaults.push(Vault {
            path: path.to_string(),
            added_at: now.clone(),
            last_opened_at: now,
        }),
    }
    profiles.current = Some(path.to_string());
    profiles.vaults.sort_by(|a, b| b.last_opened_at.cmp(&a.last_opened_at));
}

pub fn create_vault(home_dir: &Path, path: &str) -> Result<Profiles, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("路径不能为空".to_string());
    }
    let target = Path::new(trimmed);
    if target.exists() && !target.is_dir() {
        return Err(format!("路径已存在且不是目录: {trimmed}"));
    }
    if !target.exists() {
        fs::create_dir_all(target).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    crate::db::ensure_db(target)?;
    let mut profiles = load(home_dir);
    upsert_vault(&mut profiles, trimmed);
    save(home_dir, &profiles)?;
    Ok(profiles)
}

pub fn open_vault(home_dir: &Path, path: &str) -> Result<Profiles, String> {
    let trimmed = path.trim();
    let target = Path::new(trimmed);
    if !target.exists() {
        return Err(format!("目录不存在: {trimmed}"));
    }
    if !target.is_dir() {
        return Err(format!("路径不是目录: {trimmed}"));
    }
    crate::db::ensure_db(target)?;
    let mut profiles = load(home_dir);
    upsert_vault(&mut profiles, trimmed);
    save(home_dir, &profiles)?;
    Ok(profiles)
}

pub fn get_profiles(home_dir: &Path) -> Result<ProfilesResponse, String> {
    let home = home_dir.to_string_lossy().into_owned();
    Ok(ProfilesResponse {
        home_dir: home,
        profiles: load(home_dir),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        TempDir::new().expect("创建临时目录失败")
    }

    fn vault_at<'a>(profiles: &'a Profiles, path: &str) -> Option<&'a Vault> {
        profiles.vaults.iter().find(|v| v.path == path)
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        let tmp = setup();
        assert_eq!(load(tmp.path()), Profiles::default());
    }

    #[test]
    fn test_load_invalid_json_returns_default() {
        let tmp = setup();
        let dir = tmp.path().join(".rhizome");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("profiles.json"), "not json").unwrap();
        assert_eq!(load(tmp.path()), Profiles::default());
    }

    #[test]
    fn test_save_creates_directory_and_load_roundtrip() {
        let tmp = setup();
        let mut profiles = Profiles::default();
        upsert_vault(&mut profiles, "C:\\vaults\\notes");
        save(tmp.path(), &profiles).expect("保存失败");
        assert_eq!(load(tmp.path()), profiles);
        assert!(tmp.path().join(".rhizome").join("profiles.json").exists());
    }

    #[test]
    fn test_upsert_sets_current_and_updates_timestamp() {
        let mut profiles = Profiles::default();
        upsert_vault(&mut profiles, "C:\\a");
        let first = vault_at(&profiles, "C:\\a").unwrap().last_opened_at.clone();
        upsert_vault(&mut profiles, "C:\\b");
        upsert_vault(&mut profiles, "C:\\a");

        assert_eq!(profiles.current.as_deref(), Some("C:\\a"));
        assert_eq!(profiles.vaults.len(), 2);
        assert_eq!(profiles.vaults[0].path, "C:\\a");
        assert!(vault_at(&profiles, "C:\\a").unwrap().last_opened_at >= first);
        assert_eq!(vault_at(&profiles, "C:\\a").unwrap().added_at, first);
    }

    #[test]
    fn test_create_vault_rejects_empty_path() {
        let tmp = setup();
        let err = create_vault(tmp.path(), "   ").unwrap_err();
        assert_eq!(err, "路径不能为空");
    }

    #[test]
    fn test_create_vault_rejects_existing_file() {
        let tmp = setup();
        let file = tmp.path().join("afile");
        fs::write(&file, "x").unwrap();
        let err = create_vault(tmp.path(), file.to_str().unwrap()).unwrap_err();
        assert!(err.contains("不是目录"));
    }

    #[test]
    fn test_create_vault_creates_directory_and_persists() {
        let tmp = setup();
        let vault = tmp.path().join("new").join("vault");
        let path = vault.to_str().unwrap();
        let profiles = create_vault(tmp.path(), path).expect("创建失败");

        assert!(vault.is_dir());
        assert_eq!(profiles.current.as_deref(), Some(path));
        assert_eq!(load(tmp.path()), profiles);

        // 目录内仅创建 rhizome.db，不写入其他内容
        assert!(vault.join("rhizome.db").is_file());
        assert_eq!(fs::read_dir(&vault).unwrap().count(), 1);
    }

    #[test]
    fn test_create_vault_existing_directory_is_idempotent() {
        let tmp = setup();
        let vault = tmp.path().join("existing");
        fs::create_dir_all(&vault).unwrap();
        let path = vault.to_str().unwrap();
        let first = create_vault(tmp.path(), path).unwrap();
        let second = create_vault(tmp.path(), path).unwrap();

        assert_eq!(first.vaults.len(), 1);
        assert_eq!(second.vaults.len(), 1);
        assert_eq!(second.current.as_deref(), Some(path));
    }

    #[test]
    fn test_open_vault_rejects_missing_directory() {
        let tmp = setup();
        let err = open_vault(tmp.path(), tmp.path().join("nope").to_str().unwrap()).unwrap_err();
        assert!(err.contains("不存在"));
    }

    #[test]
    fn test_open_vault_rejects_file_path() {
        let tmp = setup();
        let file = tmp.path().join("afile");
        fs::write(&file, "x").unwrap();
        let err = open_vault(tmp.path(), file.to_str().unwrap()).unwrap_err();
        assert!(err.contains("不是目录"));
    }

    #[test]
    fn test_open_vault_records_and_sorts() {
        let tmp = setup();
        let vault_a = tmp.path().join("vault-a");
        let vault_b = tmp.path().join("vault-b");
        fs::create_dir_all(&vault_a).unwrap();
        fs::create_dir_all(&vault_b).unwrap();

        create_vault(tmp.path(), vault_a.to_str().unwrap()).unwrap();
        let profiles = open_vault(tmp.path(), vault_b.to_str().unwrap()).unwrap();

        assert_eq!(profiles.vaults.len(), 2);
        assert_eq!(profiles.current.as_deref(), Some(vault_b.to_str().unwrap()));
        assert_eq!(profiles.vaults[0].path, vault_b.to_str().unwrap());
        assert_eq!(load(tmp.path()), profiles);
    }

    #[test]
    fn test_open_vault_creates_missing_db_idempotently() {
        let tmp = setup();
        let vault = tmp.path().join("legacy");
        fs::create_dir_all(&vault).unwrap();
        assert!(!vault.join("rhizome.db").exists());

        open_vault(tmp.path(), vault.to_str().unwrap()).unwrap();
        assert!(vault.join("rhizome.db").is_file());

        open_vault(tmp.path(), vault.to_str().unwrap()).unwrap();
        assert_eq!(fs::read_dir(&vault).unwrap().count(), 1);
    }

    #[test]
    fn test_get_profiles_includes_home_dir() {
        let tmp = setup();
        let resp = get_profiles(tmp.path()).unwrap();
        assert_eq!(resp.home_dir, tmp.path().to_string_lossy());
        assert_eq!(resp.profiles, Profiles::default());
    }

    #[test]
    fn test_now_rfc3339_parseable() {
        let now = now_rfc3339();
        assert!(OffsetDateTime::parse(&now, &Rfc3339).is_ok());
    }
}
