pub struct UploadRequest {
    pub bucket: String,
    pub key: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub struct UploadResponse {
    pub bucket: String,
    pub key: String,
    pub size: u64,
    pub etag: String,
}

pub struct DownloadRequest {
    pub bucket: String,
    pub key: String,
}

pub struct DownloadResponse {
    pub content_type: String,
    pub size: u64,
    pub bytes: Vec<u8>,
}

pub struct DeleteRequest {
    pub bucket: String,
    pub key: String,
}

pub struct Object {
    pub bucket: String,
    pub key: String,
    pub content_type: String,
    pub size: u64,
    pub etag: String,
    pub last_modified: String,
}

pub struct StatRequest {
    pub bucket: String,
    pub key: String,
}

pub struct ListRequest {
    pub bucket: String,
    pub prefix: Option<String>,
    pub continuation_token: Option<String>,
    pub max_keys: Option<u32>,
}

pub struct ListResponse {
    pub objects: Vec<Object>,
    pub next_continuation_token: Option<String>,
}
