use std::{collections::BTreeMap, path::{Path, PathBuf}, time::Duration};

use crate::cli::{ReceiveArgs, SendArgs};
use anyhow::{Context, Result};
use arboard::Clipboard;
use futures::StreamExt;
use console::{style, Key, Term};
use tracing::info;
use walkdir::WalkDir;
use indicatif::{HumanBytes, HumanDuration, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use iroh::{node_info::UserData, protocol::Router, Endpoint, RelayMode, SecretKey};
use iroh_blobs::{format::collection::Collection, get::{db::DownloadProgress, fsm::{AtBlobHeaderNextError, DecodeError}, request::get_hash_seq_and_sizes}, net_protocol::Blobs, store::{ExportMode, ImportMode, ImportProgress}, ticket::BlobTicket, util::fs::canonicalized_path_to_string, BlobFormat, HashAndFormat, TempTag};
use data_encoding::HEXLOWER;
use rand::Rng;


/// create a endpoint
async fn create_endpoint() -> anyhow::Result<Endpoint> {
    let mut rng = rand::rngs::OsRng;
    let secret_key: SecretKey = SecretKey::generate(&mut rng);

    info!("开始创建endpoint");
    let endpoint = Endpoint::builder()
        // pplication-Layer_Protocol_Negotiation
        .alpns(vec![iroh_blobs::protocol::ALPN.to_vec()])
        .relay_mode(RelayMode::Disabled)
        //.discovery_n0()
        // use mDNS Discovery
        .discovery_local_network()
        .secret_key(secret_key)
        .bind().await?;

    // 获取并打印节点信息
    let user_data = UserData::try_from(String::from("local-nodes-example"))?;
    endpoint.set_user_data_for_discovery(Some(user_data));
    let node_id = endpoint.node_id();
    let node_addr = endpoint.node_addr().await?;
    info!("create node success, node_id: {}, node_addr: {:?}", node_id, node_addr);
    anyhow::Ok(endpoint)

}

/// 将文件导入数据库
async fn import(path: PathBuf, db: impl iroh_blobs::store::Store) -> anyhow::Result<(TempTag, u64, Collection)> {
    // 将路径转换为其​​绝对、规范化的形式​​
    let path = path.canonicalize().with_context(||
        format!("无法访问文件或目录：{}", path.display()))?;
        
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
    // 使用多cpu, 导入全部的文件,返回 names 和 temp tags
    let mut names_and_tags = futures_lite::stream::iter(data_source)
        .map(|(name, path)| {
            let db = db.clone();
            let progress = progress.clone();
            async move {
                let (temp_tag, file_size) = db
                    .import_file(path, ImportMode::TryReference, BlobFormat::Raw, progress)
                    .await?;
                anyhow::Ok((name, temp_tag, file_size))
            }
        }).buffer_unordered(num_cpus::get())
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;
    // 导入文件完成，销毁关闭发送器
    drop(progress);
    names_and_tags.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));
    let size = names_and_tags.iter()
        .map(|(_, _, size)| * size).sum::<u64>();
    // collect the (name, hash) tuples into a collection
    // we must also keep the tags around so the data does not get gced.
    let (collection, tags) = names_and_tags.into_iter()
        .map(|(name, tag, _)| ((name, *tag.hash()), tag))
        .unzip::<_, _, Collection, Vec<_>>();
    let temp_tag = collection.clone().store(&db).await?;
    // now that the collection is stored, we can drop the tags
    // data is protected by the collection
    drop(tags);
    show_progress.await??;
    Ok((temp_tag, size, collection))
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

