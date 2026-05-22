use clap::{Args, Subcommand};
use comfy_table::{presets::UTF8_FULL_CONDENSED, Table};
use rusqlite::Connection;

use mango_core::account::keyring::SystemKeyring;
use mango_core::account::service as account_service;
use mango_core::db::queries::api_account as account_queries;
use mango_core::models::api_account::CreateApiAccountInput;

#[derive(Args)]
pub struct AccountArgs {
    #[command(subcommand)]
    action: AccountAction,
}

#[derive(Subcommand)]
enum AccountAction {
    /// 新增 API 账号；密钥写入系统 keyring，DB 仅留后 4 位
    Add {
        #[arg(long)]
        provider_id: String,
        #[arg(long)]
        label: String,
        /// API key。缺省时通过 stdin 安全读取（不回显）
        #[arg(long)]
        key: Option<String>,
    },
    /// 列出账号；密钥仅展示 ****<last4>
    List {
        #[arg(long)]
        provider_id: Option<String>,
    },
    /// 删除账号；keyring 条目同步清除
    Remove { id: String },
    /// 校验 keyring 中是否存有非空密钥（不联网测试 provider）
    Verify { id: String },
    // 注：刻意没有 update 子命令。修改 label 走 GUI；替换密钥走 remove + add。
}

pub fn execute(conn: &Connection, args: AccountArgs) -> anyhow::Result<()> {
    // Register the platform's native credential store with keyring-core before
    // touching SystemKeyring. Idempotent across processes — calling it multiple
    // times in the same process is safe but pointless.
    keyring::use_native_store(false)
        .map_err(|e| anyhow::anyhow!("failed to register native keyring store: {e}"))?;

    let keyring = SystemKeyring;
    match args.action {
        AccountAction::Add {
            provider_id,
            label,
            key,
        } => add(conn, &keyring, provider_id, label, key),
        AccountAction::List { provider_id } => list(conn, provider_id),
        AccountAction::Remove { id } => remove(conn, &keyring, &id),
        AccountAction::Verify { id } => verify(conn, &keyring, &id),
    }
}

fn add(
    conn: &Connection,
    keyring: &SystemKeyring,
    provider_id: String,
    label: String,
    key: Option<String>,
) -> anyhow::Result<()> {
    let api_key = match key {
        Some(k) => k,
        None => rpassword::prompt_password("API Key: ")?,
    };
    let acc = account_service::create(
        conn,
        keyring,
        CreateApiAccountInput {
            provider_id,
            label,
            api_key,
        },
    )?;
    println!("账号已创建");
    println!("  ID:    {}", acc.id);
    println!("  标签:  {}", acc.label);
    println!("  密钥:  ****{}", acc.key_last4);
    Ok(())
}

fn list(conn: &Connection, provider_id: Option<String>) -> anyhow::Result<()> {
    let rows = account_queries::list(conn, provider_id)?;
    if rows.is_empty() {
        println!("尚无 API 账号。用 `mango account add` 添加。");
        return Ok(());
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec!["ID", "Provider", "Label", "Key", "Created"]);
    for a in &rows {
        table.add_row(vec![
            short_id(&a.id),
            a.provider_id.clone(),
            a.label.clone(),
            format!("****{}", a.key_last4),
            a.created_at.clone(),
        ]);
    }
    println!("{table}");
    println!("\n共 {} 个账号", rows.len());
    Ok(())
}

fn remove(conn: &Connection, keyring: &SystemKeyring, id: &str) -> anyhow::Result<()> {
    account_service::delete(conn, keyring, id)?;
    println!("账号已删除");
    Ok(())
}

fn verify(conn: &Connection, keyring: &SystemKeyring, id: &str) -> anyhow::Result<()> {
    account_service::verify_storage(conn, keyring, id)?;
    println!("ok");
    Ok(())
}

fn short_id(id: &str) -> String {
    let n = 8.min(id.len());
    id[..n].to_string()
}
