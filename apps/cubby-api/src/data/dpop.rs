use anyhow::Result;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as b64_url;
use base64::Engine as _;
use picky::key::PrivateKey;
use serde_json::Value;
use sha2::{Digest, Sha256};
use turbomcp_dpop::{DpopKeyManager, DpopProofGenerator};

pub struct DPoPKeyPair {
    pub private_key: PrivateKey,
    pub private_jwk: String,
    pub public_jwk_thumbprint: String,
}

#[derive(Debug, Clone)]
pub struct DPoPProofParams {
    pub method: String,
    pub url: String,
    pub access_token: Option<String>,
    pub nonce: Option<String>,
}

/// Generate a new RSA key pair for DPoP
pub fn generate_dpop_keypair() -> Result<DPoPKeyPair> {
    let private_key = PrivateKey::generate_rsa(2048)?;

    // Store as PEM for reconstruction (simplified storage)
    let private_key_pem = private_key.to_pem()?.to_string();
    let public_key = private_key.to_public_key()?;
    let public_key_pem = public_key.to_pem()?.to_string();

    let private_jwk = serde_json::json!({
        "kty": "RSA",
        "use": "sig",
        "alg": "RS256",
        "private_pem": private_key_pem,
        "public_pem": public_key_pem,
    });

    let private_jwk_string = serde_json::to_string(&private_jwk)?;

    // Generate a simple thumbprint based on the public key
    let mut hasher = Sha256::new();
    hasher.update(public_key_pem.as_bytes());
    let hash = hasher.finalize();
    let public_jwk_thumbprint = b64_url.encode(hash);

    Ok(DPoPKeyPair {
        private_key,
        private_jwk: private_jwk_string,
        public_jwk_thumbprint,
    })
}

/// Create a DPoP proof JWT using the turbomcp-dpop crate
pub async fn create_dpop_proof(private_jwk_json: &str, params: &DPoPProofParams) -> Result<String> {
    // Create an in-memory key manager for this operation
    let key_manager = DpopKeyManager::new_memory().await?;

    // Create proof generator
    let proof_generator = DpopProofGenerator::new(key_manager.into());

    // Generate DPoP proof using the turbomcp-dpop API
    let proof_jwt = proof_generator
        .generate_proof(&params.method, &params.url, params.access_token.as_deref())
        .await?;

    Ok(proof_jwt.to_jwt_string())
}

/// Convert JWK JSON back to private key (from our simplified storage format)
fn jwk_to_private_key(jwk_json: &str) -> Result<PrivateKey> {
    let jwk: Value = serde_json::from_str(jwk_json)?;

    // Extract the private key PEM
    let private_pem = jwk["private_pem"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing private key PEM"))?;

    let private_key = PrivateKey::from_pem_str(private_pem)?;

    Ok(private_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_dpop_keypair() {
        let keypair = generate_dpop_keypair().unwrap();
        assert!(!keypair.private_jwk.is_empty());
        assert!(!keypair.public_jwk_thumbprint.is_empty());
    }

    #[tokio::test]
    async fn test_create_dpop_proof() {
        let keypair = generate_dpop_keypair().unwrap();
        let params = DPoPProofParams {
            method: "POST".to_string(),
            url: "https://example.com/token".to_string(),
            access_token: None,
            nonce: None,
        };

        let proof = create_dpop_proof(&keypair.private_jwk, &params)
            .await
            .unwrap();
        assert!(!proof.is_empty());
        // The proof should be a valid JWT format (header.payload.signature)
        assert_eq!(proof.matches('.').count(), 2);
    }
}
