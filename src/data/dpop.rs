use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as b64_url;
use picky::key::PrivateKey;
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

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
    
    // Convert to JWK format for storage
    let private_jwk_value = private_key_to_jwk(&private_key)?;
    let private_jwk = serde_json::to_string(&private_jwk_value)?;
    
    // Generate public key JWK thumbprint
    let public_jwk = extract_public_jwk(&private_jwk_value)?;
    let public_jwk_thumbprint = generate_jwk_thumbprint(&public_jwk)?;
    
    Ok(DPoPKeyPair {
        private_key,
        private_jwk,
        public_jwk_thumbprint,
    })
}

/// Generate a DPoP proof JWT
pub fn create_dpop_proof(
    private_jwk_json: &str,
    params: &DPoPProofParams,
) -> Result<String> {
    let _private_key = jwk_to_private_key(private_jwk_json)?;
    
    // Parse the JWK to get the public key components for the header
    let jwk: Value = serde_json::from_str(private_jwk_json)?;
    let public_jwk = extract_public_jwk(&jwk)?;
    
    // Create JWT header
    let header = json!({
        "typ": "dpop+jwt",
        "alg": "RS256",
        "jwk": public_jwk
    });
    
    let iat = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let jti = generate_jti();
    
    // Create JWT payload/claims
    let mut claims = json!({
        "jti": jti,
        "htm": params.method,
        "htu": canonicalize_url(&params.url)?,
        "iat": iat
    });
    
    // Add access token hash if provided
    if let Some(access_token) = &params.access_token {
        let ath = generate_access_token_hash(access_token)?;
        claims["ath"] = json!(ath);
    }
    
    // Add nonce if provided
    if let Some(nonce) = &params.nonce {
        claims["nonce"] = json!(nonce);
    }
    
    // Encode header and payload
    let header_b64 = b64_url.encode(serde_json::to_string(&header)?.as_bytes());
    let payload_b64 = b64_url.encode(serde_json::to_string(&claims)?.as_bytes());
    
    // Create signing input
    let signing_input = format!("{}.{}", header_b64, payload_b64);
    
    // For now, we'll use a placeholder signature
    // In production, you'd use proper RSA-PSS or PKCS#1 v1.5 signing
    // This is simplified for demonstration
    let signature_placeholder = format!("placeholder_signature_for_{}", generate_jti());
    let signature_b64 = b64_url.encode(signature_placeholder.as_bytes());
    
    Ok(format!("{}.{}", signing_input, signature_b64))
}

/// Convert a private key to JWK format (simplified RSA version)
fn private_key_to_jwk(private_key: &PrivateKey) -> Result<Value> {
    // For now, we'll store the key in PEM format with some metadata
    // In production, you'd want to extract the actual RSA components (n, e, d, etc.)
    let private_key_pem = private_key.to_pem()?.to_string();
    let public_key = private_key.to_public_key()?;
    let public_key_pem = public_key.to_pem()?.to_string();
    
    Ok(json!({
        "kty": "RSA",
        "use": "sig",
        "alg": "RS256",
        "private_pem": private_key_pem,
        "public_pem": public_key_pem,
        // In a full implementation, you'd include n, e, d, p, q, dp, dq, qi
    }))
}

/// Extract public JWK from private JWK
fn extract_public_jwk(private_jwk: &Value) -> Result<Value> {
    Ok(json!({
        "kty": "RSA",
        "use": "sig",
        "alg": "RS256",
        "public_pem": private_jwk["public_pem"]
        // In a full implementation, you'd include n, e from the RSA key
    }))
}

/// Generate JWK thumbprint (SHA-256 hash of canonical JWK)
fn generate_jwk_thumbprint(jwk: &Value) -> Result<String> {
    // For now, use a simplified approach
    let canonical = serde_json::to_string(jwk)?;
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let hash = hasher.finalize();
    Ok(b64_url.encode(hash))
}

/// Convert JWK JSON back to private key
fn jwk_to_private_key(jwk_json: &str) -> Result<PrivateKey> {
    let jwk: Value = serde_json::from_str(jwk_json)?;
    
    // Extract the private key PEM
    let private_pem = jwk["private_pem"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing private key PEM"))?;
    
    let private_key = PrivateKey::from_pem_str(private_pem)?;
    
    Ok(private_key)
}

/// Generate a random JTI (JWT ID)
fn generate_jti() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Canonicalize URL for htu claim (remove fragment and query parameters)
fn canonicalize_url(url: &str) -> Result<String> {
    let parsed = url::Url::parse(url)?;
    Ok(format!("{}://{}{}", parsed.scheme(), parsed.host_str().unwrap_or(""), parsed.path()))
}

/// Generate access token hash for ath claim
fn generate_access_token_hash(access_token: &str) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(access_token.as_bytes());
    let hash = hasher.finalize();
    Ok(b64_url.encode(hash))
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

    #[test]
    fn test_create_dpop_proof() {
        let keypair = generate_dpop_keypair().unwrap();
        let params = DPoPProofParams {
            method: "POST".to_string(),
            url: "https://example.com/token".to_string(),
            access_token: None,
            nonce: None,
        };
        
        let proof = create_dpop_proof(&keypair.private_jwk, &params).unwrap();
        assert!(!proof.is_empty());
        // The proof should be a valid JWT format (header.payload.signature)
        assert_eq!(proof.matches('.').count(), 2);
    }

    #[test]
    fn test_canonicalize_url() {
        let url = "https://example.com:3000/path?query=value#fragment";
        let canonical = canonicalize_url(url).unwrap();
        assert_eq!(canonical, "https://example.com:3000/path");
    }
}