/// 显示文件导入进度的异步函数
async fn show_ingest_progress(recv: async_channel::Receiver<ImportProgress>) -> anyhow::Result<()> {
    // 创建多进度条管理器
    let mp = MultiProgress::new();
    // 设置输出目标为标准错误输出
    mp.set_draw_target(ProgressDrawTarget::stderr());
    // 添加一个隐藏的进度条
    let op = mp.add(ProgressBar::hidden());
    op.set_style(
        ProgressStyle::default_spinner().template("{spinner:.green} [{elapsed_precise}] {msg}")?,
    );
    // op.set_message(format!("{} Ingesting ...\n", style("[1/2]").bold().dim()));
    // op.set_length(total_files);
    // 文件名称
    let mut names = BTreeMap::new();
    // 文件大小
    let mut sizes = BTreeMap::new();
    // 进度条
    let mut pbs = BTreeMap::new();

    loop {
        let x = recv.recv().await;
        match x {
            Ok(ImportProgress::Found { id, name }) => {
                names.insert(id, name);
            }
            Ok(ImportProgress::Size { id, size }) => {
                // 当获取到文件大小时，记录大小并创建进度条
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
                pb.set_style(ProgressStyle::with_template(
                    "{msg}{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}",
                )?.progress_chars("#>-"));
                pb.set_message(format!("{} {}", style("[2/2]").bold().dim(), name));
                pb.set_length(size);
                pbs.insert(id, pb);
            }
            Ok(ImportProgress::OutboardProgress { id, offset }) => {
                if let Some(pb) = pbs.get(&id) {
                    pb.set_position(offset);
                }
            }
            Ok(ImportProgress::OutboardDone { id, .. }) => {
                // you are not guaranteed to get any OutboardProgress
                if let Some(pb) = pbs.remove(&id) {
                    pb.finish_and_clear();
                }
            }
            Ok(ImportProgress::CopyProgress { .. }) => {
                // we are not copying anything
            }
            Err(e) => {
                op.set_message(format!("Error receiving progress: {e}"));
                break;
            }
        }
    }
    op.finish_and_clear();
    Ok(())
}

/// 展示下载进度
pub async fn show_download_progress(
    recv: async_channel::Receiver<DownloadProgress>,
    total_size: u64,
) -> anyhow::Result<()> {
    let mp = MultiProgress::new();
    mp.set_draw_target(ProgressDrawTarget::stderr());
    let op = mp.add(make_download_progress());
    op.set_message(format!("{} Connecting ...\n", style("[1/3]").bold().dim()));
    let mut total_done = 0;
    let mut sizes = BTreeMap::new();
    loop {
        let x = recv.recv().await;
        info!("DownloadProgress:{:?}", x);
        match x {
            Ok(DownloadProgress::Connected) => {
                op.set_message(format!("{} Requesting ...\n", style("[2/3]").bold().dim()));
            }
            Ok(DownloadProgress::FoundHashSeq { children, .. }) => {
                op.set_message(format!(
                    "{} Downloading {} blob(s)\n",
                    style("[3/3]").bold().dim(),
                    children + 1,
                ));
                op.set_length(total_size);
                op.reset();
            }
            Ok(DownloadProgress::Found { id, size, .. }) => {
                sizes.insert(id, size);
            }
            Ok(DownloadProgress::Progress { offset, .. }) => {
                op.set_position(total_done + offset);
            }
            Ok(DownloadProgress::Done { id }) => {
                total_done += sizes.remove(&id).unwrap_or_default();
            }
            Ok(DownloadProgress::AllDone(stats)) => {
                op.finish_and_clear();
                eprintln!(
                    "Transferred {} in {}, {}/s",
                    HumanBytes(stats.bytes_read),
                    HumanDuration(stats.elapsed),
                    HumanBytes((stats.bytes_read as f64 / stats.elapsed.as_secs_f64()) as u64)
                );
                break;
            }
            Ok(DownloadProgress::Abort(e)) => {
                anyhow::bail!("download aborted: {e:?}");
            }
            Err(e) => {
                anyhow::bail!("error reading progress: {e:?}");
            }
            _ => {}
        }
    }
    Ok(())
}



fn add_to_clipboard(ticket: &BlobTicket) {
    let clipboard = Clipboard::new();
    match clipboard {
        Ok(mut clip) => {
            if let Err(e) = clip.set_text(format!("sendme receive {}", ticket)) {
                eprintln!("Could not add to clipboard: {}", e);
            } else {
                println!("Command added to clipboard.")
            }
        }
        Err(e) => eprintln!("Could not access clipboard: {}", e),
    }
}


