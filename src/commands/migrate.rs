use crate::config::Config;
use anyhow::{bail, Context, Result};
use serde_json::to_string_pretty;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn run(
    config: &Config,
    output: Option<PathBuf>,
    auto_confirm: bool,
    dry_run: bool,
    detected_source: Option<&Path>,
    inline_source: bool,
) -> Result<()> {
    let source_path = detected_source.map(Path::to_path_buf);

    let source_path = match source_path {
        Some(path) => path,
        None if inline_source => {
            bail!(
                "設定ファイルのパス情報が取得できませんでした。Node ラッパーが最新ではない可能性があるため、`npm install` を再実行するか `--config` で対象ファイルを指定してください。"
            );
        }
        None => {
            bail!(
                "移行対象の設定ファイルが見つかりません。`i18next-parser.config.js` など既存の設定を --config で指定するか、プロジェクトルートに配置してから再度実行してください。"
            );
        }
    };

    if source_path
        .file_name()
        .map(|name| name == "i18next-turbo.json")
        .unwrap_or(false)
    {
        println!(
            "注意: {} は既に i18next-turbo.json 形式です。--output で別名を指定できます。",
            source_path.display()
        );
    }

    let target_path = output.unwrap_or_else(|| PathBuf::from("i18next-turbo.json"));
    let preview = to_string_pretty(config)?;

    println!("検出した設定ファイル: {}", source_path.display());
    println!("出力先: {}", target_path.display());
    println!(
        "\n--- 変換後プレビュー ---\n{}\n--- プレビュー終了 ---\n",
        preview
    );

    if dry_run {
        println!("dry-run のためファイルは書き込まれていません。");
        return Ok(());
    }

    if target_path.exists() && !auto_confirm {
        if !prompt_yes_no(&format!(
            "{} は既に存在します。上書きしますか?",
            target_path.display()
        ))? {
            println!("操作をキャンセルしました。");
            return Ok(());
        }
    } else if !auto_confirm && !prompt_yes_no("生成した設定を保存しますか?")? {
        println!("操作をキャンセルしました。");
        return Ok(());
    }

    fs::write(&target_path, format!("{}\n", preview))
        .with_context(|| format!("{} への書き込みに失敗しました", target_path.display()))?;

    println!("保存しました: {}", target_path.display());
    Ok(())
}

fn prompt_yes_no(message: &str) -> Result<bool> {
    let mut stdout = io::stdout();
    loop {
        print!("{} [y/N]: ", message);
        stdout
            .flush()
            .context("標準出力への書き込みに失敗しました")?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("標準入力からの読み取りに失敗しました")?;
        match input.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" | "" => return Ok(false),
            _ => {
                println!("y か n を入力してください。");
            }
        }
    }
}
