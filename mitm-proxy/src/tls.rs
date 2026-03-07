use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use rcgen::{
    BasicConstraints, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose, SanType,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::sync::Mutex;

pub struct CertAuthority {
    ca_cert_der: CertificateDer<'static>,
    ca_cert: rcgen::Certificate,
    ca_key_pair: KeyPair,
    cert_cache: Mutex<HashMap<String, Arc<rustls::ServerConfig>>>,
}

impl CertAuthority {
    /// Generate a new CA certificate and key pair.
    /// Writes ca.crt and ca.key to the specified directory.
    pub fn generate(cert_path: &str, key_path: &str) -> Result<Self> {
        let ca_key_pair = KeyPair::generate().context("Failed to generate CA key pair")?;

        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, "ProxyClawd MITM CA");
        params
            .distinguished_name
            .push(DnType::OrganizationName, "ProxyClawd");
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];

        let ca_cert = params
            .self_signed(&ca_key_pair)
            .context("Failed to self-sign CA certificate")?;

        let ca_key_pem = ca_key_pair.serialize_pem();

        std::fs::write(cert_path, ca_cert.pem())
            .with_context(|| format!("Failed to write CA cert to {cert_path}"))?;
        std::fs::write(key_path, &ca_key_pem)
            .with_context(|| format!("Failed to write CA key to {key_path}"))?;

        let ca_cert_der = CertificateDer::from(ca_cert.der().to_vec());

        Ok(Self {
            ca_cert_der,
            ca_cert,
            ca_key_pair,
            cert_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Get or create a rustls ServerConfig for the given domain,
    /// with a leaf certificate signed by our CA.
    pub async fn server_config_for_domain(&self, domain: &str) -> Result<Arc<rustls::ServerConfig>> {
        let mut cache = self.cert_cache.lock().await;
        if let Some(config) = cache.get(domain) {
            return Ok(config.clone());
        }

        let config = self.generate_server_config(domain)?;
        let config = Arc::new(config);
        cache.insert(domain.to_string(), config.clone());
        Ok(config)
    }

    fn generate_server_config(&self, domain: &str) -> Result<rustls::ServerConfig> {
        let leaf_key_pair =
            KeyPair::generate().context("Failed to generate leaf key pair")?;

        let mut leaf_params = CertificateParams::default();
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, domain);

        let san = SanType::DnsName(
            domain
                .to_string()
                .try_into()
                .context("Invalid domain for SAN")?,
        );
        leaf_params.subject_alt_names = vec![san];
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf_params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];

        let leaf_cert = leaf_params
            .signed_by(&leaf_key_pair, &self.ca_cert, &self.ca_key_pair)
            .context("Failed to sign leaf certificate")?;

        let leaf_cert_der = CertificateDer::from(leaf_cert.der().to_vec());
        let leaf_key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key_pair.serialize_der()));

        let cert_chain = vec![leaf_cert_der, self.ca_cert_der.clone()];

        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, leaf_key_der)
            .context("Failed to build rustls ServerConfig")?;

        Ok(server_config)
    }
}