fn show_get_error(e: anyhow::Error) -> anyhow::Error {
    if let Some(err) = e.downcast_ref::<DecodeError>() {
        match err {
            DecodeError::NotFound => {
                eprintln!("{}", style("send side no longer has a file").yellow())
            }
            DecodeError::LeafNotFound(_) | DecodeError::ParentNotFound(_) => eprintln!(
                "{}",
                style("send side no longer has part of a file").yellow()
            ),
            DecodeError::Io(err) => eprintln!(
                "{}",
                style(format!("generic network error: {}", err)).yellow()
            ),
            DecodeError::Read(err) => eprintln!(
                "{}",
                style(format!("error reading data from quinn: {}", err)).yellow()
            ),
            DecodeError::LeafHashMismatch(_) | DecodeError::ParentHashMismatch(_) => {
                eprintln!("{}", style("send side sent wrong data").red())
            }
        };
    } else if let Some(header_error) = e.downcast_ref::<AtBlobHeaderNextError>() {
        // TODO(iroh-bytes): get_to_db should have a concrete error type so you don't have to guess
        match header_error {
            AtBlobHeaderNextError::Io(err) => eprintln!(
                "{}",
                style(format!("generic network error: {}", err)).yellow()
            ),
            AtBlobHeaderNextError::Read(err) => eprintln!(
                "{}",
                style(format!("error reading data from quinn: {}", err)).yellow()
            ),
            AtBlobHeaderNextError::NotFound => {
                eprintln!("{}", style("send side no longer has a file").yellow())
            }
        };
    } else {
        eprintln!(
            "{}",
            style(format!("generic error: {:?}", e.root_cause())).red()
        );
    }
    e
}

/// 验证路径是否有效 
/// 路径不能包含 '/'
fn validate_path_component(component: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !component.contains('/'),
        "path components must not contain the only correct path separator, /"
    );
    anyhow::Ok(())
}

/// get 导出路径
fn get_export_path(root: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let parts = name.split('/');
    let mut path = root.to_path_buf();
    for part in parts {
        validate_path_component(part)?;
        path.push(part);
    }
    Ok(path)
}

/// 导出文件
async fn export(db: impl iroh_blobs::store::Store, collection: Collection) -> anyhow::Result<()> {
    // get current dir
    let root = std::env::current_dir()?;
    for (name, hash) in collection.iter() {
        let target = get_export_path(&root, name)?;
        if target.exists() {
            eprintln!(
                "target {} already exists. Export stopped.",
                target.display()
            );
            eprintln!("You can remove the file or directory and try again. The download will not be repeated.");
            anyhow::bail!("target {} already exists", target.display());
        }
        db.export(
            *hash, 
            target, 
            ExportMode::TryReference, 
            Box::new(move |_position| Ok(()))
        ,).await?;

    }
    Ok(())
}

/// 文件传输
/// 发送文件
/// 返回文件码
pub async fn send_file(args: SendArgs) -> anyhow::Result<()> {
    // 创建 endpoint
    let endpoint = create_endpoint().await?;

    // 临时目录
    // use a flat store - todo: use a partial in mem store instead
    let suffix = rand::thread_rng().gen::<[u8; 16]>();
    let cwd = std::env::current_dir()?;
    let blobs_data_dir = cwd.join(format!(".sendme-send-{}", HEXLOWER.encode(&suffix)));
    if blobs_data_dir.exists() {
        println!(
            "can not share twice from the same directory: {}",
            cwd.display(),
        );
        std::process::exit(1);
    }
    tokio::fs::create_dir_all(&blobs_data_dir).await?;

    // 创建 blobs
    let blobs = Blobs::persistent(&blobs_data_dir).await?.build(&endpoint);
    

    let router = Router::builder(endpoint)
        .accept(iroh_blobs::ALPN, blobs.clone())
        .spawn();

    let path = args.path;
    let (temp_tag, size, collection) = import(path.clone(), blobs.store().clone()).await?;
    let hash = *temp_tag.hash();

    // let _ = router.endpoint().home_relay().initialized().await?;

    // 生成ticket
    let addr = router.endpoint().node_addr().await?;
    let ticket = BlobTicket::new(addr, hash, BlobFormat::HashSeq)?;

    let entry_type = if path.is_file() { "file" } else { "directory" };

    println!(
        "import {} {}, {}, hash: {}",
        entry_type,
        path.display(),
        HumanBytes(size),
        hash.to_string()
    );

    for (name, hash) in collection.iter() {
        println!("    {} {name}", hash.to_string());
    }

    println!("to get this data, use");
    println!("transfer receive {}", ticket);


    let _keyboard = tokio::task::spawn(async move {
        let term = Term::stdout();
        println!("press c to copy command to clipboard, or use the --clipboard argument");
        loop {
            if let Ok(Key::Char('c')) = term.read_key() {
                add_to_clipboard(&ticket);
            }
        }
    });

    tokio::signal::ctrl_c().await?;

    drop(temp_tag);

    tokio::time::timeout(Duration::from_secs(2), router.shutdown()).await??;
    tokio::fs::remove_dir_all(blobs_data_dir).await?;
    println!("shutting down");


    anyhow::Ok(())

}


