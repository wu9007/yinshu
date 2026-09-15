use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io, path::Path};
use thiserror::Error;
use yinshu_core::{config::AgentConfig, ip_whitelist::validate_allowed_ip_entry};
use zeroize::Zeroize;

const ENCRYPTED_FORMAT: &str = "yinshu-config-encrypted";
const PAYLOAD_FORMAT: &str = "yinshu-config";
const TRANSFER_VERSION: u16 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const ARGON2_MEMORY_KIB: u32 = 19_456;
const ARGON2_ITERATIONS: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;
const GCM_TAG_BYTES: u8 = 16;

/// 导出配置时各配置项是否包含在文件中的选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportConfigOptions {
    pub service_port: bool,
    pub allowed_origins: bool,
    pub allowed_ips: bool,
}

impl ExportConfigOptions {
    /// 返回包含所有可导出配置项的默认选项。
    pub fn all() -> Self {
        Self {
            service_port: true,
            allowed_origins: true,
            allowed_ips: true,
        }
    }
}

/// 加密配置文件在磁盘上的外层结构。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedConfigFile {
    pub format: String,
    pub version: u16,
    pub crypto: CryptoMetadata,
    pub payload: String,
}

/// 加密配置文件使用的 KDF 和加密算法参数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CryptoMetadata {
    pub kdf: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub cipher: String,
    pub tag_bytes: u8,
    pub salt: String,
    pub nonce: String,
}

/// 加密前的 YinShu 配置迁移载荷。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigTransferPayload {
    pub format: String,
    pub version: u16,
    pub config: PartialTransferConfig,
}

/// 配置迁移载荷中按模块拆分的可选配置。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PartialTransferConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<PartialServiceTransferConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<PartialSecurityTransferConfig>,
}

/// 配置迁移载荷中的本地服务设置。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PartialServiceTransferConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

/// 配置迁移载荷中的安全白名单设置。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PartialSecurityTransferConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_origins: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_ips: Option<Vec<String>>,
}

/// 导入配置前展示给前端的差异预览。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreview {
    pub file_hash: String,
    pub items: Vec<ImportPreviewItem>,
}

/// 导入预览中的单个配置项变化。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewItem {
    pub key: String,
    pub label: String,
    pub current: String,
    pub next: String,
}

/// 配置导入导出过程中的错误。
#[derive(Debug, Error)]
pub enum ConfigTransferError {
    #[error("不是有效的 YinShu 配置文件")]
    InvalidFile,
    #[error("密码错误或文件损坏")]
    InvalidPasswordOrPayload,
    #[error("配置导出失败")]
    Serialize,
    #[error("{0}")]
    InvalidField(String),
}

impl From<io::Error> for ConfigTransferError {
    fn from(error: io::Error) -> Self {
        ConfigTransferError::InvalidField(error.to_string())
    }
}

/// 把加密配置文件写入指定路径。
pub fn write_encrypted_file(
    path: &Path,
    file: &EncryptedConfigFile,
) -> Result<(), ConfigTransferError> {
    let content = serde_json::to_string_pretty(file).map_err(|_| ConfigTransferError::Serialize)?;
    fs::write(path, content)?;
    Ok(())
}

/// 从指定路径读取加密配置文件。
pub fn read_encrypted_file(path: &Path) -> Result<EncryptedConfigFile, ConfigTransferError> {
    let (file, _) = read_encrypted_file_with_hash(path)?;
    Ok(file)
}

/// 读取加密配置文件，并返回文件内容哈希用于导入确认。
pub fn read_encrypted_file_with_hash(
    path: &Path,
) -> Result<(EncryptedConfigFile, String), ConfigTransferError> {
    let content = fs::read_to_string(path).map_err(|_| ConfigTransferError::InvalidFile)?;
    let hash = sha256_hex(content.as_bytes());
    let file = serde_json::from_str::<EncryptedConfigFile>(&content)
        .map_err(|_| ConfigTransferError::InvalidFile)?;
    Ok((file, hash))
}

fn sha256_hex(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to String should not fail");
    }
    out
}

