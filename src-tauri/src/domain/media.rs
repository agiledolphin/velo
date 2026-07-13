use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub source_url: String,
    pub title: String,
    pub site: String,
    pub thumbnail_url: Option<String>,
    pub duration_seconds: Option<u64>,
    pub formats: Vec<MediaFormat>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFormat {
    pub id: String,
    pub label: String,
    pub container: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub filesize_bytes: Option<u64>,
    pub has_video: bool,
    pub has_audio: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InspectError {
    pub code: &'static str,
    pub message: String,
}

impl InspectError {
    pub fn invalid_url() -> Self {
        Self {
            code: "invalid_url",
            message: "请输入完整的 http 或 https 视频页面地址。".into(),
        }
    }
}
