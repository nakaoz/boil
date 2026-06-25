use std::time::{Duration, Instant};

use tokio::time::sleep;

use crate::{boil::BoilClient, config::Config};

pub struct IpQuality {
    pub country: String,
    pub isp: String,
    pub is_proxy: bool,
    pub is_hosting: bool,
}

impl IpQuality {
    pub fn cf_risk(&self) -> &'static str {
        if self.is_proxy || self.is_hosting { "高 ⚠️" } else { "低 ✅" }
    }
    pub fn ip_type(&self) -> &'static str {
        if self.is_proxy { "代理 ❌" } else if self.is_hosting { "机房 ⚠️" } else { "住宅 ✅" }
    }
}

pub struct ReconnectResult {
    pub old_ip: Option<String>,
    pub new_ip: Option<String>,
    pub reachable: bool,
    pub quality: Option<IpQuality>,
}

pub async fn do_reconnect(
    config: &Config,
    router_id: &str,
    interface: &str,
) -> anyhow::Result<ReconnectResult> {
    let c = BoilClient::new()?;
    let data = c.query_all_authed(&config.boil_account, &config.boil_password).await?;
    let old_ip = data.get_ip(router_id, interface).map(str::to_string);

    anyhow::ensure!(
        data.daily_limit == 0 || data.daily_used < data.daily_limit,
        "今日额度已用尽（{}/{}），明日再试",
        data.daily_used,
        data.daily_limit
    );

    c.reconnect(router_id, interface).await?;

    // 重拨后最多等待 60 秒检测 IP 变化。先给面板/拨号链路 8 秒缓冲，
    // 然后每 3 秒查询一次；最后一次 sleep 会按剩余时间截断，避免超过总等待上限。
    let ip_change_timeout = Duration::from_secs(60);
    let reconnect_wait = Duration::from_secs(8);
    let poll_interval = Duration::from_secs(3);
    let started_at = Instant::now();
    sleep(reconnect_wait).await;

    let mut new_ip: Option<String> = None;
    loop {
        let d2 = c.query_all().await?;
        let ip = d2.get_ip(router_id, interface).map(str::to_string);
        if ip.is_some() && ip != old_ip {
            new_ip = ip;
            break;
        }

        let elapsed = started_at.elapsed();
        if elapsed >= ip_change_timeout {
            break;
        }
        sleep(std::cmp::min(poll_interval, ip_change_timeout - elapsed)).await;
    }

    let (reachable, quality) = match &new_ip {
        Some(ip) => tokio::join!(check_reachable(ip), check_ip_quality(ip)),
        None => (false, None),
    };

    Ok(ReconnectResult { old_ip, new_ip, reachable, quality })
}

pub async fn check_ip_quality(ip: &str) -> Option<IpQuality> {
    let url = format!("http://ip-api.com/json/{ip}?fields=status,country,isp,proxy,hosting");
    let resp: serde_json::Value = reqwest::Client::new()
        .get(&url)
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    if resp["status"].as_str() != Some("success") {
        return None;
    }
    Some(IpQuality {
        country: resp["country"].as_str().unwrap_or("未知").to_string(),
        isp: resp["isp"].as_str().unwrap_or("未知").to_string(),
        is_proxy: resp["proxy"].as_bool().unwrap_or(false),
        is_hosting: resp["hosting"].as_bool().unwrap_or(false),
    })
}

pub async fn check_reachable(ip: &str) -> bool {
    for port in [80u16, 443, 22] {
        if tokio::time::timeout(
            Duration::from_secs(3),
            tokio::net::TcpStream::connect(format!("{ip}:{port}")),
        )
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false)
        {
            return true;
        }
    }
    false
}
