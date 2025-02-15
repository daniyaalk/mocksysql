use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, IpAddr, Ipv4Addr, PrivateKeyDer, ServerName, UnixTime};
use rustls::{ClientConnection, DigitallySignedStruct, Error, ServerConnection, SignatureScheme};
use std::io::{Read, Write};
use std::sync::Arc;

#[derive(Debug)]
pub struct NoCertificateVerification;

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA1,
            SignatureScheme::ECDSA_SHA1_Legacy,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

pub fn handle_tls<RW: Read + Write>(
    from: &mut RW,
    to: &mut RW,
    buf: &mut [u8; 4096],
) -> (ClientConnection, ServerConnection) {
    let CertifiedKey { cert, key_pair } =
        generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            PrivateKeyDer::Pkcs8(key_pair.serialize_der().into()),
        )
        .unwrap();
    let server_config = Arc::new(server_config);
    let mut server = rustls::ServerConnection::new(server_config).unwrap();
    server
        .complete_io(from)
        .expect("Unable to initiate TLS connection with server!");

    let client_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
        .with_no_client_auth();

    let mut client = rustls::ClientConnection::new(
        Arc::new(client_config),
        ServerName::IpAddress(IpAddr::V4(Ipv4Addr::try_from("127.0.0.1").unwrap())),
    )
    .unwrap();
    let a = client.complete_io(to);

    if a.is_err() {
        println!("TLS handshake failure: {:?}", a.err().unwrap());
    }
    println!("TLS handshake complete");

    (client, server)
}

pub fn handle_server_tls<RW: Read + Write>(
    from: &mut RW,
    buf: &mut [u8; 4096],
) -> ClientConnection {
    let client_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
        .with_no_client_auth();

    let mut client = rustls::ClientConnection::new(
        Arc::new(client_config),
        ServerName::IpAddress(IpAddr::V4(Ipv4Addr::try_from("127.0.0.1").unwrap())),
    )
    .unwrap();
    let a = client.complete_io(from);

    if a.is_err() {
        println!("TLS handshake failure: {:?}", a.err().unwrap());
    }
    println!("TLS handshake complete");

    client
}