/// 接收文件方法
pub async fn receive_file(args: ReceiveArgs) -> anyhow::Result<()> {
    // Use short code instead of tickets
    let ticket = args.code;
    let addr = ticket.node_addr().clone();
    let endpoint: Endpoint = create_endpoint().await?;


    let dir_name: String = format!(".re-sendme-get-{}", ticket.hash().to_hex());
    let iroh_data_dir = std::env::current_dir()?.join(dir_name);
    let db = iroh_blobs::store::fs::Store::load(&iroh_data_dir).await?;
    let mp: MultiProgress = MultiProgress::new();
    let connect_progress: ProgressBar = mp.add(ProgressBar::hidden());
    connect_progress.set_draw_target(ProgressDrawTarget::stderr());
    connect_progress.set_style(ProgressStyle::default_spinner());
    connect_progress.set_message(format!("connecting to {}", addr.node_id));
    let connection = endpoint.connect(addr, iroh_blobs::protocol::ALPN).await?;
    let hash_and_format = HashAndFormat {
        hash: ticket.hash(),
        format: ticket.format(),
    };
    connect_progress.finish_and_clear();
    let (send, recv) = async_channel::bounded(32);
    let progress = iroh_blobs::util::progress::AsyncChannelProgressSender::new(send);
    let (_hash_seq, sizes) =
        get_hash_seq_and_sizes(&connection, &hash_and_format.hash, 1024 * 1024 * 32)
            .await
            .map_err(show_get_error)?;
    let total_size = sizes.iter().sum::<u64>();
    let total_files = sizes.len().saturating_sub(1);
    let payload_size = sizes.iter().skip(1).sum::<u64>();
    eprintln!(
        "getting collection {} {} files, {}",
        &ticket.hash().to_string(),
        total_files,
        HumanBytes(payload_size)
    );
    eprintln!(
        "getting {} blobs in total, {}",
        sizes.len(),
        HumanBytes(total_size)
    );
    let _task = tokio::spawn(show_download_progress(recv, total_size));
    let get_conn = || async move { Ok(connection) };
    let stats = iroh_blobs::get::db::get_to_db(&db, get_conn, &hash_and_format, progress)
        .await
        .map_err(|e| show_get_error(anyhow::anyhow!(e)))?;
    let collection = Collection::load_db(&db, &hash_and_format.hash).await?;
    for (name, hash) in collection.iter() {
        println!("    {} {name}", hash.to_string());
    }
    if let Some((name, _)) = collection.iter().next() {
        if let Some(first) = name.split('/').next() {
            println!("downloading to: {};", first);
        }
    }
    export(db, collection).await?;
    tokio::fs::remove_dir_all(iroh_data_dir).await?;

    println!(
            "downloaded {} files, {}. took {} ({}/s)",
            total_files,
            HumanBytes(payload_size),
            HumanDuration(stats.elapsed),
            HumanBytes((stats.bytes_read as f64 / stats.elapsed.as_secs_f64()) as u64),
        );
    
    Ok(())
}

