use std::{ffi::OsStr, path::{Path, PathBuf}};
use anyhow::{Ok, Result};
use tokio::{fs::File, io::AsyncReadExt};
use tokio::io::BufReader;
use quinn::SendStream;
use bytes::{Bytes, BytesMut};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use quic_transport::quic_endpoint::ClientEndpoint;



/// 文件传输结构体
#[derive(Debug)]
pub struct FileTransfer {
    /// 文件路径
    pub file_path: PathBuf,
    /// 文件名
    pub file_name: String,
    /// 分块大小（字节）
    pub chunk_size: usize,
    /// 文件大小（字节）
    pub file_size: usize,
    /// 传输状态
    pub state: TransferState,
}

#[derive(Debug, PartialEq)]
pub enum TransferState {
    /// 等待开始
    Queued,
    /// 传输中
    Transferring,
    /// 传输完成
    Finished,
    /// 传输失败
    Failed,

}


impl FileTransfer {
    /// 创建文件传输结构体
    pub fn new(source_path: impl AsRef<Path>, chunk_size: usize) -> Self {
        let path = source_path.as_ref();
        Self{
            file_path: path.to_path_buf(),
            file_name: path.file_name().unwrap_or(OsStr::new("未命名")).to_string_lossy().to_string(),
            chunk_size: chunk_size,
            file_size: path.metadata().map(|m| m.len() as usize).unwrap_or(0),
            state: TransferState::Queued,
        }
    }

    /// 打开文件
    async fn open_file(&self) -> Result<File> {
        // 打开文件
        let file = tokio::fs::File::open(&self.file_path).await?;
        Ok(file)
    }

    /// 发送文件
    async fn send_file(&self, mut send_stream: SendStream) -> Result<()> {
        // 打开文件
        let file = self.open_file().await?;
        // 读取文件
        let mut reader = BufReader::new(file);
        // let mut buffer = vec![0u8; self.chunk_size];
        let mut buffer = BytesMut::with_capacity(self.chunk_size);
        loop {
            // 重用缓冲区空间
            buffer.clear();
            let bytes_read = reader.read_buf(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            let chunk: Bytes = buffer.split().freeze();
            send_stream.write_chunks(&mut [chunk]).await?;
            // 发送数据
            println!("发送数据: {:?}", &buffer[..bytes_read]);
        }
        let finish = send_stream.finish();
        if finish.is_err() {
            println!("发送数据失败: {:?}", finish);
            return Err(anyhow::anyhow!("发送文件失败"));
        }
        Ok(())
    }
    
}


#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_new_file_transfer() {
        let file_transfer = FileTransfer::new("test.txt", 1024);
        println!("文件:{:?}", file_transfer);
        assert_eq!(file_transfer.file_name, "test.txt");
        assert_eq!(file_transfer.chunk_size, 1024);
        assert_eq!(file_transfer.state, TransferState::Queued);
    }

    #[tokio::test]
    async fn test_open_file() {
        let file_transfer = FileTransfer::new("../../test.txt", 64);
        println!("文件:{:?}", file_transfer);
        let result = file_transfer.open_file().await;
        println!("打开文件结果:{:?}", result);
    
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_file() {
        // 读取文件
        let file_transfer = FileTransfer::new("../../test.txt", 64);
        println!("文件:{:?}", file_transfer);
        // 创建客户端
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5000);
        ClientEndpoint::new(addr1, server_certs);
        let result = file_transfer.send_file().await;
        println!("发送文件结果:{:?}", result);
    
        assert!(result.is_ok());
    }

}
