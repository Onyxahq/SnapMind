//! 全软件共用的网络出口：大模型、机器翻译、模型下载、检查更新都从这里拿 HTTP 客户端，
//! 所以「代理」只在这一处生效。
//!
//! - 没开代理：沿用系统设置（reqwest 默认行为）。
//! - 开了代理：所有对外请求走它；本机地址（Ollama 等）不走代理。
//! - 开了「代理不通时直连」：每次出门前先探一下代理端口通不通（结果缓存 10 秒），不通就这次直连。

use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use reqwest::{Client, ClientBuilder, NoProxy, Proxy, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxySetting {
    pub address: String,
    pub fallback_direct: bool,
}

/// 这一次请求该怎么出门。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// 没有配置代理，按系统默认
    System,
    Proxy(Url),
    /// 代理不通（或地址无效），按设置直连
    Direct,
}

static CURRENT: RwLock<Option<ProxySetting>> = RwLock::new(None);
/// (代理地址, 探测时间, 通不通)
static PROBE: Mutex<Option<(String, Instant, bool)>> = Mutex::new(None);

const PROBE_TTL: Duration = Duration::from_secs(10);
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const LOCAL: &str = "localhost,127.0.0.1,::1,*.local";

/// 设置页改了代理就调用；`None` 表示不用代理。
pub fn set_proxy(setting: Option<ProxySetting>) {
    *CURRENT.write().expect("代理设置锁") = setting;
    *PROBE.lock().expect("代理探测锁") = None;
}

/// 把用户填的地址整理成 URL：可以只写 `127.0.0.1:7890`（当作 HTTP 代理），
/// 也可以写 `http://`、`https://`、`socks5://`、`socks5h://`。
pub fn parse_address(input: &str) -> Result<Url, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("empty".into());
    }
    let full = if s.contains("://") { s.to_string() } else { format!("http://{s}") };
    let url = Url::parse(&full).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h") || url.host_str().is_none() {
        return Err("scheme".into());
    }
    Ok(url)
}

async fn reachable(url: &Url) -> bool {
    let key = url.as_str().to_string();
    if let Some((k, at, ok)) = PROBE.lock().expect("代理探测锁").as_ref() {
        if *k == key && at.elapsed() < PROBE_TTL {
            return *ok;
        }
    }
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']).to_string();
    let port = url.port_or_known_default().unwrap_or(match url.scheme() {
        "socks5" | "socks5h" => 1080,
        _ => 80,
    });
    let ok = tokio::time::timeout(PROBE_TIMEOUT, tokio::net::TcpStream::connect((host.as_str(), port))).await.is_ok_and(|r| r.is_ok());
    *PROBE.lock().expect("代理探测锁") = Some((key, Instant::now(), ok));
    ok
}

/// 这一次请求该怎么出门（会探测代理通不通）。
pub async fn route() -> Route {
    let current = CURRENT.read().expect("代理设置锁").clone();
    route_for(current).await
}

async fn route_for(setting: Option<ProxySetting>) -> Route {
    let Some(p) = setting else {
        return Route::System;
    };
    match parse_address(&p.address) {
        Ok(url) if reachable(&url).await => Route::Proxy(url),
        // 代理不通：要么按设置直连，要么照样走代理，让请求报出明确的连接失败
        Ok(url) => {
            if p.fallback_direct {
                Route::Direct
            } else {
                Route::Proxy(url)
            }
        }
        Err(_) if p.fallback_direct => Route::Direct,
        Err(_) => Route::System,
    }
}

/// 按当前代理设置建客户端。所有对外网络请求都应该用它，而不是直接 `Client::builder()`。
pub async fn client(builder: ClientBuilder) -> Result<Client, reqwest::Error> {
    let builder = match route().await {
        Route::System => builder,
        Route::Direct => builder.no_proxy(),
        Route::Proxy(url) => builder.proxy(Proxy::all(url)?.no_proxy(NoProxy::from_string(LOCAL))),
    };
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_forms() {
        assert_eq!(parse_address("127.0.0.1:7890").unwrap().as_str(), "http://127.0.0.1:7890/");
        assert_eq!(parse_address(" socks5://localhost:1080 ").unwrap().scheme(), "socks5");
        assert!(parse_address("ftp://a:1").is_err());
        assert!(parse_address("").is_err());
        assert!(parse_address("http://").is_err());
    }

    #[tokio::test]
    async fn unreachable_proxy_falls_back_only_when_asked() {
        // 端口 1 基本没人监听
        let p = |fallback_direct| Some(ProxySetting { address: "127.0.0.1:1".into(), fallback_direct });
        assert_eq!(route_for(p(true)).await, Route::Direct);
        assert!(matches!(route_for(p(false)).await, Route::Proxy(_)));
        assert_eq!(route_for(None).await, Route::System);
    }
}