/// 根据导出选项从完整配置构建迁移载荷。
pub fn build_transfer_payload(
    config: &AgentConfig,
    options: &ExportConfigOptions,
) -> ConfigTransferPayload {
    let service = options
        .service_port
        .then_some(PartialServiceTransferConfig {
            port: Some(config.service.port),
        });

    let security =
        (options.allowed_origins || options.allowed_ips).then(|| PartialSecurityTransferConfig {
            allowed_origins: options
                .allowed_origins
                .then(|| config.security.allowed_origins.clone()),
            allowed_ips: options
                .allowed_ips
                .then(|| config.security.allowed_ips.clone()),
        });

    ConfigTransferPayload {
        format: PAYLOAD_FORMAT.to_string(),
        version: TRANSFER_VERSION,
        config: PartialTransferConfig { service, security },
    }
}

/// 使用密码把配置迁移载荷加密为可保存文件。
pub fn encrypt_payload(
    payload: &ConfigTransferPayload,
    password: &str,
) -> Result<EncryptedConfigFile, ConfigTransferError> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);

    let mut key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| ConfigTransferError::Serialize)?;
    let plaintext = serde_json::to_vec(payload).map_err(|_| ConfigTransferError::Serialize)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
        .map_err(|_| ConfigTransferError::Serialize)?;
    key.zeroize();

    Ok(EncryptedConfigFile {
        format: ENCRYPTED_FORMAT.to_string(),
        version: TRANSFER_VERSION,
        crypto: CryptoMetadata {
            kdf: "argon2id13".to_string(),
            memory_kib: ARGON2_MEMORY_KIB,
            iterations: ARGON2_ITERATIONS,
            parallelism: ARGON2_PARALLELISM,
            cipher: "aes-256-gcm".to_string(),
            tag_bytes: GCM_TAG_BYTES,
            salt: BASE64.encode(salt),
            nonce: BASE64.encode(nonce),
        },
        payload: BASE64.encode(ciphertext),
    })
}

/// 使用密码解密配置文件并校验载荷格式。
pub fn decrypt_payload(
    file: &EncryptedConfigFile,
    password: &str,
) -> Result<ConfigTransferPayload, ConfigTransferError> {
    validate_envelope(file)?;
    let salt = decode_fixed::<SALT_LEN>(&file.crypto.salt)?;
    let nonce = decode_fixed::<NONCE_LEN>(&file.crypto.nonce)?;
    let ciphertext = BASE64
        .decode(&file.payload)
        .map_err(|_| ConfigTransferError::InvalidPasswordOrPayload)?;

    let mut key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| ConfigTransferError::InvalidPasswordOrPayload)?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| ConfigTransferError::InvalidPasswordOrPayload)?;
    key.zeroize();

    let payload = serde_json::from_slice::<ConfigTransferPayload>(&plaintext)
        .map_err(|_| ConfigTransferError::InvalidPasswordOrPayload)?;
    validate_payload(&payload)?;
    Ok(payload)
}

/// 把导入载荷合并到当前配置，并校验字段合法性。
pub fn merge_payload(
    current: &AgentConfig,
    payload: &ConfigTransferPayload,
) -> Result<AgentConfig, ConfigTransferError> {
    validate_payload(payload)?;
    let mut next = current.clone();

    if let Some(service) = &payload.config.service {
        if let Some(port) = service.port {
            if port == 0 {
                return Err(ConfigTransferError::InvalidField(
                    "本地端口必须大于 0".to_string(),
                ));
            }
            next.service.port = port;
        }
    }

    if let Some(security) = &payload.config.security {
        if let Some(allowed_origins) = &security.allowed_origins {
            for origin in allowed_origins {
                yinshu_core::protocol::validate_origin(origin).map_err(|_| {
                    ConfigTransferError::InvalidField(format!("Origin 无效: {origin}"))
                })?;
            }
            next.security.allowed_origins = allowed_origins.clone();
        }
        if let Some(allowed_ips) = &security.allowed_ips {
            for entry in allowed_ips {
                validate_allowed_ip_entry(entry).map_err(ConfigTransferError::InvalidField)?;
            }
            next.security.allowed_ips = allowed_ips.clone();
        }
    }

    Ok(next.normalized())
}

