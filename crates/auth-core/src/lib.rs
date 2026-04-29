use std::sync::Arc;

use axum::{
    body::Body,
    extract::{FromRequestParts, State},
    http::{header, request::Parts, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use securebase_proto::auth::TokenClaims;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("missing Authorization header")]
    MissingHeader,
    #[error("malformed Authorization header")]
    MalformedHeader,
    #[error("invalid token: {0}")]
    InvalidToken(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let body = Json(serde_json::json!({ "error": self.to_string() }));
        (StatusCode::UNAUTHORIZED, body).into_response()
    }
}

#[derive(Clone)]
pub struct AuthTokenVerifier {
    decoding_key: Arc<DecodingKey>,
    validation: Arc<Validation>,
}

impl AuthTokenVerifier {
    pub fn new(secret: &[u8]) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);

        /*
        has to be false otherwise gotrue and auth-core audience mismatches reject tokens
         */
        validation.validate_aud = false;

        AuthTokenVerifier {
            decoding_key: Arc::new(DecodingKey::from_secret(secret)),
            validation: Arc::new(validation),
        }
    }

    pub fn verify(&self, token: &str) -> Result<TokenClaims, AuthError> {
        let token_data = decode::<TokenClaims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| AuthError::InvalidToken(e.to_string()))?;

        Ok(token_data.claims)
    }
}

pub async fn require_auth(
    State(verifier): State<AuthTokenVerifier>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, AuthError> {
    let header_value = req
        .headers()
        .get(header::AUTHORIZATION)
        .ok_or(AuthError::MissingHeader)?
        .to_str()
        .map_err(|_| AuthError::MalformedHeader)?;

    let token = header_value
        .strip_prefix("Bearer ")
        .ok_or(AuthError::MalformedHeader)?;

    let claims = verifier.verify(token)?;
    req.extensions_mut().insert(AuthClaims(claims));

    Ok(next.run(req).await)
}

#[derive(Debug, Clone)]
pub struct AuthClaims(pub TokenClaims);

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for AuthClaims {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthClaims>()
            .cloned()
            .ok_or(AuthError::MissingHeader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    #[test]
    fn encode_then_verify_round_trip() {
        let secret = b"supasecret";
        let exp = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600) as i64;
        let claims = TokenClaims {
            subject: "11111111-2222-3333-4444-555555555555".into(),
            exp,
            email: Some("email@gmail.com".into()),
        };
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(secret),
        )
        .unwrap();

        let verifier = AuthTokenVerifier::new(secret);
        let decoded = verifier.verify(&token).unwrap();
        assert_eq!(decoded.subject, "11111111-2222-3333-4444-555555555555");
    }

    #[test]
    fn tampered_token_is_rejected() {
        let secret = b"supasecret";
        let exp = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600) as i64;
        let claims = TokenClaims {
            subject: "abc".into(),
            exp,
            email: None,
        };
        let mut token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(secret),
        )
        .unwrap();
        token.push('x');

        assert!(AuthTokenVerifier::new(secret).verify(&token).is_err());
    }
}
