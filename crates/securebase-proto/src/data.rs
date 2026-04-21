pub struct InsertRequest {
    pub collection: String,
    pub data: serde_json::Value,
}

pub struct UpdateRequest {
    pub collection: String,
    pub id: String,
    pub data: serde_json::Value,
}

pub struct Row {
    pub id: String,
    pub collection: String,
    pub data: serde_json::Value,
}
