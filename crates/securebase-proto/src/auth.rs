pub struct SignupRequest {
    pub email: String,
    pub password: String,
}

pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

pub struct AuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub subject: String,
}
