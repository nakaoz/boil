use dialoguer::Select;

use crate::{boil::BoilClient, config::Config, core::do_reconnect};

pub async fn cmd_status(config: &Config) -> anyhow::Result<()> {
    let c = BoilClient::new()?;
    c.login(&config.boil_account, &config.boil_password).await?;
    let data = c.query_all().await?;

    println!("📡 服务器状态 | 今日换 IP {}/{} 次\n", data.daily_used, data.daily_limit);
    for item in &data.zone_items {
        let ip = data.get_ip(&item.router_id, &item.interface).unwrap_or("未知");
        let tag = if item.nat_no_change { "🔒 NAT" } else { "✅ 可换" };
        println!("  {}\n  IP: {}  {}\n", item.label, ip, tag);
    }
    Ok(())
}

pub async fn cmd_change(config: &Config) -> anyhow::Result<()> {
    let c = BoilClient::new()?;
    c.login(&config.boil_account, &config.boil_password).await?;
    let data = c.query_all().await?;

    let changeable = data.changeable();
    if changeable.is_empty() {
        println!("⚠️  没有可换 IP 的服务器");
        return Ok(());
    }

    let target = if changeable.len() == 1 {
        changeable[0]
    } else {
        let labels: Vec<String> = changeable
            .iter()
            .map(|r| {
                let ip = data.get_ip(&r.router_id, &r.interface).unwrap_or("未知");
                format!("{} ({})", r.label, ip)
            })
            .collect();
        let idx = Select::new()
            .with_prompt("选择要换 IP 的服务器")
            .items(&labels)
            .default(0)
            .interact()?;
        changeable[idx]
    };

    println!("⏳ 换 IP 中...");
    let res = do_reconnect(config, &target.router_id, &target.interface).await?;

    match res.new_ip {
        Some(new_ip) => {
            let reach = if res.reachable { "TCP 可达 ✅" } else { "TCP 未通 ⚠️" };
            println!(
                "\n✅ 换 IP 完成\n   旧 IP: {}\n   新 IP: {}  {}\n",
                res.old_ip.as_deref().unwrap_or("未知"),
                new_ip,
                reach,
            );
            if let Some(q) = res.quality {
                println!(
                    "📊 IP 质量\n   地区: {}\n   ISP:  {}\n   类型: {}\n   CF 风险: {}",
                    q.country, q.isp, q.ip_type(), q.cf_risk()
                );
            }
        }
        None => {
            println!(
                "⚠️  重拨已触发，但未检测到 IP 变化\n   旧 IP: {}\n   请到面板手动确认",
                res.old_ip.as_deref().unwrap_or("未知")
            );
        }
    }
    Ok(())
}
