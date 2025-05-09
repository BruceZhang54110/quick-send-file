use anyhow::Ok;
use quinn::{Endpoint, ClientConfig, ServerConfig};
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use std::net::SocketAddr;
use anyhow::Result;
use std::sync::Arc;


#[derive(Debug)]
pub struct ClientEndpoint {
    endpoint: Endpoint,
    bind_addr: SocketAddr,
}

impl ClientEndpoint {

    pub fn new(bind_addr: SocketAddr, server_certs: &[&[u8]]) -> Result<Self> {
        let config = Self::generate_client_config(server_certs)?;
        let mut endpoint = Endpoint::client(bind_addr)?;
        endpoint.set_default_client_config(config);
        Ok(ClientEndpoint {
            endpoint: endpoint,
            bind_addr: bind_addr,
        })
    }
    
    /// Constructs a QUIC endpoint configured for use a client only.
    fn generate_client_config(server_certs: &[&[u8]]) -> Result<ClientConfig> {
        let mut certs = rustls::RootCertStore::empty();
        for cert in server_certs {
            certs.add(CertificateDer::from(*cert))?;
        }
        ClientConfig::with_platform_verifier();
        Ok(ClientConfig::with_root_certificates(Arc::new(certs))?)
    }


    
}

#[derive(Debug)]
pub struct ServerEndpoint {
    endpoint: Endpoint,
    bind_addr: SocketAddr,
    certificate: CertificateDer<'static>,
}

impl ServerEndpoint {

    fn new(bind_addr: SocketAddr) -> Result<Self> {
        let (server_config, cert) = Self::generate_server_config()?;
        let endpoint = Endpoint::server(server_config, bind_addr)?;
        Ok(ServerEndpoint {
            endpoint: endpoint,
            bind_addr: bind_addr,
            certificate: cert,
        })
    }
    
    /// Constructs a QUIC endpoint configured for use a server only.
    fn generate_server_config() -> Result<(ServerConfig, CertificateDer<'static>)> {
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
    
}

/// Attempt QUIC connection with the given server address.
async fn run_client(endpoint: &Endpoint, server_addr: SocketAddr) {
    let connect = endpoint.connect(server_addr, "localhost").unwrap();
    let connection = connect.await.unwrap();
    println!("[client] connected: addr={}", connection.remote_address());
}