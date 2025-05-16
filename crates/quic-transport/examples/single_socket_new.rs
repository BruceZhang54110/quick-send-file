use anyhow::Result;
use quic_transport::quic_endpoint::ServerEndpointContainer;
use quic_transport::quic_endpoint::ClientEndpoint;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::IpAddr;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {

    // 提前生成证书文件
    // 1 个独立服务器：监听 5000 端口
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5000);
    let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    let server_endpoint = ServerEndpointContainer::new(addr, Path::new("server.cert"), Path::new("server.key"))?;
    tokio::spawn(async move {
        let connection = server_endpoint.get_endpoint().accept().await.unwrap().await.unwrap();
        println!(
            "[server] incoming connection: addr={}",
            connection.remote_address()
        );
        while let Ok((mut send, mut recv)) = connection.accept_bi().await {
            // Because it is a bidirectional stream, we can both send and receive.
            let recv_byte = recv.read_to_end(50).await.unwrap();
            println!("revc from client: {:?}", recv_byte);
            println!("revc from client: {:?}", String::from_utf8(recv_byte).unwrap());

            send.write_all(b"response").await.unwrap();
            send.finish().unwrap();
        }
    });

    let client_endpoint = ClientEndpoint::new(client_addr, Path::new("server.cert"))?;
    let client_endpoint = client_endpoint.get_endpoint();

    let connecting = client_endpoint.connect(addr, "localhost").unwrap();
    let connection = connecting.await.unwrap();
    println!("[client] connected: addr={}", connection.remote_address());

    let (mut send, mut recv) = connection.open_bi().await?;
    send.write_all(b"test").await?;
    send.finish()?;

    let recv_byte = recv.read_to_end(10).await?;
    println!("recv from server:{:?}", recv_byte);
    let recv_str = String::from_utf8(recv_byte).unwrap();
    println!("recv from server:{:?}", recv_str);

    client_endpoint.wait_idle().await;
    Ok(())

}