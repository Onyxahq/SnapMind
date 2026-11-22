//! API Key 存在系统凭据库（Windows 凭据管理器 / Linux Secret Service），不写进配置文件。
//! 服务名固定为 `SnapMind`，账户名是服务商 id。

const SERVICE: &str = "SnapMind";

#[derive(Debug, thiserror::Error)]
#[error("访问系统凭据库失败：{0}")]
pub struct SecretError(String);

fn entry(account: &str) -> Result<keyring::Entry, SecretError> {
    keyring::Entry::new(SERVICE, account).map_err(|e| SecretError(e.to_string()))
}

/// 保存（空字符串表示删除）。
pub fn set(account: &str, secret: &str) -> Result<(), SecretError> {
    let secret = secret.trim();
    if secret.is_empty() {
        return delete(account);
    }
    entry(account)?.set_password(secret).map_err(|e| SecretError(e.to_string()))
}

/// 读取；不存在时返回 None。
pub fn get(account: &str) -> Result<Option<String>, SecretError> {
    match entry(account)?.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(SecretError(e.to_string())),
    }
}

/// 删除；本来就没有也算成功。
pub fn delete(account: &str) -> Result<(), SecretError> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(SecretError(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真的读写一次系统凭据库（用专门的测试账户，测完删除）。
    #[cfg(windows)]
    #[test]
    fn roundtrip_in_system_store() {
        let acct = "snapmind-selftest";
        set(acct, " secret-123 ").unwrap();
        assert_eq!(get(acct).unwrap().as_deref(), Some("secret-123"));
        set(acct, "").unwrap();
        assert_eq!(get(acct).unwrap(), None);
        delete(acct).unwrap();
    }
}
