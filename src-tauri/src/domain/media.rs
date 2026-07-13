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
    pub fn invalid_request() -> Self {
        Self {
            code: "invalid_request",
            message: "解析请求无效，请重新开始。".into(),
        }
    }

    pub fn invalid_url() -> Self {
        Self {
            code: "invalid_url",
            message: "请输入完整的 http 或 https 视频页面地址。".into(),
        }
    }

    pub fn engine_unavailable() -> Self {
        Self {
            code: "engine_unavailable",
            message: "未找到媒体解析引擎，请检查 yt-dlp 是否已正确安装。".into(),
        }
    }

    pub fn timed_out() -> Self {
        Self {
            code: "inspect_timeout",
            message: "解析等待时间过长，请稍后重试。".into(),
        }
    }

    pub fn cancelled() -> Self {
        Self {
            code: "inspect_cancelled",
            message: "解析已取消。".into(),
        }
    }

    pub fn output_too_large() -> Self {
        Self {
            code: "engine_output_too_large",
            message: "页面返回的媒体信息过多，暂时无法处理。".into(),
        }
    }

    pub fn engine_failed() -> Self {
        Self {
            code: "engine_failed",
            message: "媒体解析失败，请确认地址可访问后重试。".into(),
        }
    }

    pub fn site_unsupported() -> Self {
        Self {
            code: "site_unsupported",
            message: "这个网站或页面类型暂不受支持。".into(),
        }
    }

    pub fn authentication_required() -> Self {
        Self {
            code: "authentication_required",
            message: "该内容需要登录或 Cookie，当前版本暂不支持账号凭据。".into(),
        }
    }

    pub fn geo_restricted() -> Self {
        Self {
            code: "geo_restricted",
            message: "该内容受地区限制，当前网络位置无法访问。".into(),
        }
    }

    pub fn rate_limited() -> Self {
        Self {
            code: "rate_limited",
            message: "网站暂时限制了请求，请稍后再试。".into(),
        }
    }

    pub fn content_unavailable() -> Self {
        Self {
            code: "content_unavailable",
            message: "该内容可能已删除、设为私密或暂时不可用。".into(),
        }
    }

    pub fn access_denied() -> Self {
        Self {
            code: "access_denied",
            message: "网站拒绝访问该内容，请确认页面在浏览器中可以正常打开。".into(),
        }
    }

    pub fn network_failed() -> Self {
        Self {
            code: "network_failed",
            message: "无法连接到目标网站，请检查网络后重试。".into(),
        }
    }

    pub fn invalid_response() -> Self {
        Self {
            code: "invalid_engine_response",
            message: "媒体解析结果无法识别，请更新 yt-dlp 后重试。".into(),
        }
    }

    pub fn no_formats() -> Self {
        Self {
            code: "no_formats",
            message: "没有找到可用的媒体格式。".into(),
        }
    }
}
