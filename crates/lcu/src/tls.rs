//! TLS to the LCU, trusting exactly one root: Riot's own.

use std::sync::{Arc, OnceLock};

use rustls::{
    CertificateError, ClientConfig, DigitallySignedStruct, Error, RootCertStore, SignatureScheme,
    client::{
        WebPkiServerVerifier,
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    },
    crypto::ring,
    pki_types::{CertificateDer, ServerName, UnixTime, pem::PemObject},
};

const RIOT_ROOT: &[u8] = include_bytes!("riotgames.pem");

pub(crate) fn config() -> Arc<ClientConfig> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    CONFIG.get_or_init(build).clone()
}

fn build() -> Arc<ClientConfig> {
    let provider = Arc::new(ring::default_provider());
    let mut roots = RootCertStore::empty();
    let root =
        CertificateDer::from_pem_slice(RIOT_ROOT).expect("riotgames.pem holds one certificate");
    roots
        .add(root)
        .expect("riotgames.pem is a usable trust anchor");
    let webpki = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone())
        .build()
        .expect("a single root builds a verifier");
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("ring supports the default protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(RiotRoot(webpki)))
        .with_no_client_auth();
    Arc::new(config)
}

/// Webpki's verdict, except that a leaf chaining to the Riot root is accepted whatever name it
/// carries. The endpoint is always loopback and the private root is the whole trust decision;
/// webpki checks the name only after the chain has verified, so nothing else is relaxed.
#[derive(Debug)]
struct RiotRoot(Arc<WebPkiServerVerifier>);

impl ServerCertVerifier for RiotRoot {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        match self
            .0
            .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
        {
            Err(Error::InvalidCertificate(
                CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
            )) => Ok(ServerCertVerified::assertion()),
            verdict => verdict,
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.0.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.0.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.supported_verify_schemes()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_bundled_root_builds_a_config() {
        let config = super::config();
        assert!(!config.crypto_provider().cipher_suites.is_empty());
    }
}
