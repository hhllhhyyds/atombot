//! Web search tool — supports Brave, Tavily, SearXNG, Jina, DuckDuckGo.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agent::tools::{Tool, ToolError};
use crate::security::network::validate_url_target;

const USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_7_2) AppleWebKit/537.36 (compatible; atombot/1.0)";

#[derive(Debug, Clone, Default)]
pub struct WebSearchConfig {
    pub provider: String,
    pub api_key: String,
    pub base_url: String,
    pub max_results: usize,
}

impl WebSearchConfig {
    pub fn from_env() -> Self {
        Self {
            provider: std::env::var("WEB_SEARCH_PROVIDER").unwrap_or_else(|_| "duckduckgo".to_string()),
            api_key: std::env::var("WEB_SEARCH_API_KEY").unwrap_or_default(),
            base_url: std::env::var("WEB_SEARCH_BASE_URL").unwrap_or_default(),
            max_results: std::env::var("WEB_SEARCH_MAX_RESULTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
        }
    }
}

#[derive(Debug, Deserialize)]
struct BraveResponse {
    web: Option<BraveWebResults>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResults {
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    #[serde(rename = "description")]
    content: String,
}

#[derive(Debug, Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct SearXNGResponse {
    results: Vec<SearXNGResult>,
}

#[derive(Debug, Deserialize)]
struct SearXNGResult {
    title: String,
    url: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct JinaResponse {
    data: Vec<JinaResult>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct DuckDuckGoResponse {
    RelatedTopics: Vec<DuckDuckGoTopic>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case, dead_code)]
struct DuckDuckGoTopic {
    Text: Option<String>,
    FirstURL: Option<String>,
    #[serde(default)]
    Icon: DuckDuckGoIcon,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct DuckDuckGoIcon {
    #[serde(rename = "URL")]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JinaResult {
    title: String,
    url: String,
    content: String,
}

pub struct WebSearchTool {
    config: WebSearchConfig,
    proxy: Option<String>,
}

impl WebSearchTool {
    pub fn new(config: WebSearchConfig, proxy: Option<String>) -> Self {
        Self { config, proxy }
    }
}

fn normalize(text: String) -> String {
    let text = regex::Regex::new(r"<[^>]+>")
        .unwrap()
        .replace_all(&text, "");
    // Decode HTML entities
    let text = html_escape::decode_html_entities(&text);
    let text = text.trim().to_string();
    // Normalize whitespace
    let text = regex::Regex::new(r"[ \t]+")
        .unwrap()
        .replace_all(&text, " ");
    let text = regex::Regex::new(r"\n{3,}")
        .unwrap()
        .replace_all(&text, "\n\n");
    text.to_string()
}

fn format_results(query: &str, items: &[SearchItem], count: usize) -> String {
    if items.is_empty() {
        return format!("No results for: {query}");
    }
    let mut lines = vec![format!("Results for: {query}\n")];
    for (i, item) in items.iter().take(count).enumerate() {
        let title = normalize(item.title.clone());
        let snippet = normalize(item.content.clone());
        lines.push(format!("{}. {title}\n   {}", i + 1, item.url));
        if !snippet.is_empty() {
            lines.push(format!("   {snippet}"));
        }
    }
    lines.join("\n")
}

#[derive(Clone)]
struct SearchItem {
    title: String,
    url: String,
    content: String,
}

impl WebSearchTool {
    async fn search_brave(&self, query: &str, n: usize) -> String {
        let api_key = if self.config.api_key.is_empty() {
            std::env::var("BRAVE_API_KEY").unwrap_or_default()
        } else {
            self.config.api_key.clone()
        };

        if api_key.is_empty() {
            return self.search_duckduckgo(query, n).await;
        }

        let client = match self.build_client() {
            Ok(c) => c,
            Err(e) => return format!("Error: {e}"),
        };

        match client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("Accept", "application/json")
            .header("X-Subscription-Token", &api_key)
            .query(&[("q", query), ("count", &n.to_string())])
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status() == 429 {
                    return "Error: Brave API rate limited".to_string();
                }
                match resp.json::<BraveResponse>().await {
                    Ok(data) => {
                        let items: Vec<SearchItem> = data
                            .web
                            .map(|w| w.results)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|r| SearchItem {
                                title: r.title,
                                url: r.url,
                                content: r.content,
                            })
                            .collect();
                        format_results(query, &items, n)
                    }
                    Err(e) => format!("Error parsing Brave response: {e}"),
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn search_tavily(&self, query: &str, n: usize) -> String {
        let api_key = if self.config.api_key.is_empty() {
            std::env::var("TAVILY_API_KEY").unwrap_or_default()
        } else {
            self.config.api_key.clone()
        };

        if api_key.is_empty() {
            return self.search_duckduckgo(query, n).await;
        }

        let client = match self.build_client() {
            Ok(c) => c,
            Err(e) => return format!("Error: {e}"),
        };

        #[derive(Serialize)]
        struct TavilyRequest<'a> {
            query: &'a str,
            max_results: usize,
        }

        match client
            .post("https://api.tavily.com/search")
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&TavilyRequest { query, max_results: n })
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status() == 429 {
                    return "Error: Tavily API rate limited".to_string();
                }
                match resp.json::<TavilyResponse>().await {
                    Ok(data) => {
                        let items: Vec<SearchItem> = data
                            .results
                            .into_iter()
                            .map(|r| SearchItem {
                                title: r.title,
                                url: r.url,
                                content: r.content,
                            })
                            .collect();
                        format_results(query, &items, n)
                    }
                    Err(e) => format!("Error parsing Tavily response: {e}"),
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn search_searxng(&self, query: &str, n: usize) -> String {
        let base_url = if self.config.base_url.is_empty() {
            std::env::var("SEARXNG_BASE_URL").unwrap_or_default()
        } else {
            self.config.base_url.clone()
        };

        if base_url.is_empty() {
            return "Error: SEARXNG_BASE_URL not configured".to_string();
        }

        let endpoint = format!("{}/search", base_url.trim_end_matches('/'));

        if let Err(e) = validate_url_target(&endpoint) {
            return format!("Error: invalid SearXNG URL: {e}");
        }

        let client = match self.build_client() {
            Ok(c) => c,
            Err(e) => return format!("Error: {e}"),
        };

        match client
            .get(&endpoint)
            .header("User-Agent", USER_AGENT)
            .query(&[("q", query), ("format", "json")])
            .send()
            .await
        {
            Ok(resp) => match resp.json::<SearXNGResponse>().await {
                Ok(data) => {
                    let items: Vec<SearchItem> = data
                        .results
                        .into_iter()
                        .take(n)
                        .map(|r| SearchItem {
                            title: r.title,
                            url: r.url,
                            content: r.content,
                        })
                        .collect();
                    format_results(query, &items, n)
                }
                Err(e) => format!("Error parsing SearXNG response: {e}"),
            },
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn search_jina(&self, query: &str, n: usize) -> String {
        let api_key = if self.config.api_key.is_empty() {
            std::env::var("JINA_API_KEY").unwrap_or_default()
        } else {
            self.config.api_key.clone()
        };

        let client = match self.build_client() {
            Ok(c) => c,
            Err(e) => return format!("Error: {e}"),
        };

        let mut req = client
            .get("https://s.jina.ai/")
            .header("Accept", "application/json")
            .query(&[("q", query)]);

        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {api_key}"));
        }

        match req.send().await {
            Ok(resp) => {
                if resp.status() == 429 {
                    return self.search_duckduckgo(query, n).await;
                }
                match resp.json::<JinaResponse>().await {
                    Ok(data) => {
                        let items: Vec<SearchItem> = data
                            .data
                            .into_iter()
                            .take(n)
                            .map(|mut r| {
                                // Truncate content to 500 chars
                                if r.content.len() > 500 {
                                    r.content.truncate(500);
                                }
                                SearchItem {
                                    title: r.title,
                                    url: r.url,
                                    content: r.content,
                                }
                            })
                            .collect();
                        format_results(query, &items, n)
                    }
                    Err(e) => format!("Error parsing Jina response: {e}"),
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    async fn search_duckduckgo(&self, query: &str, n: usize) -> String {
        let client = match self.build_client() {
            Ok(c) => c,
            Err(e) => return format!("Error: {e}"),
        };

        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_redirect=1",
            urlencoding::encode(query)
        );

        match client.get(&url).send().await {
            Ok(resp) => {
                if resp.status() == 403 {
                    return "Error: DuckDuckGo request blocked".to_string();
                }
                match resp.json::<DuckDuckGoResponse>().await {
                    Ok(data) => {
                        let items: Vec<SearchItem> = data
                            .RelatedTopics
                            .into_iter()
                            .filter(|t| t.FirstURL.is_some())
                            .take(n)
                            .map(|t| SearchItem {
                                title: t.Text.clone().unwrap_or_default(),
                                url: t.FirstURL.unwrap_or_default(),
                                content: t.Text.unwrap_or_default(),
                            })
                            .collect();
                        format_results(query, &items, n)
                    }
                    Err(e) => format!("Error parsing DuckDuckGo response: {e}"),
                }
            }
            Err(e) => format!("Error: {e}"),
        }
    }

    fn build_client(&self) -> Result<reqwest::Client, String> {
        let builder = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15));

        let builder = match &self.proxy {
            Some(proxy) => match reqwest::Proxy::http(proxy) {
                Ok(p) => builder.proxy(p),
                Err(e) => return Err(format!("Invalid proxy URL: {e}")),
            },
            None => builder,
        };

        builder.build().map_err(|e| format!("Client build error: {e}"))
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        "Search the web. Returns titles, URLs, and snippets. Free: DuckDuckGo (default), Jina. Paid: Brave, Tavily, SearXNG."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of results (1-10, default 5)",
                    "minimum": 1,
                    "maximum": 10,
                },
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let count = args
            .get("count")
            .and_then(|v| v.as_i64())
            .unwrap_or(self.config.max_results as i64)
            .max(1)
            .min(10) as usize;

        if query.is_empty() {
            return Err(ToolError::InvalidArgs("query is required".to_string()));
        }

        let provider = self.config.provider.trim().to_lowercase();
        let result = match provider.as_str() {
            "brave" => self.search_brave(query, count).await,
            "tavily" => self.search_tavily(query, count).await,
            "searxng" => self.search_searxng(query, count).await,
            "jina" => self.search_jina(query, count).await,
            "duckduckgo" | "" => self.search_duckduckgo(query, count).await,
            _ => format!("Error: unknown search provider '{provider}'. Available: duckduckgo (free), brave, tavily, searxng, jina"),
        };

        Ok(result)
    }
}