/// 生成导入载荷对当前配置的变更预览。
pub fn preview_payload(
    current: &AgentConfig,
    payload: &ConfigTransferPayload,
) -> Result<ImportPreview, ConfigTransferError> {
    let next = merge_payload(current, payload)?;
    let mut items = Vec::new();

    if payload
        .config
        .service
        .as_ref()
        .is_some_and(|service| service.port.is_some())
    {
        items.push(preview_item(
            "service.port",
            "本地端口",
            current.service.port.to_string(),
            next.service.port.to_string(),
        ));
    }

    if payload
        .config
        .security
        .as_ref()
        .is_some_and(|security| security.allowed_origins.is_some())
    {
        items.push(preview_item(
            "security.allowed_origins",
            "网站白名单",
            format!("{} 项", current.security.allowed_origins.len()),
            format!("{} 项", next.security.allowed_origins.len()),
        ));
    }

    if payload
        .config
        .security
        .as_ref()
        .is_some_and(|security| security.allowed_ips.is_some())
    {
        items.push(preview_item(
            "security.allowed_ips",
            "IP 白名单",
            format!("{} 项", current.security.allowed_ips.len()),
            format!("{} 项", next.security.allowed_ips.len()),
        ));
    }

    Ok(ImportPreview {
        file_hash: String::new(),
        items,
    })
}

fn preview_item(
    key: &str,
    label: &str,
    current: impl Into<String>,
    next: impl Into<String>,
) -> ImportPreviewItem {
    ImportPreviewItem {
        key: key.to_string(),
        label: label.to_string(),
        current: current.into(),
        next: next.into(),
    }
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], ConfigTransferError> {
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        None,
    )
    .map_err(|_| ConfigTransferError::Serialize)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| ConfigTransferError::InvalidPasswordOrPayload)?;
    Ok(key)
}

fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N], ConfigTransferError> {
    let bytes = BASE64
        .decode(value)
        .map_err(|_| ConfigTransferError::InvalidPasswordOrPayload)?;
    bytes
        .try_into()
        .map_err(|_| ConfigTransferError::InvalidPasswordOrPayload)
}

fn validate_envelope(file: &EncryptedConfigFile) -> Result<(), ConfigTransferError> {
    if file.format != ENCRYPTED_FORMAT
        || file.version != TRANSFER_VERSION
        || file.crypto.kdf != "argon2id13"
        || file.crypto.memory_kib != ARGON2_MEMORY_KIB
        || file.crypto.iterations != ARGON2_ITERATIONS
        || file.crypto.parallelism != ARGON2_PARALLELISM
        || file.crypto.cipher != "aes-256-gcm"
        || file.crypto.tag_bytes != GCM_TAG_BYTES
    {
        return Err(ConfigTransferError::InvalidFile);
    }
    Ok(())
}

