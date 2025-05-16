use anyhow::Ok;
use quinn::{Endpoint, ClientConfig, ServerConfig};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use std::net::SocketAddr;
use anyhow::Result;
use std::sync::Arc;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::path::Path;

#[derive(Debug)]
pub struct ClientEndpoint {
    endpoint: Endpoint,
    bind_addr: SocketAddr,
}

impl ClientEndpoint {

    pub fn new(bind_addr: SocketAddr, server_cert_path: &Path) -> Result<Self> {
        let config = Self::generate_client_config(server_cert_path)?;
        let mut endpoint = Endpoint::client(bind_addr)?;
        endpoint.set_default_client_config(config);
        Ok(ClientEndpoint {
            endpoint: endpoint,
            bind_addr: bind_addr,
        })
    }
    
    /// Constructs a QUIC endpoint configured for use a client only.
    fn generate_client_config(server_cert_path: &Path) -> Result<ClientConfig> {
        let mut certs = rustls::RootCertStore::empty();
        certs.add(CertificateDer::from_pem_file(server_cert_path)?)?;
        ClientConfig::with_platform_verifier();
        Ok(ClientConfig::with_root_certificates(Arc::new(certs))?)
    }

    pub fn get_endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    


    
}

#[derive(Debug)]
pub struct ServerEndpointContainer {
    endpoint: Endpoint,
    bind_addr: SocketAddr,
}

impl ServerEndpointContainer {

    pub fn new(bind_addr: SocketAddr, cert_path: &Path, server_key_path: &Path) -> Result<Self> {
        let server_config = Self::generate_server_config(cert_path, server_key_path)?;
        let endpoint = Endpoint::server(server_config, bind_addr)?;
        Ok(ServerEndpointContainer {
            endpoint: endpoint,
            bind_addr: bind_addr,
        })
    }
    
    /// Constructs a QUIC endpoint configured for use a server only.
    fn generate_server_config(cert_path: &Path, server_key_path: &Path) -> Result<ServerConfig> {
        // 使用 rcgen 生成自签名证书
        //let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_der = CertificateDer::from_pem_file(cert_path)?;
        let priv_key = PrivatePkcs8KeyDer::from_pem_file(server_key_path)?;
        let mut server_config =
            ServerConfig::with_single_cert(vec![cert_der.clone()], priv_key.into())?;
        let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
        transport_config.max_concurrent_uni_streams(0_u8.into());

        Ok(server_config)
    }

    pub fn get_endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
    pub fn get_bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }
    
}

/// Attempt QUIC connection with the given server address.
async fn run_client(endpoint: &Endpoint, server_addr: SocketAddr) {
    let connect = endpoint.connect(server_addr, "localhost").unwrap();
    let connection = connect.await.unwrap();
    println!("[client] connected: addr={}", connection.remote_address());
}

mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_endpoint() {
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5000);
        let server_endpoint = ServerEndpointContainer::new(addr1, Path::new("server.cert"), Path::new("server.key")).unwrap();
        println!("[server] listening on: addr={:?}", server_endpoint.bind_addr);
        let client_endpoint = ClientEndpoint::new(addr1, Path::new("server.cert")).unwrap();
        println!("[client] listening on: addr={:?}", client_endpoint.endpoint.local_addr());

    }
}