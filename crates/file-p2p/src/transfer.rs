use std::{collections::BTreeMap, path::PathBuf};

use crate::cli::SendArgs;
use anyhow::{Context, Result};
use std::result::Result::Ok;
use console::style;
use indicatif::{style, HumanBytes, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use iroh::{protocol::Router, Endpoint, SecretKey};
use iroh_blobs::{format::collection::Collection, net_protocol::Blobs, store::ImportProgress, util::fs::canonicalized_path_to_string, TempTag};
use walkdir::WalkDir;


/// create a endpoint
async fn create_endpoint() -> anyhow::Result<Endpoint> {
    let mut rng = rand::rngs::OsRng;
    let secret_key: SecretKey = SecretKey::generate(&mut rng);

    let endpoint = Endpoint::builder()
        // pplication-Layer_Protocol_Negotiation
        .alpns(vec![iroh_blobs::protocol::ALPN.to_vec()])
        .discovery_n0()
        // use mDNS Discovery
        .discovery_local_network()
        .secret_key(secret_key)
        .bind().await?;
    anyhow::Ok(endpoint)

}

/// 将文件导入数据库
async fn import(path: PathBuf, db: impl iroh_blobs::store::Store) -> anyhow::Result<(TempTag, u64, Collection)> {
    // 将路径转换为其​​绝对、规范化的形式​​
    let path = path.canonicalize()?;
    anyhow::ensure!(path.exists(), "path {} does not exist", path.display());
    let root= path.parent().context("context get parent")?;
    
    // 递归获取文件目录
    let files = WalkDir::new(path.clone()).into_iter();
    let data_source = files.map(|entry| {
        let entry = entry?;
        // 过滤掉非文件
        if !entry.file_type().is_file() {
            return anyhow::Ok(None);
        }
        let path = entry.into_path();
        // 相对路径作为name
        let relative = path.strip_prefix(root)?;
        let name = canonicalized_path_to_string(relative, true)?;
        anyhow::Ok(Some((name, path)))
    }).filter_map(Result::transpose).collect::<anyhow::Result<Vec<_>>>()?;

    let (send, recv) = async_channel::bounded(32);
    let progress = iroh_blobs::util::progress::AsyncChannelProgressSender::new(send);
    let show_progress = tokio::spawn(show_ingest_progress(recv));




    todo!()
}

fn make_download_progress() -> ProgressBar {
    let pb = ProgressBar::hidden();
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb.set_style(
        ProgressStyle::with_template(
            "{msg}{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} {binary_bytes_per_sec}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    pb
}

async fn show_ingest_progress(recv: async_channel::Receiver<ImportProgress>) -> anyhow::Result<()> {
    let mp = MultiProgress::new();
    mp.set_draw_target(ProgressDrawTarget::stderr());
    let op = mp.add(make_download_progress());
    op.set_message(format!("{} Connecting ...\n", style("[1/3]").bold().dim()));
    let mut total_done = 0;
    let mut names = BTreeMap::new();
    let mut sizes = BTreeMap::new();
    let mut pbs = BTreeMap::new();

    loop {
        let x = recv.recv().await;
        match x {
            Ok(ImportProgress::Found { id, name }) => {
                names.insert(id, name);
            }
            Ok(ImportProgress::Size { id, size }) => {
                sizes.insert(id, size);
                let total_size = sizes.values().sum::<u64>();
                op.set_message(format!(
                    "{} Ingesting {} files, {}\n",
                    style("[1/2]").bold().dim(),
                    sizes.len(),
                    HumanBytes(total_size)
                ));
                let name = names.get(&id).cloned().unwrap_or_default();
                let pb = mp.add(ProgressBar::hidden());
                pbs.insert(id, pb);
            }
            _ => {}
        }
    }
    todo!()
}

/// 文件传输
/// 发送文件
/// 返回文件码
pub async fn send_file(args: SendArgs) -> anyhow::Result<String> {
    // 创建 endpoint
    let endpoint = create_endpoint().await?;
    // 创建 blobs
    let blobs = Blobs::persistent(args.path).await?.build(&endpoint);
    

    let router = Router::builder(endpoint)
        .accept(iroh_blobs::ALPN, blobs.clone())
        .spawn();
    
    anyhow::Ok("".to_string())

}