fn validate_payload(payload: &ConfigTransferPayload) -> Result<(), ConfigTransferError> {
    if payload.format != PAYLOAD_FORMAT || payload.version != TRANSFER_VERSION {
        return Err(ConfigTransferError::InvalidFile);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yinshu_core::config::{AgentConfig, UiLanguage};

    fn sample_config() -> AgentConfig {
        let mut config = AgentConfig::default();
        config.service.port = 19090;
        config.security.allowed_origins = vec!["https://example.com".to_string()];
        config.security.allowed_ips = vec!["127.0.0.1".to_string(), "192.168.1.0/24".to_string()];
        config
    }

    #[test]
    fn build_payload_only_includes_selected_fields() {
        let config = sample_config();
        let options = ExportConfigOptions {
            service_port: true,
            allowed_origins: false,
            allowed_ips: true,
        };

        let payload = build_transfer_payload(&config, &options);

        assert_eq!(payload.format, "yinshu-config");
        assert_eq!(payload.version, 1);
        assert_eq!(payload.config.service.unwrap().port, Some(19090));
        let security = payload.config.security.unwrap();
        assert_eq!(security.allowed_origins, None);
        assert_eq!(
            security.allowed_ips,
            Some(vec!["127.0.0.1".to_string(), "192.168.1.0/24".to_string()])
        );
    }

    #[test]
    fn build_payload_does_not_export_ui_language() {
        let mut config = sample_config();
        config.app.language = UiLanguage::En;

        let payload = build_transfer_payload(&config, &ExportConfigOptions::all());
        let json = serde_json::to_string(&payload).unwrap();

        assert!(!json.contains("language"));
        assert!(!json.contains("app"));
    }

    #[test]
    fn encrypted_payload_roundtrips_with_non_empty_password() {
        let payload = build_transfer_payload(&sample_config(), &ExportConfigOptions::all());

        let encrypted = encrypt_payload(&payload, "passw0rd").unwrap();
        let decrypted = decrypt_payload(&encrypted, "passw0rd").unwrap();

        assert_eq!(decrypted, payload);
        assert_eq!(encrypted.format, "yinshu-config-encrypted");
        assert_eq!(encrypted.version, 1);
        assert_eq!(encrypted.crypto.kdf, "argon2id13");
        assert_eq!(encrypted.crypto.memory_kib, 19_456);
        assert_eq!(encrypted.crypto.iterations, 2);
        assert_eq!(encrypted.crypto.parallelism, 1);
        assert_eq!(encrypted.crypto.cipher, "aes-256-gcm");
        assert_eq!(encrypted.crypto.tag_bytes, 16);
    }

    #[test]
    fn encrypted_payload_roundtrips_with_empty_password() {
        let payload = build_transfer_payload(&sample_config(), &ExportConfigOptions::all());

        let encrypted = encrypt_payload(&payload, "").unwrap();
        let decrypted = decrypt_payload(&encrypted, "").unwrap();

        assert_eq!(decrypted, payload);
    }

    #[test]
    fn encrypted_config_file_writes_and_reads_json() {
        let path = std::env::temp_dir().join(format!(
            "yinshu-config-transfer-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let payload = build_transfer_payload(&sample_config(), &ExportConfigOptions::all());
        let encrypted = encrypt_payload(&payload, "").unwrap();
        write_encrypted_file(&path, &encrypted).unwrap();
        let read = read_encrypted_file(&path).unwrap();

        assert_eq!(read, encrypted);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn encrypted_config_file_hash_changes_with_content() {
        let path = std::env::temp_dir().join(format!(
            "yinshu-config-transfer-hash-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        let payload = build_transfer_payload(&sample_config(), &ExportConfigOptions::all());
        let first = encrypt_payload(&payload, "first").unwrap();
        write_encrypted_file(&path, &first).unwrap();
        let (_, first_hash) = read_encrypted_file_with_hash(&path).unwrap();

        let second = encrypt_payload(&payload, "second").unwrap();
        write_encrypted_file(&path, &second).unwrap();
        let (_, second_hash) = read_encrypted_file_with_hash(&path).unwrap();

        assert_ne!(first_hash, second_hash);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn decrypt_rejects_wrong_password() {
        let payload = build_transfer_payload(&sample_config(), &ExportConfigOptions::all());
        let encrypted = encrypt_payload(&payload, "right").unwrap();

        let error = decrypt_payload(&encrypted, "wrong").unwrap_err();

        assert!(matches!(
            error,
            ConfigTransferError::InvalidPasswordOrPayload
        ));
    }

    #[test]
    fn decrypts_php_compatible_test_vector() {
        let encrypted = EncryptedConfigFile {
            format: "yinshu-config-encrypted".to_string(),
            version: 1,
            crypto: CryptoMetadata {
                kdf: "argon2id13".to_string(),
                memory_kib: 19_456,
                iterations: 2,
                parallelism: 1,
                cipher: "aes-256-gcm".to_string(),
                tag_bytes: 16,
                salt: "AAECAwQFBgcICQoLDA0ODw==".to_string(),
                nonce: "EBESExQVFhcYGRob".to_string(),
            },
            payload: "OTX3vkYug76bv335qmWdp85pbgu85QfwarlnqhxGoV0U+4sRez0dlwWy+5eIe597KLRqdHg7XJVbjLds/mXROcLHLhTJrJJ+DWpB2Xc6BX2sKii+bziOsb8akhUwxqo=".to_string(),
        };

        let payload = decrypt_payload(&encrypted, "test-password").unwrap();

        assert_eq!(payload.format, "yinshu-config");
        assert_eq!(payload.version, 1);
        assert_eq!(payload.config.service.unwrap().port, Some(17890));
    }

    #[test]
    fn merge_payload_ignores_legacy_remote_fields() {
        let current = sample_config();
        let payload = serde_json::from_str::<ConfigTransferPayload>(
            r#"{"format":"yinshu-config","version":1,"config":{"remote":{"enabled":true,"endpoint_url":"https://old.example.com/tasks"}}}"#,
        )
        .unwrap();

        let merged = merge_payload(&current, &payload).unwrap();

        assert_eq!(merged.service.port, current.service.port);
        assert_eq!(
            merged.security.allowed_origins,
            current.security.allowed_origins
        );
    }

    #[test]
    fn merge_payload_replaces_allowed_origins_as_list() {
        let mut current = sample_config();
        current.security.allowed_origins = vec!["https://old.example.com".to_string()];
        let mut payload = build_transfer_payload(&current, &ExportConfigOptions::all());
        payload.config.security.as_mut().unwrap().allowed_origins =
            Some(vec!["https://new.example.com".to_string()]);

        let merged = merge_payload(&current, &payload).unwrap();

        assert_eq!(
            merged.security.allowed_origins,
            vec!["https://new.example.com".to_string()]
        );
    }

    #[test]
    fn merge_payload_replaces_allowed_ips_and_restores_loopback() {
        let current = sample_config();
        let mut payload = build_transfer_payload(&current, &ExportConfigOptions::all());
        payload.config.security.as_mut().unwrap().allowed_ips =
            Some(vec!["10.0.0.0/24".to_string()]);

        let merged = merge_payload(&current, &payload).unwrap();

        assert_eq!(
            merged.security.allowed_ips,
            vec!["127.0.0.1".to_string(), "10.0.0.0/24".to_string()]
        );
    }

    #[test]
    fn merge_payload_rejects_invalid_allowed_ips() {
        let current = sample_config();
        let mut payload = build_transfer_payload(&current, &ExportConfigOptions::all());
        payload.config.security.as_mut().unwrap().allowed_ips = Some(vec!["0.0.0.0".to_string()]);

        assert!(matches!(
            merge_payload(&current, &payload).unwrap_err(),
            ConfigTransferError::InvalidField(_)
        ));
    }

    #[test]
    fn merge_payload_rejects_invalid_values() {
        let current = sample_config();

        let mut invalid_origin = build_transfer_payload(&current, &ExportConfigOptions::all());
        invalid_origin
            .config
            .security
            .as_mut()
            .unwrap()
            .allowed_origins = Some(vec!["not-a-url".to_string()]);
        assert!(matches!(
            merge_payload(&current, &invalid_origin).unwrap_err(),
            ConfigTransferError::InvalidField(_)
        ));

        let mut invalid_port = build_transfer_payload(&current, &ExportConfigOptions::all());
        invalid_port.config.service.as_mut().unwrap().port = Some(0);
        assert!(matches!(
            merge_payload(&current, &invalid_port).unwrap_err(),
            ConfigTransferError::InvalidField(_)
        ));
    }

    #[test]
    fn preview_payload_lists_selected_service_and_security_fields() {
        let current = sample_config();
        let payload = build_transfer_payload(&current, &ExportConfigOptions::all());

        let preview = preview_payload(&current, &payload).unwrap();
        let keys: Vec<&str> = preview.items.iter().map(|item| item.key.as_str()).collect();

        assert!(keys.contains(&"service.port"));
        assert!(keys.contains(&"security.allowed_origins"));
        assert!(keys.contains(&"security.allowed_ips"));
        assert!(!keys.iter().any(|key| key.starts_with("remote.")));
    }
}
