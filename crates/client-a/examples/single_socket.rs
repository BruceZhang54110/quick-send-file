use anyhow::Result;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::sync::Arc;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use quinn::{ClientConfig, Endpoint, ServerConfig};

#[tokio::main]
async fn main() -> Result<()> {

    // 创建三个地址，分别启动三个服务器，分别返回三个证书
    // 3 个独立服务器：分别监听 5000/5001/5002 端口
    let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5000);
    let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5001);
    let addr3 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5002);
    // 每个服务器生成自签名证书
    let server1_cert = run_server(addr1)?;
    let server2_cert = run_server(addr2)?;
    let server3_cert = run_server(addr3)?;

    // 客户端显式信任这三个证书
    let client_endpoint = make_client_endpoint(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        &[&server1_cert, &server2_cert, &server3_cert],
    )?;

    // connect to multiple endpoints using the same socket/endpoint
    tokio::join!(
        run_client(&client_endpoint, addr1),
        run_client(&client_endpoint, addr2),
        run_client(&client_endpoint, addr3),
    );

    // Make sure the server has a chance to clean up
    client_endpoint.wait_idle().await;


    Result::Ok(())
    
}

/// Constructs a QUIC endpoint configured for use a client only.
///
/// ## Args
///
/// - server_certs: list of trusted certificates.
#[allow(unused)]
pub fn make_client_endpoint(
    bind_addr: SocketAddr,
    server_certs: &[&[u8]],
) -> Result<Endpoint> {
    let client_cfg = configure_client(server_certs)?;
    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(client_cfg);
    Ok(endpoint)
}

fn run_server(
    addr: SocketAddr,
) -> Result<CertificateDer<'static>> {
    let (endpoint, server_cert) = make_server_endpoint(addr)?;
    // accept a single connection
    tokio::spawn(async move {
        let connection = endpoint.accept().await.unwrap().await.unwrap();
        println!(
            "[server] incoming connection: addr={}",
            connection.remote_address()
        );
    });

    Ok(server_cert)
}

/// Attempt QUIC connection with the given server address.
async fn run_client(endpoint: &Endpoint, server_addr: SocketAddr) {
    let connect = endpoint.connect(server_addr, "localhost").unwrap();
    let connection = connect.await.unwrap();
    println!("[client] connected: addr={}", connection.remote_address());
}

pub fn make_server_endpoint(
    bind_addr: SocketAddr,
) -> Result<(Endpoint, CertificateDer<'static>)> {
    let (server_config, server_cert) = configure_server()?;
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, server_cert))
}

/// Returns default server configuration along with its certificate.
fn configure_server()
-> Result<(ServerConfig, CertificateDer<'static>)> {
    // 使用 rcgen 生成自签名证书
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = CertificateDer::from(cert.cert);
    let priv_key = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

    let mut server_config =
        ServerConfig::with_single_cert(vec![cert_der.clone()], priv_key.into())?;
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());

    Ok((server_config, cert_der))
}

/// Builds default quinn client config and trusts given certificates.
///
/// ## Args
///
/// - server_certs: a list of trusted certificates in DER format.
fn configure_client(
    server_certs: &[&[u8]],
) -> Result<ClientConfig> {
    let mut certs = rustls::RootCertStore::empty();
    for cert in server_certs {
        certs.add(CertificateDer::from(*cert))?;
    }

    Ok(ClientConfig::with_root_certificates(Arc::new(certs))?)
}